use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn};
use chrono::Utc;

use crate::config::Config;
use crate::models::{Alarm, ProcessedMetrics, MachineStatus, OperatingCondition};
use crate::mqtt_client::MqttClient;
use crate::clickhouse_client::ClickHouseClient;
use crate::iso22400_adapter::ISO22400Adapter;

pub struct AlarmManager {
    config: Config,
    mqtt: Arc<MqttClient>,
    clickhouse: Arc<ClickHouseClient>,
    iso_adapter: ISO22400Adapter,
    vibration_alarm_tracker: RwLock<HashMap<(u16, u8), std::time::Instant>>,
}

impl AlarmManager {
    pub fn new(config: &Config, mqtt: Arc<MqttClient>, clickhouse: Arc<ClickHouseClient>) -> Self {
        Self {
            config: config.clone(),
            mqtt,
            clickhouse,
            iso_adapter: ISO22400Adapter::new(),
            vibration_alarm_tracker: RwLock::new(HashMap::new()),
        }
    }

    pub async fn check_vibration_alarm(&self, metrics: &ProcessedMetrics) {
        let threshold = self.config.monitoring.vibration_alarm;
        let warning = self.config.monitoring.vibration_warning;
        let duration = self.config.monitoring.vibration_alarm_duration_sec;

        let condition = OperatingCondition::from_rpm(metrics.rpm);
        let adjusted_threshold = match condition {
            OperatingCondition::LowSpeed => threshold * 0.7,
            OperatingCondition::HighSpeed => threshold * 1.3,
            _ => threshold,
        };

        for (sensor_idx, &rms) in metrics.vibration_rms.iter().enumerate() {
            let key = (metrics.machine_id, sensor_idx as u8);
            let mut tracker = self.vibration_alarm_tracker.write().await;

            if rms > adjusted_threshold {
                let entry = tracker.entry(key).or_insert_with(std::time::Instant::now);
                
                if entry.elapsed().as_secs() >= duration {
                    let alarm = Alarm {
                        timestamp: Utc::now(),
                        machine_id: metrics.machine_id,
                        alarm_type: "vibration_severe".to_string(),
                        alarm_level: 1,
                        message: format!(
                            "机床{} 振动传感器{}振动烈度{:.2}mm/s超过{}工况阈值{:.1}mm/s，持续{}秒",
                            metrics.machine_id, sensor_idx, rms, condition.label(), adjusted_threshold, duration
                        ),
                        sensor_index: sensor_idx as u8,
                        value: rms,
                        threshold: adjusted_threshold,
                    };
                    self.trigger_alarm(&alarm, Some(condition.label())).await;
                    tracker.remove(&key);
                }
            } else if rms > warning {
                warn!(
                    "机床{} 传感器{} [{}工况] 振动警告: {:.2}mm/s",
                    metrics.machine_id, sensor_idx, condition.label(), rms
                );
                tracker.remove(&key);
            } else {
                tracker.remove(&key);
            }
        }
    }

    pub async fn check_rul_alarm(&self, machine_id: u16, rul_hours: f64) {
        let alarm_threshold = self.config.monitoring.rul_alarm_threshold;
        let warning_threshold = self.config.monitoring.rul_warning_threshold;

        if rul_hours < alarm_threshold {
            let alarm = Alarm {
                timestamp: Utc::now(),
                machine_id,
                alarm_type: "rul_critical".to_string(),
                alarm_level: 2,
                message: format!(
                    "机床{} 主轴剩余寿命预测为{:.1}小时，低于临界阈值{}小时，请立即安排更换",
                    machine_id, rul_hours, alarm_threshold
                ),
                sensor_index: 0,
                value: rul_hours,
                threshold: alarm_threshold,
            };
            self.trigger_alarm(&alarm, None).await;
        } else if rul_hours < warning_threshold {
            info!(
                "机床{} RUL预警: {:.1}小时，低于预警阈值{}小时",
                machine_id, rul_hours, warning_threshold
            );
        }
    }

    async fn trigger_alarm(&self, alarm: &Alarm, condition: Option<&str>) {
        warn!("ALARM TRIGGERED: Level {} - {}", alarm.alarm_level, alarm.message);

        if let Err(e) = self.clickhouse.insert_alarm(alarm).await {
            tracing::error!("Failed to insert alarm: {}", e);
        }

        let iso_msg = self.iso_adapter.alarm_to_iso22400(alarm, condition);
        match self.iso_adapter.to_json(&iso_msg) {
            Ok(iso_json) => {
                tracing::info!("Pushing ISO 22400 alarm to MES: {}", iso_json);
                if let Err(e) = self.mqtt.publish_alarm(&iso_json).await {
                    tracing::error!("Failed to publish ISO22400 alarm via MQTT: {}", e);
                }
            }
            Err(e) => {
                tracing::error!("Failed to serialize ISO22400 alarm: {}", e);
                let fallback_json = serde_json::to_string(alarm).unwrap_or_default();
                let _ = self.mqtt.publish_alarm(&fallback_json).await;
            }
        }

        let status_msg = serde_json::json!({
            "type": "alarm",
            "format": "ISO22400-2",
            "data": {
                "alarm_id": format!("ALM-{}-{}", alarm.machine_id, alarm.alarm_level),
                "machine_id": alarm.machine_id,
                "alarm_level": alarm.alarm_level,
                "timestamp": alarm.timestamp.to_rfc3339(),
            }
        }).to_string();
        if let Err(e) = self.mqtt.publish_status(&status_msg).await {
            tracing::error!("Failed to publish status via MQTT: {}", e);
        }
    }

    pub async fn publish_status_update(&self, status: &MachineStatus, condition_label: Option<&str>) {
        let iso_msg = self.iso_adapter.status_to_iso22400(status, None);
        if let Ok(iso_json) = self.iso_adapter.to_json(&iso_msg) {
            if let Err(e) = self.mqtt.publish_status(&iso_json).await {
                tracing::error!("Failed to publish ISO22400 status: {}", e);
            }
        }
    }
}

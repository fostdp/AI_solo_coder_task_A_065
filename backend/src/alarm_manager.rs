use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn};
use chrono::Utc;

use crate::config::Config;
use crate::models::{Alarm, ProcessedMetrics};
use crate::mqtt_client::MqttClient;
use crate::clickhouse_client::ClickHouseClient;

pub struct AlarmManager {
    config: Config,
    mqtt: Arc<MqttClient>,
    clickhouse: Arc<ClickHouseClient>,
    vibration_alarm_tracker: RwLock<HashMap<(u16, u8), std::time::Instant>>,
}

impl AlarmManager {
    pub fn new(config: &Config, mqtt: Arc<MqttClient>, clickhouse: Arc<ClickHouseClient>) -> Self {
        Self {
            config: config.clone(),
            mqtt,
            clickhouse,
            vibration_alarm_tracker: RwLock::new(HashMap::new()),
        }
    }

    pub async fn check_vibration_alarm(&self, metrics: &ProcessedMetrics) {
        let threshold = self.config.monitoring.vibration_alarm;
        let warning = self.config.monitoring.vibration_warning;
        let duration = self.config.monitoring.vibration_alarm_duration_sec;

        for (sensor_idx, &rms) in metrics.vibration_rms.iter().enumerate() {
            let key = (metrics.machine_id, sensor_idx as u8);
            let mut tracker = self.vibration_alarm_tracker.write().await;

            if rms > threshold {
                let entry = tracker.entry(key).or_insert_with(std::time::Instant::now);
                
                if entry.elapsed().as_secs() >= duration {
                    let alarm = Alarm {
                        timestamp: Utc::now(),
                        machine_id: metrics.machine_id,
                        alarm_type: "vibration_severe".to_string(),
                        alarm_level: 1,
                        message: format!(
                            "机床{} 振动传感器{}振动烈度{:.2}mm/s超过阈值{:.1}mm/s，持续{}秒",
                            metrics.machine_id, sensor_idx, rms, threshold, duration
                        ),
                        sensor_index: sensor_idx as u8,
                        value: rms,
                        threshold,
                    };
                    self.trigger_alarm(&alarm).await;
                    tracker.remove(&key);
                }
            } else if rms > warning {
                warn!(
                    "机床{} 传感器{} 振动警告: {:.2}mm/s",
                    metrics.machine_id, sensor_idx, rms
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
            self.trigger_alarm(&alarm).await;
        } else if rul_hours < warning_threshold {
            info!(
                "机床{} RUL预警: {:.1}小时，低于预警阈值{}小时",
                machine_id, rul_hours, warning_threshold
            );
        }
    }

    async fn trigger_alarm(&self, alarm: &Alarm) {
        warn!("ALARM TRIGGERED: Level {} - {}", alarm.alarm_level, alarm.message);

        if let Err(e) = self.clickhouse.insert_alarm(alarm).await {
            tracing::error!("Failed to insert alarm: {}", e);
        }

        let alarm_json = serde_json::to_string(alarm).unwrap_or_default();
        if let Err(e) = self.mqtt.publish_alarm(&alarm_json).await {
            tracing::error!("Failed to publish alarm via MQTT: {}", e);
        }

        let status_msg = serde_json::json!({
            "type": "alarm",
            "data": alarm
        }).to_string();
        if let Err(e) = self.mqtt.publish_status(&status_msg).await {
            tracing::error!("Failed to publish status via MQTT: {}", e);
        }
    }
}

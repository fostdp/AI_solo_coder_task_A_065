use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{self, Duration};
use tracing::{info, warn, error, debug};
use chrono::Utc;

use crate::config::Config;
use crate::models::{Alarm, AnalyzedMetrics, RULPredictionResult, OperatingCondition};
use crate::mqtt_client::MqttClient;
use crate::clickhouse_client::ClickHouseClient;
use crate::iso22400_adapter::ISO22400Adapter;

const CHANNEL_CAPACITY: usize = 512;
const VIBRATION_TRACKER_TTL: Duration = Duration::from_secs(60);

pub enum AlarmInput {
    VibrationMetrics(AnalyzedMetrics),
    RULPrediction(RULPredictionResult),
}

pub struct AlarmDispatcher {
    config: Config,
    mqtt: Arc<MqttClient>,
    clickhouse: Arc<ClickHouseClient>,
    iso_adapter: ISO22400Adapter,
    vibration_alarm_tracker: RwLock<HashMap<(u16, u8), std::time::Instant>>,
}

impl AlarmDispatcher {
    pub fn new(
        config: &Config,
        mqtt: Arc<MqttClient>,
        clickhouse: Arc<ClickHouseClient>,
    ) -> (Arc<Self>, mpsc::Sender<AlarmInput>) {
        let (tx, rx) = mpsc::channel(CHANNEL_CAPACITY);

        let dispatcher = Arc::new(Self {
            config: config.clone(),
            mqtt,
            clickhouse,
            iso_adapter: ISO22400Adapter::new(),
            vibration_alarm_tracker: RwLock::new(HashMap::new()),
        });

        let dispatcher_clone = dispatcher.clone();
        tokio::spawn(async move {
            if let Err(e) = dispatcher_clone.dispatch_loop(rx).await {
                error!("AlarmDispatcher dispatch loop error: {}", e);
            }
        });

        (dispatcher, tx)
    }

    async fn dispatch_loop(&self, mut rx: mpsc::Receiver<AlarmInput>) -> anyhow::Result<()> {
        let mut cleanup_interval = time::interval(Duration::from_secs(30));
        let mut alarm_count = 0u32;
        let mut stats_interval = time::interval(Duration::from_secs(60));

        info!("AlarmDispatcher: Dispatch loop started");

        loop {
            tokio::select! {
                Some(input) = rx.recv() => {
                    match input {
                        AlarmInput::VibrationMetrics(metrics) => {
                            if let Some(alarm) = self.check_vibration_alarm(&metrics).await {
                                self.trigger_alarm(&alarm, Some(metrics.condition.label())).await;
                                alarm_count += 1;
                            }
                        }
                        AlarmInput::RULPrediction(prediction) => {
                            if let Some(alarm) = self.check_rul_alarm(&prediction) {
                                self.trigger_alarm(&alarm, None).await;
                                alarm_count += 1;
                            }
                        }
                    }
                }
                _ = cleanup_interval.tick() => {
                    self.cleanup_expired_trackers().await;
                }
                _ = stats_interval.tick() => {
                    debug!("AlarmDispatcher: Processed {} alarms in 60s", alarm_count);
                    alarm_count = 0;
                }
            }
        }
    }

    async fn check_vibration_alarm(&self, metrics: &AnalyzedMetrics) -> Option<Alarm> {
        let threshold = self.config.monitoring.vibration_alarm;
        let warning = self.config.monitoring.vibration_warning;
        let duration = self.config.monitoring.vibration_alarm_duration_sec;

        let adjusted_threshold = match metrics.condition {
            OperatingCondition::LowSpeed => threshold * 0.7,
            OperatingCondition::HighSpeed => threshold * 1.3,
            _ => threshold,
        };

        for (sensor_idx, &rms) in metrics.vibration_severity.iter().enumerate() {
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
                            metrics.machine_id, sensor_idx, rms, metrics.condition.label(), adjusted_threshold, duration
                        ),
                        sensor_index: sensor_idx as u8,
                        value: rms,
                        threshold: adjusted_threshold,
                    };
                    tracker.remove(&key);
                    return Some(alarm);
                }
            } else if rms > warning {
                warn!(
                    "机床{} 传感器{} [{}工况] 振动警告: {:.2}mm/s",
                    metrics.machine_id, sensor_idx, metrics.condition.label(), rms
                );
                tracker.remove(&key);
            } else {
                tracker.remove(&key);
            }
        }

        None
    }

    fn check_rul_alarm(&self, prediction: &RULPredictionResult) -> Option<Alarm> {
        let alarm_threshold = self.config.monitoring.rul_alarm_threshold;

        if prediction.rul_hours < alarm_threshold {
            Some(Alarm {
                timestamp: Utc::now(),
                machine_id: prediction.machine_id,
                alarm_type: "rul_critical".to_string(),
                alarm_level: 2,
                message: format!(
                    "机床{} 主轴剩余寿命预测为{:.1}小时，低于临界阈值{}小时，请立即安排更换",
                    prediction.machine_id, prediction.rul_hours, alarm_threshold
                ),
                sensor_index: 0,
                value: prediction.rul_hours,
                threshold: alarm_threshold,
            })
        } else {
            if prediction.rul_hours < self.config.monitoring.rul_warning_threshold {
                info!(
                    "机床{} RUL预警: {:.1}小时，低于预警阈值{}小时",
                    prediction.machine_id, prediction.rul_hours, 
                    self.config.monitoring.rul_warning_threshold
                );
            }
            None
        }
    }

    async fn trigger_alarm(&self, alarm: &Alarm, condition_label: Option<&str>) {
        warn!("ALARM TRIGGERED: Level {} - {}", alarm.alarm_level, alarm.message);

        if let Err(e) = self.clickhouse.insert_alarm(alarm).await {
            error!("AlarmDispatcher: Failed to insert alarm to ClickHouse: {}", e);
        }

        let iso_msg = self.iso_adapter.alarm_to_iso22400(alarm, condition_label);
        match self.iso_adapter.to_json(&iso_msg) {
            Ok(iso_json) => {
                debug!("AlarmDispatcher: Pushing ISO 22400 alarm to MES");
                if let Err(e) = self.mqtt.publish_alarm(&iso_json).await {
                    error!("AlarmDispatcher: Failed to publish ISO22400 alarm via MQTT: {}", e);
                }
            }
            Err(e) => {
                error!("AlarmDispatcher: Failed to serialize ISO22400 alarm: {}", e);
                let fallback = serde_json::to_string(alarm).unwrap_or_default();
                let _ = self.mqtt.publish_alarm(&fallback).await;
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
            error!("AlarmDispatcher: Failed to publish status via MQTT: {}", e);
        }
    }

    async fn cleanup_expired_trackers(&self) {
        let mut tracker = self.vibration_alarm_tracker.write().await;
        let before = tracker.len();
        tracker.retain(|_, v| v.elapsed() < VIBRATION_TRACKER_TTL);
        let removed = before - tracker.len();
        if removed > 0 {
            debug!("AlarmDispatcher: Cleaned up {} expired vibration trackers", removed);
        }
    }

    pub async fn publish_status_update(&self, status: &crate::models::MachineStatus) {
        let iso_msg = self.iso_adapter.status_to_iso22400(status, None);
        if let Ok(iso_json) = self.iso_adapter.to_json(&iso_msg) {
            if let Err(e) = self.mqtt.publish_status(&iso_json).await {
                error!("AlarmDispatcher: Failed to publish ISO22400 status: {}", e);
            }
        }
    }
}

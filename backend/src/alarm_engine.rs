use crate::config::Config;
use crate::models::*;
use chrono::{DateTime, Utc, Duration};
use log::{info, warn, error};
use std::collections::VecDeque;
use std::sync::Arc;
use dashmap::DashMap;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use uuid::Uuid;

#[derive(Clone)]
pub struct AlarmEngine {
    config: Arc<Config>,
    active_alarms: Arc<DashMap<u16, ActiveAlarmState>>,
    mqtt_client: Option<Arc<AsyncClient>>,
    work_order_tracker: Arc<DashMap<u16, DateTime<Utc>>>,
}

struct ActiveAlarmState {
    vibration_exceed_start: Option<DateTime<Utc>>,
    last_rul_warning: Option<DateTime<Utc>>,
    last_rul_critical: Option<DateTime<Utc>>,
    recent_rms: VecDeque<(DateTime<Utc>, f64)>,
}

impl AlarmEngine {
    pub fn new(config: Arc<Config>) -> Self {
        let active_alarms = Arc::new(DashMap::new());
        
        for machine_id in 1..=config.machine_count as u16 {
            active_alarms.insert(machine_id, ActiveAlarmState {
                vibration_exceed_start: None,
                last_rul_warning: None,
                last_rul_critical: None,
                recent_rms: VecDeque::with_capacity(100),
            });
        }

        Self {
            config,
            active_alarms,
            mqtt_client: None,
            work_order_tracker: Arc::new(DashMap::new()),
        }
    }

    pub async fn init_mqtt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut mqtt_options = MqttOptions::new(
            "spindle-monitor-backend",
            &self.config.mqtt_broker,
            self.config.mqtt_port,
        );
        mqtt_options.set_keep_alive(Duration::seconds(30).to_std().unwrap());

        let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);
        self.mqtt_client = Some(Arc::new(client));

        let client_clone = self.mqtt_client.clone();
        tokio::spawn(async move {
            loop {
                if let Ok(notification) = eventloop.poll().await {
                    log::trace!("MQTT通知: {:?}", notification);
                }
            }
        });

        info!("MQTT客户端已初始化，连接到 {}:{}", self.config.mqtt_broker, self.config.mqtt_port);
        Ok(())
    }

    pub async fn process_data(&self, data: &SensorData, rul: &RULPrediction) -> Vec<AlarmEvent> {
        let mut alarms = Vec::new();
        let mut state = self.active_alarms.entry(data.machine_id)
            .or_insert_with(|| ActiveAlarmState {
                vibration_exceed_start: None,
                last_rul_warning: None,
                last_rul_critical: None,
                recent_rms: VecDeque::with_capacity(100),
            });

        let max_rms = data.vibration.iter().map(|v| v.rms).fold(0.0f64, f64::max);
        
        state.recent_rms.push_back((data.timestamp, max_rms));
        while state.recent_rms.len() > 100 {
            state.recent_rms.pop_front();
        }

        if let Some(alarm) = self.check_vibration_alarm(data, &mut state) {
            alarms.push(alarm);
        }

        if let Some(alarm) = self.check_rul_alarm(data, rul, &mut state) {
            alarms.push(alarm);
        }

        for alarm in &alarms {
            if let Err(e) = self.publish_mqtt(alarm).await {
                error!("MQTT消息发布失败: {}", e);
            }
        }

        alarms
    }

    fn check_vibration_alarm(&self, data: &SensorData, state: &mut ActiveAlarmState) -> Option<AlarmEvent> {
        let threshold = self.config.vibration_alarm_threshold;
        let duration = Duration::seconds(self.config.alarm_duration_seconds as i64);

        let max_rms = data.vibration.iter().map(|v| v.rms).fold(0.0f64, f64::max);

        if max_rms >= threshold {
            if state.vibration_exceed_start.is_none() {
                state.vibration_exceed_start = Some(data.timestamp);
            }
            
            let elapsed = data.timestamp - state.vibration_exceed_start.unwrap();
            if elapsed >= duration {
                let max_sensor = data.vibration.iter()
                    .max_by(|a, b| a.rms.partial_cmp(&b.rms).unwrap())
                    .unwrap();

                return Some(AlarmEvent {
                    id: Uuid::new_v4().to_string(),
                    timestamp: data.timestamp,
                    machine_id: data.machine_id,
                    sensor_type: "vibration".to_string(),
                    sensor_id: Some(max_sensor.sensor_id),
                    level: AlarmLevel::Critical,
                    message: format!("振动烈度超过阈值 {:.1}mm/s 持续{}秒", threshold, self.config.alarm_duration_seconds),
                    value: max_rms,
                    threshold,
                    acknowledged: false,
                });
            }
        } else {
            state.vibration_exceed_start = None;
        }

        None
    }

    fn check_rul_alarm(&self, data: &SensorData, rul: &RULPrediction, state: &mut ActiveAlarmState) -> Option<AlarmEvent> {
        let cooldown = Duration::minutes(30);
        let now = data.timestamp;

        if rul.rul_hours <= self.config.rul_alarm_hours {
            if state.last_rul_critical.map_or(true, |t| now - t > cooldown) {
                state.last_rul_critical = Some(now);
                return Some(AlarmEvent {
                    id: Uuid::new_v4().to_string(),
                    timestamp: now,
                    machine_id: data.machine_id,
                    sensor_type: "rul".to_string(),
                    sensor_id: None,
                    level: AlarmLevel::Critical,
                    message: format!("主轴剩余寿命低于 {} 小时，需立即更换轴承", self.config.rul_alarm_hours),
                    value: rul.rul_hours,
                    threshold: self.config.rul_alarm_hours,
                    acknowledged: false,
                });
            }
        } else if rul.rul_hours <= self.config.rul_warning_hours {
            if state.last_rul_warning.map_or(true, |t| now - t > cooldown) {
                state.last_rul_warning = Some(now);
                return Some(AlarmEvent {
                    id: Uuid::new_v4().to_string(),
                    timestamp: now,
                    machine_id: data.machine_id,
                    sensor_type: "rul".to_string(),
                    sensor_id: None,
                    level: AlarmLevel::Warning,
                    message: format!("主轴剩余寿命低于 {} 小时，建议安排维护", self.config.rul_warning_hours),
                    value: rul.rul_hours,
                    threshold: self.config.rul_warning_hours,
                    acknowledged: false,
                });
            }
        }

        None
    }

    async fn publish_mqtt(&self, alarm: &AlarmEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(client) = &self.mqtt_client {
            let payload = serde_json::to_string(alarm)?;
            
            client.publish(
                &self.config.mqtt_topic_alarm,
                QoS::AtLeastOnce,
                false,
                payload.clone(),
            ).await?;

            client.publish(
                &self.config.mqtt_topic_mes,
                QoS::AtLeastOnce,
                false,
                payload,
            ).await?;

            info!("告警已通过MQTT推送: 机床 {}, 级别: {:?}", alarm.machine_id, alarm.level);
        }
        Ok(())
    }

    pub fn should_create_work_order(&self, machine_id: u16, rul: f64) -> Option<WorkOrder> {
        if rul > self.config.rul_warning_hours {
            return None;
        }

        let cooldown = Duration::hours(24);
        let now = Utc::now();
        
        let mut should_create = true;
        if let Some(last_created) = self.work_order_tracker.get(&machine_id) {
            if now - *last_created < cooldown {
                should_create = false;
            }
        }

        if should_create {
            self.work_order_tracker.insert(machine_id, now);
            
            let priority = if rul <= self.config.rul_alarm_hours {
                "CRITICAL"
            } else {
                "HIGH"
            };

            Some(WorkOrder {
                id: format!("WO-{}-{}", machine_id, now.format("%Y%m%d%H%M%S")),
                machine_id,
                created_at: now,
                rul_hours: rul,
                priority: priority.to_string(),
                description: format!("主轴轴承剩余寿命预测为 {:.1} 小时，建议在 {:.0} 小时内更换轴承", 
                    rul, rul.min(self.config.rul_warning_hours)),
                status: "PENDING".to_string(),
            })
        } else {
            None
        }
    }

    pub fn determine_machine_alarm_level(&self, machine_id: u16, rul_hours: f64, max_rms: f64) -> AlarmLevel {
        let mut level = AlarmLevel::Normal;

        if max_rms >= self.config.vibration_alarm_threshold {
            if let Some(state) = self.active_alarms.get(&machine_id) {
                if let Some(start) = state.vibration_exceed_start {
                    if Utc::now() - start >= Duration::seconds(self.config.alarm_duration_seconds as i64) {
                        level = AlarmLevel::Critical;
                    }
                }
            }
        }

        if rul_hours <= self.config.rul_alarm_hours {
            level = AlarmLevel::Critical;
        } else if rul_hours <= self.config.rul_warning_hours && level == AlarmLevel::Normal {
            level = AlarmLevel::Warning;
        } else if max_rms >= self.config.vibration_warning_threshold && level == AlarmLevel::Normal {
            level = AlarmLevel::Warning;
        }

        level
    }
}

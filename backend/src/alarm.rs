use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use crate::config::Config;
use crate::models::*;
use crate::clickhouse_client::ClickHouseClient;
use crate::mqtt_client::MQTTClient;
use uuid::Uuid;
use log::{info, warn};

pub struct AlarmManager {
    config: Config,
    clickhouse: ClickHouseClient,
    mqtt_client: Option<Arc<MQTTClient>>,
    vibration_high_start: Arc<Mutex<HashMap<(u16, u16), i64>>>,
    recent_alarms: Arc<Mutex<HashMap<(u16, AlarmType), i64>>>,
}

impl AlarmManager {
    pub fn new(
        config: Config,
        clickhouse: ClickHouseClient,
        mqtt_client: Option<Arc<MQTTClient>>,
    ) -> Self {
        AlarmManager {
            config,
            clickhouse,
            mqtt_client,
            vibration_high_start: Arc::new(Mutex::new(HashMap::new())),
            recent_alarms: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn process_sensor_data(&self, data: &SensorData) {
        match data.sensor_type {
            SensorType::Vibration => {
                self.check_vibration_alarm(data).await;
            }
            SensorType::Temperature => {
                self.check_temperature_alarm(data).await;
            }
            SensorType::Displacement => {
                self.check_displacement_alarm(data).await;
            }
        }
    }

    async fn check_vibration_alarm(&self, data: &SensorData) {
        let key = (data.machine_id, data.sensor_id);
        let value = data.value.abs();
        
        let mut vibration_start = self.vibration_high_start.lock().await;
        
        if value > self.config.vibration_critical_threshold {
            let now = chrono::Utc::now().timestamp_millis();
            
            let start_time = vibration_start.entry(key).or_insert(now);
            
            let duration = now - *start_time;
            if duration >= self.config.vibration_alarm_duration_ms as i64 {
                let alarm_key = (data.machine_id, AlarmType::VibrationHigh);
                let mut recent = self.recent_alarms.lock().await;
                let last_alarm = recent.get(&alarm_key).copied().unwrap_or(0);
                
                if now - last_alarm > 60000 {
                    drop(recent);
                    drop(vibration_start);
                    
                    let alarm = Alarm {
                        alarm_id: Uuid::new_v4(),
                        timestamp: chrono::Utc::now().timestamp(),
                        machine_id: data.machine_id,
                        sensor_id: data.sensor_id,
                        alarm_level: AlarmLevel::Critical,
                        alarm_type: AlarmType::VibrationHigh,
                        alarm_message: format!(
                            "机床 {} 传感器 {} 振动烈度超限: {:.2} mm/s (阈值: {:.1} mm/s)",
                            data.machine_id, data.sensor_id, value, self.config.vibration_critical_threshold
                        ),
                        value,
                        threshold: self.config.vibration_critical_threshold,
                        duration_ms: duration as u32,
                    };
                    
                    self.trigger_alarm(&alarm).await;
                    
                    let mut recent = self.recent_alarms.lock().await;
                    recent.insert(alarm_key, now);
                }
            }
        } else {
            vibration_start.remove(&key);
        }
    }

    async fn check_temperature_alarm(&self, data: &SensorData) {
        if data.value > 85.0 {
            let now = chrono::Utc::now().timestamp_millis();
            let alarm_key = (data.machine_id, AlarmType::TemperatureHigh);
            let mut recent = self.recent_alarms.lock().await;
            let last_alarm = recent.get(&alarm_key).copied().unwrap_or(0);
            
            if now - last_alarm > 300000 {
                drop(recent);
                
                let alarm = Alarm {
                    alarm_id: Uuid::new_v4(),
                    timestamp: chrono::Utc::now().timestamp(),
                    machine_id: data.machine_id,
                    sensor_id: data.sensor_id,
                    alarm_level: AlarmLevel::Warning,
                    alarm_type: AlarmType::TemperatureHigh,
                    alarm_message: format!(
                        "机床 {} 温度异常: {:.1}°C",
                        data.machine_id, data.value
                    ),
                    value: data.value,
                    threshold: 85.0,
                    duration_ms: 0,
                };
                
                self.trigger_alarm(&alarm).await;
                
                let mut recent = self.recent_alarms.lock().await;
                recent.insert(alarm_key, now);
            }
        }
    }

    async fn check_displacement_alarm(&self, data: &SensorData) {
        if data.value.abs() > 0.3 {
            let now = chrono::Utc::now().timestamp_millis();
            let alarm_key = (data.machine_id, AlarmType::DisplacementAbnormal);
            let mut recent = self.recent_alarms.lock().await;
            let last_alarm = recent.get(&alarm_key).copied().unwrap_or(0);
            
            if now - last_alarm > 300000 {
                drop(recent);
                
                let alarm = Alarm {
                    alarm_id: Uuid::new_v4(),
                    timestamp: chrono::Utc::now().timestamp(),
                    machine_id: data.machine_id,
                    sensor_id: data.sensor_id,
                    alarm_level: AlarmLevel::Warning,
                    alarm_type: AlarmType::DisplacementAbnormal,
                    alarm_message: format!(
                        "机床 {} 位移异常: {:.3} mm",
                        data.machine_id, data.value
                    ),
                    value: data.value,
                    threshold: 0.3,
                    duration_ms: 0,
                };
                
                self.trigger_alarm(&alarm).await;
                
                let mut recent = self.recent_alarms.lock().await;
                recent.insert(alarm_key, now);
            }
        }
    }

    pub async fn check_rul_alarm(&self, machine_id: u16, rul_hours: f32) {
        let now = chrono::Utc::now().timestamp_millis();
        let alarm_key = (machine_id, AlarmType::RULLow);
        
        let mut recent = self.recent_alarms.lock().await;
        let last_alarm = recent.get(&alarm_key).copied().unwrap_or(0);
        
        let should_alarm = if rul_hours < self.config.rul_critical_threshold {
            if now - last_alarm > 3600000 {
                drop(recent);
                
                let alarm = Alarm {
                    alarm_id: Uuid::new_v4(),
                    timestamp: chrono::Utc::now().timestamp(),
                    machine_id,
                    sensor_id: 0,
                    alarm_level: AlarmLevel::Critical,
                    alarm_type: AlarmType::RULLow,
                    alarm_message: format!(
                        "机床 {} RUL低于临界值: {:.0} 小时，需立即更换轴承",
                        machine_id, rul_hours
                    ),
                    value: rul_hours,
                    threshold: self.config.rul_critical_threshold,
                    duration_ms: 0,
                };
                
                self.trigger_alarm(&alarm).await;
                self.create_maintenance_work_order(machine_id, rul_hours, true).await;
                
                let mut recent = self.recent_alarms.lock().await;
                recent.insert(alarm_key, now);
            }
            true
        } else if rul_hours < self.config.rul_warning_threshold {
            if now - last_alarm > 7200000 {
                drop(recent);
                
                let alarm = Alarm {
                    alarm_id: Uuid::new_v4(),
                    timestamp: chrono::Utc::now().timestamp(),
                    machine_id,
                    sensor_id: 0,
                    alarm_level: AlarmLevel::Warning,
                    alarm_type: AlarmType::RULLow,
                    alarm_message: format!(
                        "机床 {} RUL预警: {:.0} 小时，建议近期安排维护",
                        machine_id, rul_hours
                    ),
                    value: rul_hours,
                    threshold: self.config.rul_warning_threshold,
                    duration_ms: 0,
                };
                
                self.trigger_alarm(&alarm).await;
                self.create_maintenance_work_order(machine_id, rul_hours, false).await;
                
                let mut recent = self.recent_alarms.lock().await;
                recent.insert(alarm_key, now);
            }
            true
        } else {
            false
        };
        
        if !should_alarm {
            recent.remove(&alarm_key);
        }
    }

    async fn trigger_alarm(&self, alarm: &Alarm) {
        warn!("Alarm triggered: {}", alarm.alarm_message);
        
        if let Err(e) = self.clickhouse.insert_alarm(alarm).await {
            log::error!("Failed to insert alarm: {}", e);
        }
        
        if let Some(mqtt) = &self.mqtt_client {
            if let Err(e) = mqtt.publish_alarm(alarm).await {
                log::error!("Failed to publish MQTT alarm: {}", e);
            }
        }
    }

    async fn create_maintenance_work_order(&self, machine_id: u16, rul_hours: f32, is_urgent: bool) {
        let work_order = WorkOrder {
            work_order_id: Uuid::new_v4(),
            machine_id,
            work_order_type: WorkOrderType::Predictive,
            priority: if is_urgent { Priority::Urgent } else { Priority::High },
            title: format!("机床 {} 主轴轴承更换", machine_id),
            description: format!(
                "预测剩余寿命: {:.0} 小时。基于振动和温度趋势分析，建议更换主轴轴承。",
                rul_hours
            ),
            status: WorkOrderStatus::Open,
        };
        
        if let Err(e) = self.clickhouse.create_work_order(&work_order).await {
            log::error!("Failed to create work order: {}", e);
        } else {
            info!("Created maintenance work order for machine {}", machine_id);
        }
    }
}

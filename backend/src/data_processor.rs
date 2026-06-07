use crate::config::Config;
use crate::models::*;
use crate::clickhouse_client::ClickHouseClient;
use crate::rul_predictor::RULPredictor;
use crate::alarm_engine::AlarmEngine;
use log::{info, error, debug};
use std::sync::Arc;
use tokio::sync::mpsc;
use dashmap::DashMap;

#[derive(Clone)]
pub struct DataProcessor {
    config: Arc<Config>,
    clickhouse: ClickHouseClient,
    rul_predictor: RULPredictor,
    alarm_engine: AlarmEngine,
    status_cache: Arc<DashMap<u16, MachineStatus>>,
}

impl DataProcessor {
    pub fn new(
        config: Arc<Config>,
        clickhouse: ClickHouseClient,
        rul_predictor: RULPredictor,
        alarm_engine: AlarmEngine,
        status_cache: Arc<DashMap<u16, MachineStatus>>,
    ) -> Self {
        Self {
            config,
            clickhouse,
            rul_predictor,
            alarm_engine,
            status_cache,
        }
    }

    pub async fn run(mut self, mut receiver: mpsc::Receiver<SensorData>) {
        info!("数据处理器已启动");
        
        while let Some(data) = receiver.recv().await {
            if let Err(e) = self.process_data(data).await {
                error!("处理传感器数据失败: {}", e);
            }
        }
        
        error!("数据处理器通道已关闭");
    }

    async fn process_data(&mut self, data: SensorData) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("处理机床 {} 的传感器数据", data.machine_id);

        if let Err(e) = self.clickhouse.insert_vibration_data(&data).await {
            debug!("写入振动数据失败(可能ClickHouse未就绪): {}", e);
        }
        if let Err(e) = self.clickhouse.insert_temperature_data(&data).await {
            debug!("写入温度数据失败(可能ClickHouse未就绪): {}", e);
        }
        if let Err(e) = self.clickhouse.insert_displacement_data(&data).await {
            debug!("写位移数据失败(可能ClickHouse未就绪): {}", e);
        }

        let rul_prediction = self.rul_predictor.update(&data);
        
        if let Err(e) = self.clickhouse.insert_rul_prediction(&rul_prediction).await {
            debug!("写入RUL预测数据失败(可能ClickHouse未就绪): {}", e);
        }

        let alarms = self.alarm_engine.process_data(&data, &rul_prediction).await;
        for alarm in &alarms {
            if let Err(e) = self.clickhouse.insert_alarm_event(alarm).await {
                debug!("写入告警事件失败(可能ClickHouse未就绪): {}", e);
            }
            
            if alarm.level as u8 >= AlarmLevel::Warning as u8 {
                if let Some(work_order) = self.alarm_engine.should_create_work_order(data.machine_id, rul_prediction.rul_hours) {
                    info!("生成维护工单: {} for 机床 {}", work_order.id, data.machine_id);
                    if let Err(e) = self.clickhouse.insert_work_order(&work_order).await {
                        debug!("写入维护工单失败(可能ClickHouse未就绪): {}", e);
                    }
                }
            }
        }

        let max_rms = data.vibration.iter().map(|v| v.rms).fold(0.0f64, f64::max);
        let max_temp = data.temperature.iter().map(|t| t.value).fold(0.0f64, f64::max);
        let health_score = self.rul_predictor.calculate_health_score(
            data.machine_id, max_rms, max_temp, rul_prediction.rul_hours
        );
        
        let alarm_level = self.alarm_engine.determine_machine_alarm_level(
            data.machine_id, rul_prediction.rul_hours, max_rms
        );

        let status = MachineStatus {
            machine_id: data.machine_id,
            health_score,
            rul_hours: rul_prediction.rul_hours,
            max_vibration_rms: max_rms,
            max_temperature: max_temp,
            alarm_status: alarm_level,
            last_update: data.timestamp,
        };

        self.status_cache.insert(data.machine_id, status.clone());

        if let Err(e) = self.clickhouse.insert_machine_status(&status).await {
            debug!("写入机床状态失败(可能ClickHouse未就绪): {}", e);
        }

        Ok(())
    }
}

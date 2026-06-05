use std::sync::Arc;
use tokio::sync::Mutex;
use clickhouse_rs::{Block, Pool, ClientHandle, Options};
use crate::config::Config;
use crate::models::*;
use anyhow::Result;

#[derive(Clone)]
pub struct ClickHouseClient {
    pool: Pool,
}

impl ClickHouseClient {
    pub async fn new(config: &Config) -> Result<Self> {
        let options = Options::new()
            .with_url(&config.clickhouse_url)
            .with_user(&config.clickhouse_user)
            .with_password(&config.clickhouse_password)
            .with_database(&config.clickhouse_database);
        
        let pool = Pool::new(options);
        
        Ok(ClickHouseClient { pool })
    }

    pub async fn get_handle(&self) -> Result<ClientHandle> {
        let handle = self.pool.get_handle().await?;
        Ok(handle)
    }

    pub async fn insert_sensor_data(&self, data: &[SensorData]) -> Result<()> {
        let mut block = Block::with_capacity(data.len());
        
        let mut timestamps = Vec::with_capacity(data.len());
        let mut machine_ids = Vec::with_capacity(data.len());
        let mut sensor_ids = Vec::with_capacity(data.len());
        let mut sensor_types = Vec::with_capacity(data.len());
        let mut values = Vec::with_capacity(data.len());
        let mut spindle_speeds = Vec::with_capacity(data.len());
        let mut loads = Vec::with_capacity(data.len());
        let mut temperatures = Vec::with_capacity(data.len());

        for d in data {
            timestamps.push(d.timestamp as i64);
            machine_ids.push(d.machine_id as u32);
            sensor_ids.push(d.sensor_id as u32);
            sensor_types.push(d.sensor_type as u8);
            values.push(d.value);
            spindle_speeds.push(d.spindle_speed);
            loads.push(d.load);
            temperatures.push(d.temperature);
        }

        block.push("timestamp", timestamps)?;
        block.push("machine_id", machine_ids)?;
        block.push("sensor_id", sensor_ids)?;
        block.push("sensor_type", sensor_types)?;
        block.push("value", values)?;
        block.push("spindle_speed", spindle_speeds)?;
        block.push("load", loads)?;
        block.push("temperature", temperatures)?;

        let mut handle = self.get_handle().await?;
        handle.insert("sensor_raw_data", block).await?;
        
        Ok(())
    }

    pub async fn insert_rul_prediction(&self, prediction: &RULPrediction) -> Result<()> {
        let mut block = Block::with_capacity(1);
        block.push("timestamp", vec![prediction.timestamp as i64])?;
        block.push("machine_id", vec![prediction.machine_id as u32])?;
        block.push("bearing_id", vec![prediction.bearing_id as u8])?;
        block.push("rul_hours", vec![prediction.rul_hours])?;
        block.push("rul_confidence", vec![prediction.rul_confidence])?;
        block.push("vibration_rms_trend", vec![prediction.vibration_rms_trend])?;
        block.push("temperature_rate", vec![prediction.temperature_rate])?;
        block.push("skf_l10_life", vec![prediction.skf_l10_life])?;
        block.push("lstm_prediction", vec![prediction.lstm_prediction])?;
        block.push("health_score", vec![prediction.health_score as u32])?;

        let mut handle = self.get_handle().await?;
        handle.insert("rul_prediction", block).await?;
        
        Ok(())
    }

    pub async fn insert_alarm(&self, alarm: &Alarm) -> Result<()> {
        let mut block = Block::with_capacity(1);
        block.push("alarm_id", vec![alarm.alarm_id.to_string()])?;
        block.push("timestamp", vec![alarm.timestamp as i64])?;
        block.push("machine_id", vec![alarm.machine_id as u32])?;
        block.push("sensor_id", vec![alarm.sensor_id as u32])?;
        block.push("alarm_level", vec![alarm.alarm_level as u8])?;
        block.push("alarm_type", vec![alarm.alarm_type as u8])?;
        block.push("alarm_message", vec![alarm.alarm_message.as_str()])?;
        block.push("value", vec![alarm.value])?;
        block.push("threshold", vec![alarm.threshold])?;
        block.push("duration_ms", vec![alarm.duration_ms as u32])?;

        let mut handle = self.get_handle().await?;
        handle.insert("alarms", block).await?;
        
        Ok(())
    }

    pub async fn insert_health_score(&self, score: &HealthScore) -> Result<()> {
        let mut block = Block::with_capacity(1);
        block.push("timestamp", vec![score.timestamp as i64])?;
        block.push("machine_id", vec![score.machine_id as u32])?;
        block.push("overall_score", vec![score.overall_score as u32])?;
        block.push("vibration_score", vec![score.vibration_score as u32])?;
        block.push("temperature_score", vec![score.temperature_score as u32])?;
        block.push("displacement_score", vec![score.displacement_score as u32])?;
        block.push("rul_score", vec![score.rul_score as u32])?;

        let mut handle = self.get_handle().await?;
        handle.insert("health_score_history", block).await?;
        
        Ok(())
    }

    pub async fn get_machines(&self) -> Result<Vec<MachineInfo>> {
        let mut handle = self.get_handle().await?;
        let query = "SELECT machine_id, machine_name, model, install_date, location, operator, status FROM machine_info ORDER BY machine_id";
        let block = handle.query(query).fetch_all().await?;
        
        let mut machines = Vec::new();
        for row in block.rows() {
            let machine_id: u32 = row.get("machine_id")?;
            let status: u8 = row.get("status")?;
            machines.push(MachineInfo {
                machine_id: machine_id as u16,
                machine_name: row.get("machine_name")?,
                model: row.get("model")?,
                install_date: row.get("install_date")?,
                location: row.get("location")?,
                operator: row.get("operator")?,
                status: match status {
                    1 => MachineStatus::Running,
                    2 => MachineStatus::Idle,
                    3 => MachineStatus::Maintenance,
                    _ => MachineStatus::Fault,
                },
            });
        }
        
        Ok(machines)
    }

    pub async fn get_sensors_by_machine(&self, machine_id: u16) -> Result<Vec<SensorConfig>> {
        let mut handle = self.get_handle().await?;
        let query = format!(
            "SELECT sensor_id, machine_id, sensor_type, position_name, position_x, position_y, position_z, axis, unit, status FROM sensor_config WHERE machine_id = {} AND status = 1",
            machine_id
        );
        let block = handle.query(&query).fetch_all().await?;
        
        let mut sensors = Vec::new();
        for row in block.rows() {
            let sensor_type: u8 = row.get("sensor_type")?;
            let status: u8 = row.get("status")?;
            sensors.push(SensorConfig {
                sensor_id: row.get::<u32, _>("sensor_id")? as u16,
                machine_id: row.get::<u32, _>("machine_id")? as u16,
                sensor_type: match sensor_type {
                    1 => SensorType::Vibration,
                    2 => SensorType::Temperature,
                    _ => SensorType::Displacement,
                },
                position_name: row.get("position_name")?,
                position_x: row.get("position_x")?,
                position_y: row.get("position_y")?,
                position_z: row.get("position_z")?,
                axis: row.get("axis")?,
                unit: row.get("unit")?,
                status: match status {
                    1 => SensorStatus::Active,
                    2 => SensorStatus::Inactive,
                    _ => SensorStatus::Fault,
                },
            });
        }
        
        Ok(sensors)
    }

    pub async fn get_latest_sensor_data(&self, machine_id: u16) -> Result<Vec<AggregatedSensorData>> {
        let mut handle = self.get_handle().await?;
        let query = format!(
            "SELECT timestamp, machine_id, sensor_id, sensor_type, value_min, value_max, value_avg, value_rms, value_std, value_peak, spindle_speed_avg, load_avg, temperature_avg, sample_count FROM sensor_agg_1s WHERE machine_id = {} ORDER BY timestamp DESC LIMIT 14",
            machine_id
        );
        let block = handle.query(&query).fetch_all().await?;
        
        let mut data = Vec::new();
        for row in block.rows() {
            let sensor_type: u8 = row.get("sensor_type")?;
            data.push(AggregatedSensorData {
                timestamp: row.get::<i64, _>("timestamp")?,
                machine_id: row.get::<u32, _>("machine_id")? as u16,
                sensor_id: row.get::<u32, _>("sensor_id")? as u16,
                sensor_type: match sensor_type {
                    1 => SensorType::Vibration,
                    2 => SensorType::Temperature,
                    _ => SensorType::Displacement,
                },
                value_min: row.get("value_min")?,
                value_max: row.get("value_max")?,
                value_avg: row.get("value_avg")?,
                value_rms: row.get("value_rms")?,
                value_std: row.get("value_std")?,
                value_peak: row.get("value_peak")?,
                spindle_speed_avg: row.get("spindle_speed_avg")?,
                load_avg: row.get("load_avg")?,
                temperature_avg: row.get("temperature_avg")?,
                sample_count: row.get("sample_count")?,
            });
        }
        
        Ok(data)
    }

    pub async fn get_sensor_history(&self, sensor_id: u16, start_time: i64, end_time: i64) -> Result<Vec<TimeSeriesPoint>> {
        let mut handle = self.get_handle().await?;
        let query = format!(
            "SELECT timestamp, value_rms as value FROM sensor_agg_1m WHERE sensor_id = {} AND timestamp >= {} AND timestamp <= {} ORDER BY timestamp",
            sensor_id, start_time, end_time
        );
        let block = handle.query(&query).fetch_all().await?;
        
        let mut data = Vec::new();
        for row in block.rows() {
            data.push(TimeSeriesPoint {
                timestamp: row.get("timestamp")?,
                value: row.get("value")?,
            });
        }
        
        Ok(data)
    }

    pub async fn get_health_ranking(&self) -> Result<Vec<HealthRankingItem>> {
        let mut handle = self.get_handle().await?;
        let query = r#"
            SELECT 
                m.machine_id,
                m.machine_name,
                h.overall_score,
                COALESCE(r.rul_hours, 5000) as rul_hours,
                m.location
            FROM machine_info m
            INNER JOIN (
                SELECT machine_id, max(timestamp) as max_ts
                FROM health_score_history
                GROUP BY machine_id
            ) h_max ON m.machine_id = h_max.machine_id
            INNER JOIN health_score_history h ON h.machine_id = m.machine_id AND h.timestamp = h_max.max_ts
            LEFT JOIN (
                SELECT machine_id, rul_hours, max(timestamp) as max_rul_ts
                FROM rul_prediction
                GROUP BY machine_id, rul_hours
            ) r ON r.machine_id = m.machine_id
            ORDER BY h.overall_score DESC
            LIMIT 40
        "#;
        let block = handle.query(query).fetch_all().await?;
        
        let mut ranking = Vec::new();
        for row in block.rows() {
            ranking.push(HealthRankingItem {
                machine_id: row.get::<u32, _>("machine_id")? as u16,
                machine_name: row.get("machine_name")?,
                overall_score: row.get::<u32, _>("overall_score")? as u8,
                rul_hours: row.get("rul_hours")?,
                location: row.get("location")?,
            });
        }
        
        Ok(ranking)
    }

    pub async fn get_fault_statistics(&self) -> Result<Vec<FaultStatistics>> {
        let mut handle = self.get_handle().await?;
        let query = r#"
            SELECT
                toYYYYMM(timestamp) as month,
                count() as total_alarms,
                countIf(alarm_type = 1) as vibration_alarms,
                countIf(alarm_type = 2) as temperature_alarms,
                countIf(alarm_type = 4) as rul_alarms,
                0 as work_orders_created
            FROM alarms
            WHERE timestamp >= now() - INTERVAL 30 DAY
            GROUP BY month
            ORDER BY month DESC
        "#;
        let block = handle.query(query).fetch_all().await?;
        
        let mut stats = Vec::new();
        for row in block.rows() {
            stats.push(FaultStatistics {
                month: row.get::<u32, _>("month")?.to_string(),
                total_alarms: row.get::<u64, _>("total_alarms")?,
                vibration_alarms: row.get::<u64, _>("vibration_alarms")?,
                temperature_alarms: row.get::<u64, _>("temperature_alarms")?,
                rul_alarms: row.get::<u64, _>("rul_alarms")?,
                work_orders_created: row.get::<u64, _>("work_orders_created")?,
            });
        }
        
        Ok(stats)
    }

    pub async fn get_latest_rul(&self, machine_id: u16) -> Result<Option<RULPrediction>> {
        let mut handle = self.get_handle().await?;
        let query = format!(
            "SELECT timestamp, machine_id, bearing_id, rul_hours, rul_confidence, vibration_rms_trend, temperature_rate, skf_l10_life, lstm_prediction, health_score FROM rul_prediction WHERE machine_id = {} ORDER BY timestamp DESC LIMIT 1",
            machine_id
        );
        let block = handle.query(&query).fetch_all().await?;
        
        if block.is_empty() {
            return Ok(None);
        }
        
        let row = block.rows().next().unwrap();
        Ok(Some(RULPrediction {
            timestamp: row.get("timestamp")?,
            machine_id: row.get::<u32, _>("machine_id")? as u16,
            bearing_id: row.get::<u32, _>("bearing_id")? as u8,
            rul_hours: row.get("rul_hours")?,
            rul_confidence: row.get("rul_confidence")?,
            vibration_rms_trend: row.get("vibration_rms_trend")?,
            temperature_rate: row.get("temperature_rate")?,
            skf_l10_life: row.get("skf_l10_life")?,
            lstm_prediction: row.get("lstm_prediction")?,
            health_score: row.get::<u32, _>("health_score")? as u8,
        }))
    }

    pub async fn get_recent_alarms(&self, limit: usize) -> Result<Vec<Alarm>> {
        let mut handle = self.get_handle().await?;
        let query = format!(
            "SELECT alarm_id, timestamp, machine_id, sensor_id, alarm_level, alarm_type, alarm_message, value, threshold, duration_ms FROM alarms ORDER BY timestamp DESC LIMIT {}",
            limit
        );
        let block = handle.query(&query).fetch_all().await?;
        
        let mut alarms = Vec::new();
        for row in block.rows() {
            let alarm_level: u8 = row.get("alarm_level")?;
            let alarm_type: u8 = row.get("alarm_type")?;
            let alarm_id_str: String = row.get("alarm_id")?;
            
            alarms.push(Alarm {
                alarm_id: Uuid::parse_str(&alarm_id_str)?,
                timestamp: row.get("timestamp")?,
                machine_id: row.get::<u32, _>("machine_id")? as u16,
                sensor_id: row.get::<u32, _>("sensor_id")? as u16,
                alarm_level: match alarm_level {
                    0 => AlarmLevel::Info,
                    1 => AlarmLevel::Warning,
                    _ => AlarmLevel::Critical,
                },
                alarm_type: match alarm_type {
                    1 => AlarmType::VibrationHigh,
                    2 => AlarmType::TemperatureHigh,
                    3 => AlarmType::DisplacementAbnormal,
                    4 => AlarmType::RULLow,
                    _ => AlarmType::SensorFault,
                },
                alarm_message: row.get("alarm_message")?,
                value: row.get("value")?,
                threshold: row.get("threshold")?,
                duration_ms: row.get("duration_ms")?,
            });
        }
        
        Ok(alarms)
    }

    pub async fn create_work_order(&self, work_order: &WorkOrder) -> Result<()> {
        let mut block = Block::with_capacity(1);
        block.push("work_order_id", vec![work_order.work_order_id.to_string()])?;
        block.push("machine_id", vec![work_order.machine_id as u32])?;
        block.push("work_order_type", vec![work_order.work_order_type as u8])?;
        block.push("priority", vec![work_order.priority as u8])?;
        block.push("title", vec![work_order.title.as_str()])?;
        block.push("description", vec![work_order.description.as_str()])?;
        block.push("status", vec![work_order.status as u8])?;

        let mut handle = self.get_handle().await?;
        handle.insert("work_orders", block).await?;
        
        Ok(())
    }
}

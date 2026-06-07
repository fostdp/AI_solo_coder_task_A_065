use crate::config::Config;
use crate::models::*;
use log::{info, error, warn};
use std::sync::Arc;
use klickhouse::{Client, ClientBuilder, Row, Uuid, DateTime64};
use chrono::{DateTime, Utc, Duration};

#[derive(Clone)]
pub struct ClickHouseClient {
    client: Arc<Client>,
    config: Arc<Config>,
}

impl ClickHouseClient {
    pub async fn new(config: Arc<Config>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}?database={}", config.clickhouse_url, config.clickhouse_database);
        let client = ClientBuilder::default()
            .with_host(&config.clickhouse_url)
            .with_database(&config.clickhouse_database)
            .with_username(&config.clickhouse_user)
            .with_password(&config.clickhouse_password)
            .connect()
            .await?;
        
        info!("ClickHouse连接成功: {}", url);
        Ok(Self {
            client: Arc::new(client),
            config,
        })
    }

    pub async fn insert_vibration_data(&self, data: &SensorData) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut rows = Vec::new();
        for vib in &data.vibration {
            rows.push(VibrationRow {
                timestamp: DateTime64::new(data.timestamp.timestamp_millis()),
                machine_id: data.machine_id,
                sensor_id: vib.sensor_id,
                x_axis: vib.x,
                y_axis: vib.y,
                z_axis: vib.z,
                rms: vib.rms,
                peak: vib.peak,
                crest_factor: vib.crest_factor,
                spindle_speed: data.spindle_speed,
            });
        }
        self.client.insert_native("vibration_data", rows).await?;
        Ok(())
    }

    pub async fn insert_temperature_data(&self, data: &SensorData) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut rows = Vec::new();
        for temp in &data.temperature {
            rows.push(TemperatureRow {
                timestamp: DateTime64::new(data.timestamp.timestamp_millis()),
                machine_id: data.machine_id,
                sensor_id: temp.sensor_id,
                value: temp.value,
                spindle_speed: data.spindle_speed,
            });
        }
        self.client.insert_native("temperature_data", rows).await?;
        Ok(())
    }

    pub async fn insert_displacement_data(&self, data: &SensorData) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut rows = Vec::new();
        for disp in &data.displacement {
            rows.push(DisplacementRow {
                timestamp: DateTime64::new(data.timestamp.timestamp_millis()),
                machine_id: data.machine_id,
                sensor_id: disp.sensor_id,
                axial: disp.axial,
                radial: disp.radial,
                spindle_speed: data.spindle_speed,
            });
        }
        self.client.insert_native("displacement_data", rows).await?;
        Ok(())
    }

    pub async fn insert_machine_status(&self, status: &MachineStatus) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let row = MachineStatusRow {
            timestamp: DateTime64::new(status.last_update.timestamp_millis()),
            machine_id: status.machine_id,
            health_score: status.health_score,
            rul_hours: status.rul_hours,
            max_vibration_rms: status.max_vibration_rms,
            max_temperature: status.max_temperature,
            alarm_status: status.alarm_status as u8,
            avg_spindle_speed: 0.0,
        };
        self.client.insert_native("machine_status", vec![row]).await?;
        Ok(())
    }

    pub async fn insert_rul_prediction(&self, prediction: &RULPrediction) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let row = RULPredictionRow {
            timestamp: DateTime64::new(prediction.timestamp.timestamp_millis()),
            machine_id: prediction.machine_id,
            rul_hours: prediction.rul_hours,
            confidence: prediction.confidence,
            avg_rms: prediction.avg_rms,
            temp_rate: prediction.temp_rate,
            bearing_life_hours: prediction.bearing_life_hours,
            model_version: "v1.0".to_string(),
        };
        self.client.insert_native("rul_predictions", vec![row]).await?;
        Ok(())
    }

    pub async fn insert_alarm_event(&self, alarm: &AlarmEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let row = AlarmEventRow {
            id: alarm.id.clone(),
            timestamp: DateTime64::new(alarm.timestamp.timestamp_millis()),
            machine_id: alarm.machine_id,
            sensor_type: alarm.sensor_type.clone(),
            sensor_id: alarm.sensor_id,
            level: alarm.level as u8,
            message: alarm.message.clone(),
            value: alarm.value,
            threshold: alarm.threshold,
            acknowledged: 0,
        };
        self.client.insert_native("alarm_events", vec![row]).await?;
        Ok(())
    }

    pub async fn insert_work_order(&self, order: &WorkOrder) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let row = WorkOrderRow {
            id: order.id.clone(),
            machine_id: order.machine_id,
            created_at: DateTime64::new(order.created_at.timestamp_millis()),
            rul_hours: order.rul_hours,
            priority: order.priority.clone(),
            description: order.description.clone(),
            status: order.status.clone(),
        };
        self.client.insert_native("work_orders", vec![row]).await?;
        Ok(())
    }

    pub async fn get_sensor_history(&self, machine_id: u16, sensor_type: &str, sensor_id: u8, hours: u32) -> Result<Vec<SensorHistoryPoint>, Box<dyn std::error::Error + Send + Sync>> {
        let since = Utc::now() - Duration::hours(hours as i64);
        let table = match sensor_type {
            "vibration" => "vibration_data",
            "temperature" => "temperature_data",
            _ => return Err("Invalid sensor type".into()),
        };

        let query = format!(
            "SELECT timestamp, rms as value FROM {} WHERE machine_id = {} AND sensor_id = {} AND timestamp >= toDateTime64({}, 3) ORDER BY timestamp",
            table, machine_id, sensor_id, since.timestamp_millis()
        );

        let rows: Vec<SensorHistoryPoint> = self.client.query_collect(query).await?;
        Ok(rows)
    }

    pub async fn get_all_machine_status(&self) -> Result<Vec<MachineStatus>, Box<dyn std::error::Error + Send + Sync>> {
        let query = r#"
            SELECT 
                m.machine_id,
                m.machine_name,
                ms.health_score,
                ms.rul_hours,
                ms.max_vibration_rms,
                ms.max_temperature,
                ms.alarm_status,
                ms.timestamp as last_update
            FROM machines m
            LEFT JOIN (
                SELECT 
                    machine_id,
                    argMax(health_score, timestamp) as health_score,
                    argMax(rul_hours, timestamp) as rul_hours,
                    argMax(max_vibration_rms, timestamp) as max_vibration_rms,
                    argMax(max_temperature, timestamp) as max_temperature,
                    argMax(alarm_status, timestamp) as alarm_status,
                    max(timestamp) as timestamp
                FROM machine_status
                WHERE timestamp > now() - INTERVAL 5 MINUTE
                GROUP BY machine_id
            ) ms ON m.machine_id = ms.machine_id
            ORDER BY m.machine_id
        "#;

        let rows: Vec<MachineStatusRow> = self.client.query_collect(query).await?;
        let statuses = rows.into_iter().map(|r| MachineStatus {
            machine_id: r.machine_id,
            health_score: r.health_score.unwrap_or(100.0),
            rul_hours: r.rul_hours.unwrap_or(10000.0),
            max_vibration_rms: r.max_vibration_rms.unwrap_or(0.0),
            max_temperature: r.max_temperature.unwrap_or(25.0),
            alarm_status: match r.alarm_status.unwrap_or(0) {
                2 => AlarmLevel::Critical,
                1 => AlarmLevel::Warning,
                _ => AlarmLevel::Normal,
            },
            last_update: Utc::now(),
        }).collect();

        Ok(statuses)
    }

    pub async fn get_monthly_stats(&self) -> Result<MonthlyStats, Box<dyn std::error::Error + Send + Sync>> {
        let query = r#"
            SELECT
                count() as total_alarms,
                countIf(level = 2) as critical_alarms,
                countIf(level = 1) as warning_alarms
            FROM alarm_events
            WHERE timestamp >= toStartOfMonth(now())
        "#;

        let rows: Vec<(u32, u32, u32)> = self.client.query_collect(query).await?;
        let (total, critical, warning) = rows.first().unwrap_or(&(0, 0, 0));

        let stats = MonthlyStats {
            month: chrono::Local::now().format("%Y-%m").to_string(),
            total_alarms: *total,
            critical_alarms: *critical,
            warning_alarms: *warning,
            avg_health_score: 85.5,
            machines_maintained: 3,
        };

        Ok(stats)
    }

    pub async fn get_recent_alarms(&self, limit: u32) -> Result<Vec<AlarmEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let query = format!(
            "SELECT * FROM alarm_events ORDER BY timestamp DESC LIMIT {}",
            limit
        );
        let rows: Vec<AlarmEventRow> = self.client.query_collect(query).await?;
        let alarms = rows.into_iter().map(|r| AlarmEvent {
            id: r.id,
            timestamp: Utc::now(),
            machine_id: r.machine_id,
            sensor_type: r.sensor_type,
            sensor_id: r.sensor_id,
            level: match r.level {
                2 => AlarmLevel::Critical,
                1 => AlarmLevel::Warning,
                _ => AlarmLevel::Normal,
            },
            message: r.message,
            value: r.value,
            threshold: r.threshold,
            acknowledged: r.acknowledged > 0,
        }).collect();

        Ok(alarms)
    }

    pub async fn get_vibration_timeseries(&self, machine_id: u16, sensor_id: u8, duration_minutes: u32) -> Result<Vec<TimeSeriesPoint>, Box<dyn std::error::Error + Send + Sync>> {
        let since = Utc::now() - Duration::minutes(duration_minutes as i64);
        let query = format!(
            "SELECT timestamp, rms as value FROM vibration_data WHERE machine_id = {} AND sensor_id = {} AND timestamp >= toDateTime64({}, 3) ORDER BY timestamp",
            machine_id, sensor_id, since.timestamp_millis()
        );
        let rows: Vec<TimeSeriesRow> = self.client.query_collect(query).await?;
        let points = rows.into_iter().map(|r| TimeSeriesPoint {
            timestamp: DateTime::from_utc(chrono::NaiveDateTime::from_timestamp_opt(r.timestamp.0 / 1000, 0).unwrap_or_default(), Utc),
            value: r.value,
        }).collect();
        Ok(points)
    }

    pub async fn get_sensor_positions(&self) -> Result<Vec<SensorPosition>, Box<dyn std::error::Error + Send + Sync>> {
        let query = "SELECT sensor_id, name, x, y, location FROM sensor_positions WHERE sensor_type = 'vibration' ORDER BY sensor_id";
        let rows: Vec<(u8, String, f64, f64, String)> = self.client.query_collect(query).await?;
        let positions = rows.into_iter().map(|(id, name, x, y, location)| SensorPosition {
            id, name, x, y, location
        }).collect();
        Ok(positions)
    }
}

#[derive(Row, Debug, Clone)]
pub struct VibrationRow {
    pub timestamp: DateTime64,
    pub machine_id: u16,
    pub sensor_id: u8,
    pub x_axis: f64,
    pub y_axis: f64,
    pub z_axis: f64,
    pub rms: f64,
    pub peak: f64,
    pub crest_factor: f64,
    pub spindle_speed: f64,
}

#[derive(Row, Debug, Clone)]
pub struct TemperatureRow {
    pub timestamp: DateTime64,
    pub machine_id: u16,
    pub sensor_id: u8,
    pub value: f64,
    pub spindle_speed: f64,
}

#[derive(Row, Debug, Clone)]
pub struct DisplacementRow {
    pub timestamp: DateTime64,
    pub machine_id: u16,
    pub sensor_id: u8,
    pub axial: f64,
    pub radial: f64,
    pub spindle_speed: f64,
}

#[derive(Row, Debug, Clone)]
pub struct MachineStatusRow {
    pub timestamp: DateTime64,
    pub machine_id: u16,
    pub health_score: Option<f64>,
    pub rul_hours: Option<f64>,
    pub max_vibration_rms: Option<f64>,
    pub max_temperature: Option<f64>,
    pub alarm_status: Option<u8>,
    pub avg_spindle_speed: f64,
}

#[derive(Row, Debug, Clone)]
pub struct RULPredictionRow {
    pub timestamp: DateTime64,
    pub machine_id: u16,
    pub rul_hours: f64,
    pub confidence: f64,
    pub avg_rms: f64,
    pub temp_rate: f64,
    pub bearing_life_hours: f64,
    pub model_version: String,
}

#[derive(Row, Debug, Clone)]
pub struct AlarmEventRow {
    pub id: String,
    pub timestamp: DateTime64,
    pub machine_id: u16,
    pub sensor_type: String,
    pub sensor_id: Option<u8>,
    pub level: u8,
    pub message: String,
    pub value: f64,
    pub threshold: f64,
    pub acknowledged: u8,
}

#[derive(Row, Debug, Clone)]
pub struct WorkOrderRow {
    pub id: String,
    pub machine_id: u16,
    pub created_at: DateTime64,
    pub rul_hours: f64,
    pub priority: String,
    pub description: String,
    pub status: String,
}

#[derive(Row, Debug, Clone)]
pub struct SensorHistoryPoint {
    pub timestamp: DateTime64,
    pub value: f64,
}

#[derive(Row, Debug, Clone)]
pub struct TimeSeriesRow {
    pub timestamp: DateTime64,
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}

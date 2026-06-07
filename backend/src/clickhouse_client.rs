use clickhouse_rs::{Client, Pool, Options, Compression};
use chrono::{DateTime, Utc, TimeZone};
use tracing::{info, error};
use uuid::Uuid;

use crate::config::Config;
use crate::models::{ProcessedMetrics, Alarm, RULPrediction, MachineStatus, HealthRanking, MonthlyStats, SensorHistory};

pub struct ClickHouseClient {
    pool: Pool,
    database: String,
}

impl ClickHouseClient {
    pub async fn new(config: &Config) -> anyhow::Result<Self> {
        let url = format!(
            "tcp://{}:{}@{}:{}/{}",
            config.clickhouse.username,
            config.clickhouse.password,
            config.clickhouse.host,
            config.clickhouse.port,
            config.clickhouse.database
        );

        let options = Options::new(url)
            .compression(Compression::None)
            .connection_timeout(std::time::Duration::from_secs(10));
        
        let pool = Pool::new(options);
        
        let mut client = pool.get_handle().await?;
        info!("Connected to ClickHouse successfully");
        
        client.execute(format!("USE {}", config.clickhouse.database)).await?;

        Ok(Self {
            pool,
            database: config.clickhouse.database.clone(),
        })
    }

    pub async fn insert_metrics(&self, metrics: &ProcessedMetrics) -> anyhow::Result<()> {
        let mut client = self.pool.get_handle().await?;
        
        let ts: DateTime<Utc> = metrics.timestamp;
        let ts_ms = ts.timestamp_millis();

        let vibration_blob: Vec<u8> = metrics.vibration.iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let temperature_blob: Vec<u8> = metrics.temperature.iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let displacement_blob: Vec<u8> = metrics.displacement.iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let rms_blob: Vec<u8> = metrics.vibration_rms.iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let peak_blob: Vec<u8> = metrics.vibration_peak.iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();

        let freq_blob: Vec<u8> = metrics.vibration_freq.iter()
            .flat_map(|arr| arr.iter().flat_map(|v| v.to_le_bytes()))
            .collect();

        let query = format!(
            "INSERT INTO machine_metrics \
             (timestamp, machine_id, spindle_id, vibration, temperature, displacement, rpm, \
              vibration_rms, vibration_peak, vibration_freq) \
             VALUES ({}, {}, {}, [{}], [{}], [{}], {}, [{}], [{}], [{}])",
            ts_ms,
            metrics.machine_id,
            metrics.spindle_id,
            metrics.vibration.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "),
            metrics.temperature.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "),
            metrics.displacement.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "),
            metrics.rpm,
            metrics.vibration_rms.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "),
            metrics.vibration_peak.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "),
            metrics.vibration_freq.iter()
                .map(|arr| format!("[{}]", arr.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ")))
                .collect::<Vec<_>>().join(", ")
        );

        client.execute(query).await?;
        Ok(())
    }

    pub async fn insert_alarm(&self, alarm: &Alarm) -> anyhow::Result<()> {
        let mut client = self.pool.get_handle().await?;
        let ts_ms = alarm.timestamp.timestamp_millis();

        let query = format!(
            "INSERT INTO alarms \
             (timestamp, machine_id, alarm_type, alarm_level, message, sensor_index, value, threshold) \
             VALUES ({}, {}, '{}', {}, '{}', {}, {}, {})",
            ts_ms,
            alarm.machine_id,
            alarm.alarm_type,
            alarm.alarm_level,
            alarm.message.replace('\'', "''"),
            alarm.sensor_index,
            alarm.value,
            alarm.threshold
        );

        client.execute(query).await?;
        Ok(())
    }

    pub async fn insert_rul_prediction(&self, pred: &RULPrediction) -> anyhow::Result<()> {
        let mut client = self.pool.get_handle().await?;
        let ts_ms = pred.timestamp.timestamp_millis();

        let query = format!(
            "INSERT INTO rul_history \
             (timestamp, machine_id, rul_hours, health_score, vibration_trend, temperature_trend, model_source) \
             VALUES ({}, {}, {}, {}, {}, {}, '{}')",
            ts_ms,
            pred.machine_id,
            pred.rul_hours,
            pred.health_score,
            pred.vibration_trend,
            pred.temperature_trend,
            pred.model_source
        );

        client.execute(query).await?;
        Ok(())
    }

    pub async fn update_machine_status(&self, status: &MachineStatus) -> anyhow::Result<()> {
        let mut client = self.pool.get_handle().await?;
        let ts_ms = status.last_update.timestamp_millis();

        let query = format!(
            "INSERT INTO machine_status \
             (machine_id, last_update, health_score, rul_hours, vibration_severity, avg_temperature, alarm_level, total_runtime_hours) \
             VALUES ({}, {}, {}, {}, [{}], [{}], {}, {})",
            status.machine_id,
            ts_ms,
            status.health_score,
            status.rul_hours,
            status.vibration_severity.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "),
            status.avg_temperature.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "),
            status.alarm_level,
            status.total_runtime_hours
        );

        client.execute(query).await?;
        Ok(())
    }

    pub async fn create_maintenance_order(&self, machine_id: u16, description: &str, estimated_rul: f64) -> anyhow::Result<Uuid> {
        let mut client = self.pool.get_handle().await?;
        let order_id = Uuid::new_v4();
        let now = Utc::now().timestamp_millis();

        let query = format!(
            "INSERT INTO maintenance_orders \
             (order_id, created_at, machine_id, order_type, priority, description, estimated_rul, status) \
             VALUES ('{}', {}, {}, 'bearing_replacement', 'high', '{}', {}, 'pending')",
            order_id,
            now,
            machine_id,
            description.replace('\'', "''"),
            estimated_rul
        );

        client.execute(query).await?;
        Ok(order_id)
    }

    pub async fn get_health_ranking(&self, limit: usize) -> anyhow::Result<Vec<HealthRanking>> {
        let mut client = self.pool.get_handle().await?;
        
        let query = format!(
            "SELECT machine_id, health_score, rul_hours, alarm_level \
             FROM machine_status \
             ORDER BY health_score DESC \
             LIMIT {}",
            limit
        );

        let mut cursor = client.query(query).await?;
        let mut rankings = Vec::new();
        let mut rank = 1u16;

        while let Some(row) = cursor.next().await? {
            let machine_id: u16 = row.get("machine_id")?;
            let health_score: f64 = row.get("health_score")?;
            let rul_hours: f64 = row.get("rul_hours")?;
            let alarm_level: u8 = row.get("alarm_level")?;

            rankings.push(HealthRanking {
                machine_id,
                health_score,
                rul_hours,
                alarm_level,
                rank,
            });
            rank += 1;
        }

        Ok(rankings)
    }

    pub async fn get_monthly_stats(&self, month: &str) -> anyhow::Result<Vec<MonthlyStats>> {
        let mut client = self.pool.get_handle().await?;

        let query = format!(
            "SELECT month, machine_id, total_runtime, vibration_alerts, \
                    temperature_alerts, maintenance_count, avg_health_score \
             FROM monthly_stats \
             WHERE month = '{}' \
             ORDER BY machine_id",
            month
        );

        let mut cursor = client.query(query).await?;
        let mut stats = Vec::new();

        while let Some(row) = cursor.next().await? {
            let month_val: chrono::NaiveDate = row.get("month")?;
            stats.push(MonthlyStats {
                month: month_val.format("%Y-%m").to_string(),
                machine_id: row.get("machine_id")?,
                total_runtime: row.get("total_runtime")?,
                vibration_alerts: row.get("vibration_alerts")?,
                temperature_alerts: row.get("temperature_alerts")?,
                maintenance_count: row.get("maintenance_count")?,
                avg_health_score: row.get("avg_health_score")?,
            });
        }

        Ok(stats)
    }

    pub async fn get_sensor_history(&self, machine_id: u16, sensor_index: usize, hours: u32) -> anyhow::Result<SensorHistory> {
        let mut client = self.pool.get_handle().await?;
        let now = Utc::now();
        let since = now - chrono::Duration::hours(hours as i64);
        let since_ms = since.timestamp_millis();

        let query = format!(
            "SELECT timestamp, vibration_rms[{}] as rms, vibration_freq \
             FROM machine_metrics \
             WHERE machine_id = {} AND timestamp >= {} \
             ORDER BY timestamp ASC",
            sensor_index + 1,
            machine_id,
            since_ms
        );

        let mut cursor = client.query(query).await?;
        let mut timestamps = Vec::new();
        let mut values = Vec::new();
        let mut spectrum = Vec::new();
        let mut frequencies = Vec::new();

        while let Some(row) = cursor.next().await? {
            let ts: i64 = row.get("timestamp")?;
            let rms: f64 = row.get("rms")?;
            
            timestamps.push(ts);
            values.push(rms);
        }

        if frequencies.is_empty() && !values.is_empty() {
            let sample_rate = 10.0;
            for i in 0..128 {
                frequencies.push(i as f64 * sample_rate / 128.0);
            }
        }

        Ok(SensorHistory {
            timestamps,
            values,
            frequencies,
            spectrum,
        })
    }

    pub async fn get_machine_status(&self, machine_id: u16) -> anyhow::Result<Option<MachineStatus>> {
        let mut client = self.pool.get_handle().await?;

        let query = format!(
            "SELECT machine_id, last_update, health_score, rul_hours, \
                    vibration_severity, avg_temperature, alarm_level, total_runtime_hours \
             FROM machine_status \
             WHERE machine_id = {} \
             ORDER BY last_update DESC \
             LIMIT 1",
            machine_id
        );

        let mut cursor = client.query(query).await?;
        
        if let Some(row) = cursor.next().await? {
            let ts: i64 = row.get("last_update")?;
            let last_update = Utc.timestamp_millis_opt(ts).single().unwrap_or_else(Utc::now);

            Ok(Some(MachineStatus {
                machine_id: row.get("machine_id")?,
                last_update,
                health_score: row.get("health_score")?,
                rul_hours: row.get("rul_hours")?,
                vibration_severity: row.get("vibration_severity")?,
                avg_temperature: row.get("avg_temperature")?,
                alarm_level: row.get("alarm_level")?,
                total_runtime_hours: row.get("total_runtime_hours")?,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_all_machine_statuses(&self) -> anyhow::Result<Vec<MachineStatus>> {
        let mut client = self.pool.get_handle().await?;

        let query = "\
            SELECT machine_id, max(last_update) as last_update, \
                   argMax(health_score, last_update) as health_score, \
                   argMax(rul_hours, last_update) as rul_hours, \
                   argMax(vibration_severity, last_update) as vibration_severity, \
                   argMax(avg_temperature, last_update) as avg_temperature, \
                   argMax(alarm_level, last_update) as alarm_level, \
                   argMax(total_runtime_hours, last_update) as total_runtime_hours \
            FROM machine_status \
            GROUP BY machine_id \
            ORDER BY machine_id".to_string();

        let mut cursor = client.query(query).await?;
        let mut statuses = Vec::new();

        while let Some(row) = cursor.next().await? {
            let ts: i64 = row.get("last_update")?;
            let last_update = Utc.timestamp_millis_opt(ts).single().unwrap_or_else(Utc::now);

            statuses.push(MachineStatus {
                machine_id: row.get("machine_id")?,
                last_update,
                health_score: row.get("health_score")?,
                rul_hours: row.get("rul_hours")?,
                vibration_severity: row.get("vibration_severity")?,
                avg_temperature: row.get("avg_temperature")?,
                alarm_level: row.get("alarm_level")?,
                total_runtime_hours: row.get("total_runtime_hours")?,
            });
        }

        Ok(statuses)
    }
}

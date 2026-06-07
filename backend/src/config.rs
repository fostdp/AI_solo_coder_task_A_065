use serde::Deserialize;
use std::fs;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub clickhouse: ClickHouseConfig,
    pub mqtt: MqttConfig,
    pub monitoring: MonitoringConfig,
    pub machines: MachinesConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub udp_port: u16,
    pub http_port: u16,
    pub websocket_port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClickHouseConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MqttConfig {
    pub broker: String,
    pub client_id: String,
    pub topic_alarm: String,
    pub topic_status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MonitoringConfig {
    pub vibration_warning: f64,
    pub vibration_alarm: f64,
    pub vibration_alarm_duration_sec: u64,
    pub rul_warning_threshold: f64,
    pub rul_alarm_threshold: f64,
    pub health_score_update_interval_sec: u64,
    pub rul_prediction_interval_sec: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MachinesConfig {
    pub count: u16,
    pub sensors_vibration: u8,
    pub sensors_temperature: u8,
    pub sensors_displacement: u8,
}

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)
            .unwrap_or_else(|_| fs::read_to_string("config.example.toml").expect("Config file not found"));
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}

use serde::Deserialize;
use std::fs;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub clickhouse: ClickHouseConfig,
    pub mqtt: MqttConfig,
    pub monitoring: MonitoringConfig,
    pub machines: MachinesConfig,
    pub models: ModelsConfig,
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

#[derive(Debug, Clone, Deserialize)]
pub struct ModelsConfig {
    pub skf: SkfModelConfig,
    pub lstm: LstmModelConfig,
    pub hybrid: HybridModelConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkfModelConfig {
    pub basic_rated_life_hours: f64,
    pub vibration_factor_low_vib_low: f64,
    pub vib_factor_medium_low: f64,
    pub vib_factor_high: f64,
    pub temp_factor_low: f64,
    pub temp_factor_medium: f64,
    pub temp_factor_high: f64,
    pub load_factor_coefficient: f64,
    pub max_wear_factor: f64,
    pub wear_rate_per_50k_hours: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LstmModelConfig {
    pub base_rul_hours: f64,
    pub vib_high_penalty_per_std: f64,
    pub vib_low_penalty_per_std: f64,
    pub temp_penalty_per_std: f64,
    pub temp_rate_penalty_per_deg_s: f64,
    pub load_penalty_coefficient: f64,
    pub high_rpm_threshold: f64,
    pub high_rpm_penalty: f64,
    pub noise_std_dev: f64,
    pub smoothing_factor: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HybridModelConfig {
    pub weights_low_speed: [f64; 3],
    pub weights_medium_speed: [f64; 3],
    pub weights_high_speed: [f64; 3],
    pub weights_unknown: [f64; 3],
}

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)
            .unwrap_or_else(|_| fs::read_to_string("config.example.toml").expect("Config file not found"));
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}

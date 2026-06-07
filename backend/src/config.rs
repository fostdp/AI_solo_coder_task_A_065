use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub udp_port: u16,
    pub api_port: u16,
    pub clickhouse_url: String,
    pub clickhouse_user: String,
    pub clickhouse_password: String,
    pub clickhouse_database: String,
    pub mqtt_broker: String,
    pub mqtt_port: u16,
    pub mqtt_topic_alarm: String,
    pub mqtt_topic_mes: String,
    pub vibration_warning_threshold: f64,
    pub vibration_alarm_threshold: f64,
    pub rul_warning_hours: f64,
    pub rul_alarm_hours: f64,
    pub alarm_duration_seconds: u64,
    pub machine_count: usize,
    pub vibration_sensors_per_machine: usize,
    pub temperature_sensors_per_machine: usize,
    pub displacement_sensors_per_machine: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            udp_port: env::var("UDP_PORT").unwrap_or_else(|_| "9876".to_string()).parse().unwrap_or(9876),
            api_port: env::var("API_PORT").unwrap_or_else(|_| "8080".to_string()).parse().unwrap_or(8080),
            clickhouse_url: env::var("CLICKHOUSE_URL").unwrap_or_else(|_| "http://localhost:8123".to_string()),
            clickhouse_user: env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "default".to_string()),
            clickhouse_password: env::var("CLICKHOUSE_PASSWORD").unwrap_or_else(|_| "".to_string()),
            clickhouse_database: env::var("CLICKHOUSE_DB").unwrap_or_else(|_| "spindle_monitor".to_string()),
            mqtt_broker: env::var("MQTT_BROKER").unwrap_or_else(|_| "localhost".to_string()),
            mqtt_port: env::var("MQTT_PORT").unwrap_or_else(|_| "1883".to_string()).parse().unwrap_or(1883),
            mqtt_topic_alarm: env::var("MQTT_TOPIC_ALARM").unwrap_or_else(|_| "workshop/alarm".to_string()),
            mqtt_topic_mes: env::var("MQTT_TOPIC_MES").unwrap_or_else(|_| "mes/spindle".to_string()),
            vibration_warning_threshold: 2.8,
            vibration_alarm_threshold: 7.1,
            rul_warning_hours: 500.0,
            rul_alarm_hours: 200.0,
            alarm_duration_seconds: 10,
            machine_count: 40,
            vibration_sensors_per_machine: 8,
            temperature_sensors_per_machine: 4,
            displacement_sensors_per_machine: 2,
        }
    }
}

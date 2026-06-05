use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub udp_port: u16,
    pub http_port: u16,
    pub clickhouse_url: String,
    pub clickhouse_user: String,
    pub clickhouse_password: String,
    pub clickhouse_database: String,
    pub mqtt_broker: String,
    pub mqtt_port: u16,
    pub mqtt_topic: String,
    pub vibration_warning_threshold: f32,
    pub vibration_critical_threshold: f32,
    pub rul_warning_threshold: f32,
    pub rul_critical_threshold: f32,
    pub vibration_alarm_duration_ms: u64,
}

impl Config {
    pub fn from_env() -> Self {
        Config {
            udp_port: env::var("UDP_PORT")
                .unwrap_or_else(|_| "9999".to_string())
                .parse()
                .expect("Invalid UDP_PORT"),
            http_port: env::var("HTTP_PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .expect("Invalid HTTP_PORT"),
            clickhouse_url: env::var("CLICKHOUSE_URL")
                .unwrap_or_else(|_| "http://localhost:8123".to_string()),
            clickhouse_user: env::var("CLICKHOUSE_USER")
                .unwrap_or_else(|_| "default".to_string()),
            clickhouse_password: env::var("CLICKHOUSE_PASSWORD")
                .unwrap_or_else(|_| "".to_string()),
            clickhouse_database: env::var("CLICKHOUSE_DATABASE")
                .unwrap_or_else(|_| "spindle_monitor".to_string()),
            mqtt_broker: env::var("MQTT_BROKER")
                .unwrap_or_else(|_| "localhost".to_string()),
            mqtt_port: env::var("MQTT_PORT")
                .unwrap_or_else(|_| "1883".to_string())
                .parse()
                .expect("Invalid MQTT_PORT"),
            mqtt_topic: env::var("MQTT_TOPIC")
                .unwrap_or_else(|_| "spindle/alarm".to_string()),
            vibration_warning_threshold: env::var("VIBRATION_WARNING_THRESHOLD")
                .unwrap_or_else(|_| "2.8".to_string())
                .parse()
                .expect("Invalid VIBRATION_WARNING_THRESHOLD"),
            vibration_critical_threshold: env::var("VIBRATION_CRITICAL_THRESHOLD")
                .unwrap_or_else(|_| "7.1".to_string())
                .parse()
                .expect("Invalid VIBRATION_CRITICAL_THRESHOLD"),
            rul_warning_threshold: env::var("RUL_WARNING_THRESHOLD")
                .unwrap_or_else(|_| "500.0".to_string())
                .parse()
                .expect("Invalid RUL_WARNING_THRESHOLD"),
            rul_critical_threshold: env::var("RUL_CRITICAL_THRESHOLD")
                .unwrap_or_else(|_| "200.0".to_string())
                .parse()
                .expect("Invalid RUL_CRITICAL_THRESHOLD"),
            vibration_alarm_duration_ms: env::var("VIBRATION_ALARM_DURATION_MS")
                .unwrap_or_else(|_| "10000".to_string())
                .parse()
                .expect("Invalid VIBRATION_ALARM_DURATION_MS"),
        }
    }
}

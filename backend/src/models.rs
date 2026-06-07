use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorData {
    pub timestamp: i64,
    pub machine_id: u16,
    pub spindle_id: u8,
    pub vibration: Vec<f64>,
    pub temperature: Vec<f64>,
    pub displacement: Vec<f64>,
    pub rpm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedMetrics {
    pub timestamp: DateTime<Utc>,
    pub machine_id: u16,
    pub spindle_id: u8,
    pub vibration: Vec<f64>,
    pub temperature: Vec<f64>,
    pub displacement: Vec<f64>,
    pub rpm: f64,
    pub vibration_rms: Vec<f64>,
    pub vibration_peak: Vec<f64>,
    pub vibration_freq: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineStatus {
    pub machine_id: u16,
    pub last_update: DateTime<Utc>,
    pub health_score: f64,
    pub rul_hours: f64,
    pub vibration_severity: Vec<f64>,
    pub avg_temperature: Vec<f64>,
    pub alarm_level: u8,
    pub total_runtime_hours: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alarm {
    pub timestamp: DateTime<Utc>,
    pub machine_id: u16,
    pub alarm_type: String,
    pub alarm_level: u8,
    pub message: String,
    pub sensor_index: u8,
    pub value: f64,
    pub threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RULPrediction {
    pub timestamp: DateTime<Utc>,
    pub machine_id: u16,
    pub rul_hours: f64,
    pub health_score: f64,
    pub vibration_trend: f64,
    pub temperature_trend: f64,
    pub model_source: String,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: crate::config::Config,
    pub machine_statuses: HashMap<u16, MachineStatus>,
    pub recent_metrics: HashMap<u16, Vec<ProcessedMetrics>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthRanking {
    pub machine_id: u16,
    pub health_score: f64,
    pub rul_hours: f64,
    pub alarm_level: u8,
    pub rank: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyStats {
    pub month: String,
    pub machine_id: u16,
    pub total_runtime: f64,
    pub vibration_alerts: u32,
    pub temperature_alerts: u32,
    pub maintenance_count: u32,
    pub avg_health_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorHistory {
    pub timestamps: Vec<i64>,
    pub values: Vec<f64>,
    pub frequencies: Vec<f64>,
    pub spectrum: Vec<Vec<f64>>,
}

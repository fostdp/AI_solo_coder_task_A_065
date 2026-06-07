use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperatingCondition {
    LowSpeed,
    MediumSpeed,
    HighSpeed,
    Unknown,
}

impl OperatingCondition {
    pub fn from_rpm(rpm: f64) -> Self {
        match rpm {
            r if r < 2000.0 => OperatingCondition::LowSpeed,
            r if r < 4000.0 => OperatingCondition::MediumSpeed,
            r if r >= 4000.0 => OperatingCondition::HighSpeed,
            _ => OperatingCondition::Unknown,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            OperatingCondition::LowSpeed => "low_speed",
            OperatingCondition::MediumSpeed => "medium_speed",
            OperatingCondition::HighSpeed => "high_speed",
            OperatingCondition::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationParams {
    pub mean: f64,
    pub std: f64,
    pub min: f64,
    pub max: f64,
}

impl NormalizationParams {
    pub fn new(mean: f64, std: f64, min: f64, max: f64) -> Self {
        Self { mean, std, min, max }
    }

    pub fn normalize_z_score(&self, value: f64) -> f64 {
        if self.std == 0.0 { 0.0 } else { (value - self.mean) / self.std }
    }

    pub fn normalize_minmax(&self, value: f64) -> f64 {
        let range = self.max - self.min;
        if range == 0.0 { 0.5 } else { (value - self.min) / range }
    }

    pub fn denormalize_z_score(&self, normalized: f64) -> f64 {
        normalized * self.std + self.mean
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionFeatures {
    pub condition: OperatingCondition,
    pub rpm_normalized: f64,
    pub load_estimate: f64,
    pub vibration_mean: f64,
    pub vibration_std: f64,
    pub temp_mean: f64,
    pub temp_rate: f64,
}

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

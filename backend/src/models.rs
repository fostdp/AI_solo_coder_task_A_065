use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorData {
    pub timestamp: DateTime<Utc>,
    pub machine_id: u16,
    pub spindle_speed: f64,
    pub vibration: Vec<VibrationReading>,
    pub temperature: Vec<TemperatureReading>,
    pub displacement: Vec<DisplacementReading>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibrationReading {
    pub sensor_id: u8,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub rms: f64,
    pub peak: f64,
    pub crest_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureReading {
    pub sensor_id: u8,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplacementReading {
    pub sensor_id: u8,
    pub axial: f64,
    pub radial: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineStatus {
    pub machine_id: u16,
    pub health_score: f64,
    pub rul_hours: f64,
    pub max_vibration_rms: f64,
    pub max_temperature: f64,
    pub alarm_status: AlarmLevel,
    pub last_update: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlarmLevel {
    Normal = 0,
    Warning = 1,
    Critical = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmEvent {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub machine_id: u16,
    pub sensor_type: String,
    pub sensor_id: Option<u8>,
    pub level: AlarmLevel,
    pub message: String,
    pub value: f64,
    pub threshold: f64,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RULPrediction {
    pub machine_id: u16,
    pub timestamp: DateTime<Utc>,
    pub rul_hours: f64,
    pub confidence: f64,
    pub avg_rms: f64,
    pub temp_rate: f64,
    pub bearing_life_hours: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkOrder {
    pub id: String,
    pub machine_id: u16,
    pub created_at: DateTime<Utc>,
    pub rul_hours: f64,
    pub priority: String,
    pub description: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyStats {
    pub month: String,
    pub total_alarms: u32,
    pub critical_alarms: u32,
    pub warning_alarms: u32,
    pub avg_health_score: f64,
    pub machines_maintained: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorPosition {
    pub id: u8,
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub location: String,
}

impl VibrationReading {
    pub fn severity_level(&self, warning: f64, alarm: f64) -> AlarmLevel {
        if self.rms >= alarm {
            AlarmLevel::Critical
        } else if self.rms >= warning {
            AlarmLevel::Warning
        } else {
            AlarmLevel::Normal
        }
    }
}

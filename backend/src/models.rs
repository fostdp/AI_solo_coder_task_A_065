use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorData {
    pub timestamp: i64,
    pub machine_id: u16,
    pub sensor_id: u16,
    pub sensor_type: SensorType,
    pub value: f32,
    pub spindle_speed: f32,
    pub load: f32,
    pub temperature: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SensorType {
    Vibration = 1,
    Temperature = 2,
    Displacement = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedSensorData {
    pub timestamp: i64,
    pub machine_id: u16,
    pub sensor_id: u16,
    pub sensor_type: SensorType,
    pub value_min: f32,
    pub value_max: f32,
    pub value_avg: f32,
    pub value_rms: f32,
    pub value_std: f32,
    pub value_peak: f32,
    pub spindle_speed_avg: f32,
    pub load_avg: f32,
    pub temperature_avg: f32,
    pub sample_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineInfo {
    pub machine_id: u16,
    pub machine_name: String,
    pub model: String,
    pub install_date: String,
    pub location: String,
    pub operator: String,
    pub status: MachineStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MachineStatus {
    Running = 1,
    Idle = 2,
    Maintenance = 3,
    Fault = 4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorConfig {
    pub sensor_id: u16,
    pub machine_id: u16,
    pub sensor_type: SensorType,
    pub position_name: String,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub axis: String,
    pub unit: String,
    pub status: SensorStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SensorStatus {
    Active = 1,
    Inactive = 2,
    Fault = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RULPrediction {
    pub timestamp: i64,
    pub machine_id: u16,
    pub bearing_id: u8,
    pub rul_hours: f32,
    pub rul_confidence: f32,
    pub vibration_rms_trend: f32,
    pub temperature_rate: f32,
    pub skf_l10_life: f32,
    pub lstm_prediction: f32,
    pub health_score: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alarm {
    pub alarm_id: Uuid,
    pub timestamp: i64,
    pub machine_id: u16,
    pub sensor_id: u16,
    pub alarm_level: AlarmLevel,
    pub alarm_type: AlarmType,
    pub alarm_message: String,
    pub value: f32,
    pub threshold: f32,
    pub duration_ms: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlarmLevel {
    Info = 0,
    Warning = 1,
    Critical = 2,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlarmType {
    VibrationHigh = 1,
    TemperatureHigh = 2,
    DisplacementAbnormal = 3,
    RULLow = 4,
    SensorFault = 5,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthScore {
    pub timestamp: i64,
    pub machine_id: u16,
    pub overall_score: u8,
    pub vibration_score: u8,
    pub temperature_score: u8,
    pub displacement_score: u8,
    pub rul_score: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibrationSpectrum {
    pub timestamp: i64,
    pub machine_id: u16,
    pub sensor_id: u16,
    pub frequency: Vec<f32>,
    pub amplitude: Vec<f32>,
    pub rpm: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkOrder {
    pub work_order_id: Uuid,
    pub machine_id: u16,
    pub work_order_type: WorkOrderType,
    pub priority: Priority,
    pub title: String,
    pub description: String,
    pub status: WorkOrderStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkOrderType {
    Preventive = 1,
    Corrective = 2,
    Predictive = 3,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Priority {
    Low = 1,
    Medium = 2,
    High = 3,
    Urgent = 4,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkOrderStatus {
    Open = 1,
    InProgress = 2,
    Completed = 3,
    Cancelled = 4,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthRankingItem {
    pub machine_id: u16,
    pub machine_name: String,
    pub overall_score: u8,
    pub rul_hours: f32,
    pub location: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FaultStatistics {
    pub month: String,
    pub total_alarms: u64,
    pub vibration_alarms: u64,
    pub temperature_alarms: u64,
    pub rul_alarms: u64,
    pub work_orders_created: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SensorDetailResponse {
    pub sensor_config: SensorConfig,
    pub recent_data: Vec<TimeSeriesPoint>,
    pub spectrum: VibrationSpectrum,
    pub history_trend: Vec<TimeSeriesPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub timestamp: i64,
    pub value: f32,
}

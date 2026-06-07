use chrono::{DateTime, Utc, SecondsFormat};
use serde::{Serialize, Deserialize};
use serde_json::json;
use uuid::Uuid;

use crate::models::{Alarm, MachineStatus, ConditionFeatures};

const ISO22400_VERSION: &str = "2.0";
const SOURCE_SYSTEM: &str = "CNC_Spindle_Health_Monitor";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ISO22400Message {
    #[serde(rename = "MessageHeader")]
    pub header: MessageHeader,
    #[serde(rename = "MessageBody")]
    pub body: MessageBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHeader {
    #[serde(rename = "MessageID")]
    pub message_id: String,
    #[serde(rename = "MessageType")]
    pub message_type: String,
    #[serde(rename = "SourceSystem")]
    pub source_system: String,
    #[serde(rename = "Timestamp")]
    pub timestamp: String,
    #[serde(rename = "Version")]
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageBody {
    Alarm(AlarmMessageBody),
    Kpi(KpiMessageBody),
    Status(StatusMessageBody),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmMessageBody {
    #[serde(rename = "Alarm")]
    pub alarm: ISO22400Alarm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ISO22400Alarm {
    #[serde(rename = "AlarmID")]
    pub alarm_id: String,
    #[serde(rename = "AlarmType")]
    pub alarm_type: String,
    #[serde(rename = "AlarmClass")]
    pub alarm_class: String,
    #[serde(rename = "Severity")]
    pub severity: u8,
    #[serde(rename = "EquipmentID")]
    pub equipment_id: String,
    #[serde(rename = "EquipmentType")]
    pub equipment_type: String,
    #[serde(rename = "Description")]
    pub description: String,
    #[serde(rename = "AlarmTime")]
    pub alarm_time: String,
    #[serde(rename = "Condition")]
    pub condition: Option<String>,
    #[serde(rename = "ActualValue")]
    pub actual_value: Option<f64>,
    #[serde(rename = "ThresholdValue")]
    pub threshold_value: Option<f64>,
    #[serde(rename = "Unit")]
    pub unit: Option<String>,
    #[serde(rename = "ACKState")]
    pub ack_state: String,
    #[serde(rename = "ActiveState")]
    pub active_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KpiMessageBody {
    #[serde(rename = "KPICollection")]
    pub kpi_collection: KpiCollection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KpiCollection {
    #[serde(rename = "EquipmentID")]
    pub equipment_id: String,
    #[serde(rename = "TimePeriodStart")]
    pub time_period_start: String,
    #[serde(rename = "TimePeriodEnd")]
    pub time_period_end: String,
    #[serde(rename = "KPI")]
    pub kpis: Vec<ISO22400KPI>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ISO22400KPI {
    #[serde(rename = "KPIID")]
    pub kpi_id: String,
    #[serde(rename = "KPIName")]
    pub kpi_name: String,
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "Description")]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusMessageBody {
    #[serde(rename = "EquipmentStatus")]
    pub equipment_status: EquipmentStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquipmentStatus {
    #[serde(rename = "EquipmentID")]
    pub equipment_id: String,
    #[serde(rename = "EquipmentType")]
    pub equipment_type: String,
    #[serde(rename = "Status")]
    pub status: String,
    #[serde(rename = "StatusTime")]
    pub status_time: String,
    #[serde(rename = "HealthScore")]
    pub health_score: Option<f64>,
    #[serde(rename = "RULHours")]
    pub rul_hours: Option<f64>,
    #[serde(rename = "OperatingCondition")]
    pub operating_condition: Option<String>,
}

pub struct ISO22400Adapter;

impl ISO22400Adapter {
    pub fn new() -> Self {
        Self
    }

    fn create_header(message_type: &str) -> MessageHeader {
        MessageHeader {
            message_id: Uuid::new_v4().to_string(),
            message_type: message_type.to_string(),
            source_system: SOURCE_SYSTEM.to_string(),
            timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            version: ISO22400_VERSION.to_string(),
        }
    }

    pub fn alarm_to_iso22400(&self, alarm: &Alarm, condition: Option<&str>) -> ISO22400Message {
        let (severity, alarm_class) = match alarm.alarm_level {
            2 => (2, "CRITICAL".to_string()),
            1 => (4, "WARNING".to_string()),
            _ => (6, "INFORMATION".to_string()),
        };

        let unit = match alarm.alarm_type.as_str() {
            t if t.contains("vibration") => Some("mm/s".to_string()),
            t if t.contains("rul") => Some("hours".to_string()),
            _ => None,
        };

        let alarm_type_iso = match alarm.alarm_type.as_str() {
            "vibration_severe" => "EquipmentCondition.VibrationExceeded".to_string(),
            "rul_critical" => "EquipmentCondition.RULCritical".to_string(),
            other => format!("EquipmentCondition.{}", other),
        };

        let iso_alarm = ISO22400Alarm {
            alarm_id: format!("ALM-{}-{}-{}", alarm.machine_id, alarm.alarm_level, alarm.timestamp.timestamp()),
            alarm_type: alarm_type_iso,
            alarm_class,
            severity,
            equipment_id: format!("CNC-{:02}", alarm.machine_id),
            equipment_type: "CNC_Machine_Spindle".to_string(),
            description: alarm.message.clone(),
            alarm_time: alarm.timestamp.to_rfc3339_opts(SecondsFormat::Secs, true),
            condition: condition.map(|c| c.to_string()),
            actual_value: Some(alarm.value),
            threshold_value: Some(alarm.threshold),
            unit,
            ack_state: "UNACKNOWLEDGED".to_string(),
            active_state: "ACTIVE".to_string(),
        };

        ISO22400Message {
            header: Self::create_header("Alarm"),
            body: MessageBody::Alarm(AlarmMessageBody { alarm: iso_alarm }),
        }
    }

    pub fn status_to_iso22400(&self, status: &MachineStatus, 
                             features: Option<&ConditionFeatures>) -> ISO22400Message {
        let status_str = match status.alarm_level {
            2 => "FAULT".to_string(),
            1 => "WARNING".to_string(),
            _ => "RUNNING".to_string(),
        };

        let op_condition = features.map(|f| f.condition.label().to_string());

        let equip_status = EquipmentStatus {
            equipment_id: format!("CNC-{:02}", status.machine_id),
            equipment_type: "CNC_Machine_Spindle".to_string(),
            status: status_str,
            status_time: status.last_update.to_rfc3339_opts(SecondsFormat::Secs, true),
            health_score: Some(status.health_score),
            rul_hours: Some(status.rul_hours),
            operating_condition: op_condition,
        };

        ISO22400Message {
            header: Self::create_header("EquipmentStatus"),
            body: MessageBody::Status(StatusMessageBody { equipment_status: equip_status }),
        }
    }

    pub fn create_rul_kpi(&self, machine_id: u16, rul_hours: f64, 
                         health_score: f64, start_time: DateTime<Utc>) -> ISO22400Message {
        let kpis = vec![
            ISO22400KPI {
                kpi_id: "RUL".to_string(),
                kpi_name: "RemainingUsefulLife".to_string(),
                value: rul_hours,
                unit: "hours".to_string(),
                description: "Estimated remaining useful life of spindle bearing".to_string(),
            },
            ISO22400KPI {
                kpi_id: "HealthScore".to_string(),
                kpi_name: "EquipmentHealthScore".to_string(),
                value: health_score,
                unit: "points".to_string(),
                description: "Overall health score of the spindle (0-100)".to_string(),
            },
        ];

        let collection = KpiCollection {
            equipment_id: format!("CNC-{:02}", machine_id),
            time_period_start: start_time.to_rfc3339_opts(SecondsFormat::Secs, true),
            time_period_end: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            kpis,
        };

        ISO22400Message {
            header: Self::create_header("KPI"),
            body: MessageBody::Kpi(KpiMessageBody { kpi_collection: collection }),
        }
    }

    pub fn to_json(&self, msg: &ISO22400Message) -> anyhow::Result<String> {
        serde_json::to_string(msg).map_err(|e| anyhow::anyhow!("Failed to serialize ISO22400 message: {}", e))
    }

    pub fn to_pretty_json(&self, msg: &ISO22400Message) -> anyhow::Result<String> {
        serde_json::to_string_pretty(msg).map_err(|e| anyhow::anyhow!("Failed to serialize ISO22400 message: {}", e))
    }
}

impl Default for ISO22400Adapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_alarm_conversion() {
        let adapter = ISO22400Adapter::new();
        let alarm = Alarm {
            timestamp: Utc::now(),
            machine_id: 1,
            alarm_type: "vibration_severe".to_string(),
            alarm_level: 1,
            message: "测试振动告警".to_string(),
            sensor_index: 0,
            value: 8.5,
            threshold: 7.1,
        };

        let iso_msg = adapter.alarm_to_iso22400(&alarm, Some("medium_speed"));
        
        assert_eq!(iso_msg.header.message_type, "Alarm");
        assert_eq!(iso_msg.header.source_system, SOURCE_SYSTEM);
        
        if let MessageBody::Alarm(body) = &iso_msg.body {
            assert_eq!(body.alarm.severity, 4);
            assert_eq!(body.alarm.alarm_class, "WARNING");
            assert_eq!(body.alarm.equipment_id, "CNC-01");
            assert_eq!(body.alarm.unit.as_deref(), Some("mm/s"));
        } else {
            panic!("Expected Alarm message body");
        }

        let json = adapter.to_json(&iso_msg).unwrap();
        assert!(json.contains("\"AlarmType\":\"EquipmentCondition.VibrationExceeded\""));
    }

    #[test]
    fn test_rul_kpi_conversion() {
        let adapter = ISO22400Adapter::new();
        let start_time = Utc::now();
        let iso_msg = adapter.create_rul_kpi(5, 3500.5, 85.2, start_time);
        
        assert_eq!(iso_msg.header.message_type, "KPI");
        
        if let MessageBody::Kpi(body) = &iso_msg.body {
            assert_eq!(body.kpi_collection.equipment_id, "CNC-05");
            assert_eq!(body.kpi_collection.kpis.len(), 2);
            
            let rul_kpi = &body.kpi_collection.kpis[0];
            assert_eq!(rul_kpi.kpi_id, "RUL");
            assert_eq!(rul_kpi.value, 3500.5);
            assert_eq!(rul_kpi.unit, "hours");
        }
    }
}

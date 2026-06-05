use rumqttc::{AsyncClient, MqttOptions, QoS, Event, Packet};
use crate::config::Config;
use crate::models::Alarm;
use serde::{Serialize, Deserialize};
use serde_json;
use std::time::Duration;
use log::{info, error, debug};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ISO22400Header {
    pub version: String,
    pub message_id: String,
    pub timestamp: String,
    pub source: String,
    pub destination: String,
    pub message_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ISO22400EquipmentID {
    pub equipment_id: String,
    pub equipment_name: String,
    pub equipment_type: String,
    pub line_id: Option<String>,
    pub plant_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ISO22400AlarmData {
    pub alarm_id: String,
    pub alarm_code: String,
    pub alarm_type: String,
    pub severity: String,
    pub alarm_text: String,
    pub timestamp: String,
    pub equipment: ISO22400EquipmentID,
    pub component_id: Option<String>,
    pub component_type: Option<String>,
    pub measured_value: Option<f64>,
    pub threshold_value: Option<f64>,
    pub unit: Option<String>,
    pub duration_ms: Option<u64>,
    pub status: String,
    pub ack_required: bool,
    pub operator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ISO22400Envelope {
    pub header: ISO22400Header,
    pub body: ISO22400AlarmData,
}

pub enum MESMessageFormat {
    Custom,
    ISO22400,
}

pub struct MQTTClient {
    client: AsyncClient,
    topic: String,
    mes_topic: Option<String>,
    format: MESMessageFormat,
    source_id: String,
}

impl MQTTClient {
    pub async fn new(config: &Config) -> Option<Self> {
        let mut mqtt_options = MqttOptions::new(
            "spindle-monitor-backend",
            &config.mqtt_broker,
            config.mqtt_port,
        );
        mqtt_options.set_keep_alive(Duration::from_secs(30));

        let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);

        let topic = config.mqtt_topic.clone();
        let mes_topic = Some(format!("{}/mes", topic));
        let format = MESMessageFormat::ISO22400;
        let source_id = "SPINDLE_MONITOR_SYSTEM_001".to_string();

        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(Event::Incoming(Packet::Publish(publish))) => {
                        info!("Received MQTT message: {:?}", publish);
                    }
                    Ok(Event::Incoming(Packet::ConnAck(_))) => {
                        info!("MQTT connected");
                    }
                    Ok(_) => {}
                    Err(e) => {
                        error!("MQTT error: {}", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        });

        Some(MQTTClient {
            client,
            topic,
            mes_topic,
            format,
            source_id,
        })
    }

    pub async fn publish_alarm(&self, alarm: &Alarm) -> anyhow::Result<()> {
        let payload = serde_json::to_string(alarm)?;
        self.client
            .publish(&self.topic, QoS::AtLeastOnce, false, payload.clone())
            .await?;

        if let Some(mes_topic) = &self.mes_topic {
            let mes_payload = match self.format {
                MESMessageFormat::Custom => payload,
                MESMessageFormat::ISO22400 => {
                    let iso_msg = Self::convert_to_iso22400(alarm, &self.source_id);
                    serde_json::to_string(&iso_msg)?
                }
            };

            self.client
                .publish(mes_topic, QoS::AtLeastOnce, false, mes_payload)
                .await?;

            debug!("Published alarm to MES topic: {}", mes_topic);
        }

        Ok(())
    }

    pub fn convert_to_iso22400(alarm: &Alarm, source_id: &str) -> ISO22400Envelope {
        let now = chrono::Utc::now();
        let message_id = format!("ALM-{}", alarm.alarm_id);

        let severity = match alarm.alarm_level {
            crate::models::AlarmLevel::Critical => "CRITICAL",
            crate::models::AlarmLevel::Warning => "WARNING",
            crate::models::AlarmLevel::Info => "INFO",
        };

        let alarm_type_code = match alarm.alarm_type {
            crate::models::AlarmType::VibrationHigh => "VIB_HIGH",
            crate::models::AlarmType::TemperatureHigh => "TEMP_HIGH",
            crate::models::AlarmType::DisplacementAbnormal => "DISP_ABNORM",
            crate::models::AlarmType::RULLow => "RUL_LOW",
            crate::models::AlarmType::SensorFault => "SENS_FAULT",
        };

        let unit = match alarm.alarm_type {
            crate::models::AlarmType::VibrationHigh => Some("mm/s".to_string()),
            crate::models::AlarmType::TemperatureHigh => Some("°C".to_string()),
            crate::models::AlarmType::DisplacementAbnormal => Some("mm".to_string()),
            crate::models::AlarmType::RULLow => Some("hours".to_string()),
            _ => None,
        };

        let component_type = match alarm.alarm_type {
            crate::models::AlarmType::VibrationHigh => Some("BEARING".to_string()),
            crate::models::AlarmType::TemperatureHigh => Some("MOTOR".to_string()),
            crate::models::AlarmType::DisplacementAbnormal => Some("SPINDLE".to_string()),
            crate::models::AlarmType::RULLow => Some("BEARING".to_string()),
            _ => None,
        };

        let line_id = format!("Line-{}", (alarm.machine_id - 1) / 10 + 1);

        ISO22400Envelope {
            header: ISO22400Header {
                version: "2.0".to_string(),
                message_id,
                timestamp: now.to_rfc3339(),
                source: source_id.to_string(),
                destination: "MES_SYSTEM_001".to_string(),
                message_type: "ALARM_NOTIFICATION".to_string(),
            },
            body: ISO22400AlarmData {
                alarm_id: alarm.alarm_id.to_string(),
                alarm_code: format!("{:04}", alarm.alarm_type as u16 * 100 + alarm.machine_id),
                alarm_type: alarm_type_code.to_string(),
                severity: severity.to_string(),
                alarm_text: alarm.alarm_message.clone(),
                timestamp: chrono::NaiveDateTime::from_timestamp_opt(alarm.timestamp, 0)
                    .map(|dt| dt.and_utc().to_rfc3339())
                    .unwrap_or_else(|| now.to_rfc3339()),
                equipment: ISO22400EquipmentID {
                    equipment_id: format!("CNC-{:03}", alarm.machine_id),
                    equipment_name: format!("CNC-{}", alarm.machine_id),
                    equipment_type: "5_AXIS_CNC_MACHINE".to_string(),
                    line_id: Some(line_id),
                    plant_id: Some("AERO_PARTS_PLANT_01".to_string()),
                },
                component_id: if alarm.sensor_id > 0 {
                    Some(format!("SENSOR-{:03}", alarm.sensor_id))
                } else {
                    None
                },
                component_type,
                measured_value: Some(alarm.value as f64),
                threshold_value: Some(alarm.threshold as f64),
                unit,
                duration_ms: Some(alarm.duration_ms as u64),
                status: "ACTIVE".to_string(),
                ack_required: true,
                operator: None,
            },
        }
    }

    pub async fn publish_raw(&self, topic: &str, payload: &str) -> anyhow::Result<()> {
        self.client
            .publish(topic, QoS::AtLeastOnce, false, payload)
            .await?;
        Ok(())
    }

    pub async fn publish_iso22400(&self, iso_msg: &ISO22400Envelope) -> anyhow::Result<()> {
        let payload = serde_json::to_string(iso_msg)?;
        let topic = self.mes_topic.as_deref().unwrap_or(&self.topic);
        self.client
            .publish(topic, QoS::AtLeastOnce, false, payload)
            .await?;
        Ok(())
    }
}

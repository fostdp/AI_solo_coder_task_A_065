use rumqttc::{AsyncClient, MqttOptions, QoS, Event, Packet};
use crate::config::Config;
use crate::models::Alarm;
use serde_json;
use std::time::Duration;
use log::{info, error};

pub struct MQTTClient {
    client: AsyncClient,
    topic: String,
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
        
        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(Event::Incoming(Packet::Publish(publish))) => {
                        info!("Received MQTT message: {:?}", publish);
                    }
                    Ok(_) => {}
                    Err(e) => {
                        error!("MQTT error: {}", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        });

        Some(MQTTClient { client, topic })
    }

    pub async fn publish_alarm(&self, alarm: &Alarm) -> anyhow::Result<()> {
        let payload = serde_json::to_string(alarm)?;
        self.client
            .publish(&self.topic, QoS::AtLeastOnce, false, payload)
            .await?;
        Ok(())
    }

    pub async fn publish_raw(&self, topic: &str, payload: &str) -> anyhow::Result<()> {
        self.client
            .publish(topic, QoS::AtLeastOnce, false, payload)
            .await?;
        Ok(())
    }
}

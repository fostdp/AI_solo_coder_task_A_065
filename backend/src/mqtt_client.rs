use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error, warn};
use paho_mqtt as mqtt;

use crate::config::Config;

pub struct MqttClient {
    client: Option<Arc<Mutex<mqtt::AsyncClient>>>,
    config: Config,
}

impl MqttClient {
    pub async fn new(config: &Config) -> anyhow::Result<Self> {
        let create_opts = mqtt::CreateOptionsBuilder::new()
            .server_uri(&config.mqtt.broker)
            .client_id(&config.mqtt.client_id)
            .finalize();

        let client = match mqtt::AsyncClient::new(create_opts) {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to create MQTT client: {}", e);
                return Ok(Self {
                    client: None,
                    config: config.clone(),
                });
            }
        };

        let conn_opts = mqtt::ConnectOptionsBuilder::new()
            .keep_alive_interval(std::time::Duration::from_secs(30))
            .clean_session(true)
            .finalize();

        let client_arc = Arc::new(Mutex::new(client));
        
        {
            let client_lock = client_arc.lock().await;
            if let Err(e) = client_lock.connect(conn_opts).await {
                warn!("Failed to connect to MQTT broker: {}", e);
                return Ok(Self {
                    client: None,
                    config: config.clone(),
                });
            }
        }

        info!("MQTT client connected to {}", config.mqtt.broker);

        Ok(Self {
            client: Some(client_arc),
            config: config.clone(),
        })
    }

    pub async fn publish_alarm(&self, payload: &str) -> anyhow::Result<()> {
        self.publish(&self.config.mqtt.topic_alarm, payload).await
    }

    pub async fn publish_status(&self, payload: &str) -> anyhow::Result<()> {
        self.publish(&self.config.mqtt.topic_status, payload).await
    }

    async fn publish(&self, topic: &str, payload: &str) -> anyhow::Result<()> {
        let client = match &self.client {
            Some(c) => c,
            None => return Ok(()),
        };

        let msg = mqtt::Message::new(topic, payload, 1);
        
        let client_lock = client.lock().await;
        if let Err(e) = client_lock.publish(msg).await {
            error!("Failed to publish MQTT message to {}: {}", topic, e);
        }

        Ok(())
    }
}

impl Drop for MqttClient {
    fn drop(&mut self) {
        if let Some(client) = &self.client {
            let client_clone = client.clone();
            tokio::spawn(async move {
                let client_lock = client_clone.lock().await;
                let _ = client_lock.disconnect(None).await;
            });
        }
    }
}

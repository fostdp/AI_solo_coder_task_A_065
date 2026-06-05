use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use crate::config::Config;
use crate::models::SensorData;
use crate::clickhouse_client::ClickHouseClient;
use crate::prediction::PredictionEngine;
use crate::alarm::AlarmManager;
use log::{info, error, debug};
use std::collections::HashMap;

pub struct UDPServer {
    config: Config,
    clickhouse: ClickHouseClient,
    prediction_engine: Arc<PredictionEngine>,
    alarm_manager: Arc<AlarmManager>,
    buffer: Arc<Mutex<Vec<SensorData>>>,
}

impl UDPServer {
    pub fn new(
        config: Config,
        clickhouse: ClickHouseClient,
        prediction_engine: Arc<PredictionEngine>,
        alarm_manager: Arc<AlarmManager>,
    ) -> Self {
        UDPServer {
            config,
            clickhouse,
            prediction_engine,
            alarm_manager,
            buffer: Arc::new(Mutex::new(Vec::with_capacity(1000))),
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let addr = format!("0.0.0.0:{}", self.config.udp_port);
        let socket = UdpSocket::bind(&addr).await?;
        info!("UDP server listening on {}", addr);

        let socket = Arc::new(socket);
        let mut buf = vec![0u8; 65536];

        let buffer_clone = self.buffer.clone();
        let clickhouse_clone = self.clickhouse.clone();
        let prediction_engine_clone = self.prediction_engine.clone();
        let alarm_manager_clone = self.alarm_manager.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                let mut buffer = buffer_clone.lock().await;
                if buffer.len() > 0 {
                    let data = std::mem::replace(&mut *buffer, Vec::with_capacity(1000));
                    if let Err(e) = clickhouse_clone.insert_sensor_data(&data).await {
                        error!("Failed to insert sensor data: {}", e);
                    }
                    debug!("Inserted {} sensor data points", data.len());
                }
            }
        });

        loop {
            let (len, src) = socket.recv_from(&mut buf).await?;
            debug!("Received {} bytes from {}", len, src);

            match parse_ethercat_packet(&buf[..len]) {
                Ok(sensor_data_batch) => {
                    let mut buffer = self.buffer.lock().await;
                    
                    for data in &sensor_data_batch {
                        self.alarm_manager.process_sensor_data(data).await;
                    }
                    
                    let machine_ids: std::collections::HashSet<u16> = sensor_data_batch
                        .iter()
                        .map(|d| d.machine_id)
                        .collect();
                    
                    for machine_id in machine_ids {
                        let machine_data: Vec<&SensorData> = sensor_data_batch
                            .iter()
                            .filter(|d| d.machine_id == machine_id)
                            .collect();
                        
                        if !machine_data.is_empty() {
                            let prediction_engine = self.prediction_engine.clone();
                            let clickhouse = self.clickhouse.clone();
                            let alarm_manager = self.alarm_manager.clone();
                            
                            tokio::spawn(async move {
                                if let Ok(Some(rul)) = prediction_engine.predict_rul(machine_id, &machine_data).await {
                                    if let Err(e) = clickhouse.insert_rul_prediction(&rul).await {
                                        error!("Failed to insert RUL prediction: {}", e);
                                    }
                                    
                                    alarm_manager.check_rul_alarm(machine_id, rul.rul_hours).await;
                                    
                                    if let Ok(score) = prediction_engine.calculate_health_score(machine_id, &machine_data, &rul) {
                                        if let Err(e) = clickhouse.insert_health_score(&score).await {
                                            error!("Failed to insert health score: {}", e);
                                        }
                                    }
                                }
                            });
                        }
                    }
                    
                    buffer.extend(sensor_data_batch);
                }
                Err(e) => {
                    error!("Failed to parse EtherCAT packet: {}", e);
                }
            }
        }
    }
}

fn parse_ethercat_packet(data: &[u8]) -> anyhow::Result<Vec<SensorData>> {
    if data.len() < 8 {
        anyhow::bail!("Packet too small");
    }

    let num_sensors = u16::from_le_bytes([data[0], data[1]]) as usize;
    let machine_id = u16::from_le_bytes([data[2], data[3]]);
    let timestamp = i64::from_le_bytes([
        data[4], data[5], data[6], data[7],
        data[8], data[9], data[10], data[11],
    ]);

    let mut offset = 12;
    let mut result = Vec::with_capacity(num_sensors);

    for _ in 0..num_sensors {
        if offset + 24 > data.len() {
            break;
        }

        let sensor_id = u16::from_le_bytes([data[offset], data[offset + 1]]);
        let sensor_type = match data[offset + 2] {
            1 => crate::models::SensorType::Vibration,
            2 => crate::models::SensorType::Temperature,
            _ => crate::models::SensorType::Displacement,
        };
        let value = f32::from_le_bytes([data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]]);
        let spindle_speed = f32::from_le_bytes([data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11]]);
        let load = f32::from_le_bytes([data[offset + 12], data[offset + 13], data[offset + 14], data[offset + 15]]);
        let temperature = f32::from_le_bytes([data[offset + 16], data[offset + 17], data[offset + 18], data[offset + 19]]);

        result.push(SensorData {
            timestamp,
            machine_id,
            sensor_id,
            sensor_type,
            value,
            spindle_speed,
            load,
            temperature,
        });

        offset += 24;
    }

    Ok(result)
}

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};
use tokio::net::UdpSocket;
use tracing::{info, error, warn, debug};
use byteorder::{LittleEndian, ReadBytesExt};

use crate::config::Config;
use crate::models::{SensorData, TimeDomainFeatures};

const MAX_UDP_SIZE: usize = 65536;
const CHANNEL_CAPACITY: usize = 2048;

pub struct EthercatDriver {
    config: Config,
    raw_data_tx: mpsc::Sender<SensorData>,
}

impl EthercatDriver {
    pub fn new(config: &Config) -> (Self, mpsc::Receiver<SensorData>) {
        let (tx, rx) = mpsc::channel(CHANNEL_CAPACITY);
        (
            Self {
                config: config.clone(),
                raw_data_tx: tx,
            },
            rx,
        )
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let socket = Arc::new(UdpSocket::bind(format!("0.0.0.0:{}", self.config.server.udp_port)).await?);
        
        socket.set_recv_buffer_size(8 * 1024 * 1024)?;
        info!(
            "EtherCAT Driver: UDP server listening on 0.0.0.0:{}, recv_buffer=8MB",
            self.config.server.udp_port
        );

        let mut packet_count = 0u64;
        let mut drop_count = 0u64;
        let mut last_stats = tokio::time::interval(Duration::from_secs(10));

        let mut buf = vec![0u8; MAX_UDP_SIZE];
        let tx = self.raw_data_tx.clone();

        loop {
            tokio::select! {
                result = socket.recv_from(&mut buf) => {
                    match result {
                        Ok((len, _src)) => {
                            packet_count += 1;
                            match parse_udp_packet(&buf[..len]) {
                                Ok(sensor_data) => {
                                    match tx.try_send(sensor_data) {
                                        Ok(_) => {}
                                        Err(mpsc::error::TrySendError::Full(_)) => {
                                            drop_count += 1;
                                            if drop_count % 1000 == 0 {
                                                warn!("EtherCAT Driver: Channel full, dropped {} packets", drop_count);
                                            }
                                        }
                                        Err(e) => {
                                            error!("EtherCAT Driver: Send error: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("EtherCAT Driver: Failed to parse UDP packet: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("EtherCAT Driver: UDP recv error: {}", e);
                            tokio::time::sleep(Duration::from_millis(1)).await;
                        }
                    }
                }
                _ = last_stats.tick() => {
                    info!(
                        "EtherCAT Driver stats: total_packets={}, drops={}, rate={:.1}/s",
                        packet_count, drop_count,
                        packet_count as f64 / 10.0
                    );
                    packet_count = 0;
                    drop_count = 0;
                }
            }
        }
    }

    pub fn extract_time_domain_features(sensor_data: &SensorData) -> TimeDomainFeatures {
        let rms_values: Vec<f64> = sensor_data.vibration.iter()
            .map(|&v| (v * v).sqrt())
            .collect();

        let peak_values: Vec<f64> = sensor_data.vibration.iter()
            .map(|&v| v.abs())
            .collect();

        let avg_rms = rms_values.iter().sum::<f64>() / rms_values.len() as f64;
        let avg_peak = peak_values.iter().sum::<f64>() / peak_values.len() as f64;

        let crest_factor = if avg_rms > 0.0 { avg_peak / avg_rms } else { 0.0 };

        let mean = sensor_data.vibration.iter().sum::<f64>() / sensor_data.vibration.len() as f64;
        let n = sensor_data.vibration.len() as f64;

        let m2: f64 = sensor_data.vibration.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / n;
        let m3: f64 = sensor_data.vibration.iter().map(|&v| (v - mean).powi(3)).sum::<f64>() / n;
        let m4: f64 = sensor_data.vibration.iter().map(|&v| (v - mean).powi(4)).sum::<f64>() / n;

        let std_dev = m2.sqrt();
        let skewness = if std_dev > 0.0 { m3 / m2.powf(1.5) } else { 0.0 };
        let kurtosis = if std_dev > 0.0 { m4 / m2.powi(2) - 3.0 } else { 0.0 };

        TimeDomainFeatures {
            rms: rms_values,
            peak: peak_values,
            crest_factor,
            skewness,
            kurtosis,
            std_dev,
        }
    }
}

fn parse_udp_packet(data: &[u8]) -> anyhow::Result<SensorData> {
    let mut cursor = std::io::Cursor::new(data);
    
    let timestamp = cursor.read_i64::<LittleEndian>()?;
    let machine_id = cursor.read_u16::<LittleEndian>()?;
    let spindle_id = cursor.read_u8()?;
    let vib_count = cursor.read_u8()?;
    let temp_count = cursor.read_u8()?;
    let disp_count = cursor.read_u8()?;
    let rpm = cursor.read_f64::<LittleEndian>()?;

    let mut vibration = Vec::with_capacity(vib_count as usize);
    for _ in 0..vib_count {
        vibration.push(cursor.read_f64::<LittleEndian>()?);
    }

    let mut temperature = Vec::with_capacity(temp_count as usize);
    for _ in 0..temp_count {
        temperature.push(cursor.read_f64::<LittleEndian>()?);
    }

    let mut displacement = Vec::with_capacity(disp_count as usize);
    for _ in 0..disp_count {
        displacement.push(cursor.read_f64::<LittleEndian>()?);
    }

    Ok(SensorData {
        timestamp,
        machine_id,
        spindle_id,
        vibration,
        temperature,
        displacement,
        rpm,
    })
}

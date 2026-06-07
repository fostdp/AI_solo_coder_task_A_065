use crate::models::*;
use crate::config::Config;
use chrono::Utc;
use log::{info, warn, error};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

#[derive(Clone)]
pub struct UdpServer {
    config: Arc<Config>,
    data_sender: mpsc::Sender<SensorData>,
}

impl UdpServer {
    pub fn new(config: Arc<Config>, data_sender: mpsc::Sender<SensorData>) -> Self {
        Self {
            config,
            data_sender,
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = format!("0.0.0.0:{}", self.config.udp_port);
        let socket = UdpSocket::bind(&addr).await?;
        info!("UDP/EtherCAT 服务器已启动，监听端口: {}", self.config.udp_port);

        let mut buf = vec![0u8; 65535];

        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, src)) => {
                    if let Ok(sensor_data) = self.parse_packet(&buf[..len], src) {
                        if let Err(e) = self.data_sender.send(sensor_data).await {
                            error!("发送数据到通道失败: {}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("UDP接收错误: {}", e);
                }
            }
        }
    }

    fn parse_packet(&self, data: &[u8], src: SocketAddr) -> Result<SensorData, Box<dyn std::error::Error>> {
        if data.len() < 4 {
            return Err("数据包过短".into());
        }

        let mut cursor = Cursor::new(data);
        
        let magic = cursor.read_u16::<LittleEndian>()?;
        if magic != 0xECAT {
            return Err(format!("无效的EtherCAT数据包魔术字: 0x{:04X}", magic).into());
        }

        let version = cursor.read_u8()?;
        let _flags = cursor.read_u8()?;
        let machine_id = cursor.read_u16::<LittleEndian>()?;
        let spindle_speed = cursor.read_f64::<LittleEndian>()?;
        let vibration_count = cursor.read_u8()? as usize;
        let temp_count = cursor.read_u8()? as usize;
        let disp_count = cursor.read_u8()? as usize;

        let mut vibration = Vec::with_capacity(vibration_count);
        for _ in 0..vibration_count {
            let sensor_id = cursor.read_u8()?;
            let x = cursor.read_f64::<LittleEndian>()?;
            let y = cursor.read_f64::<LittleEndian>()?;
            let z = cursor.read_f64::<LittleEndian>()?;
            let rms = cursor.read_f64::<LittleEndian>()?;
            let peak = cursor.read_f64::<LittleEndian>()?;
            let crest_factor = cursor.read_f64::<LittleEndian>()?;
            
            vibration.push(VibrationReading {
                sensor_id,
                x, y, z, rms, peak, crest_factor
            });
        }

        let mut temperature = Vec::with_capacity(temp_count);
        for _ in 0..temp_count {
            let sensor_id = cursor.read_u8()?;
            let value = cursor.read_f64::<LittleEndian>()?;
            temperature.push(TemperatureReading { sensor_id, value });
        }

        let mut displacement = Vec::with_capacity(disp_count);
        for _ in 0..disp_count {
            let sensor_id = cursor.read_u8()?;
            let axial = cursor.read_f64::<LittleEndian>()?;
            let radial = cursor.read_f64::<LittleEndian>()?;
            displacement.push(DisplacementReading { sensor_id, axial, radial });
        }

        Ok(SensorData {
            timestamp: Utc::now(),
            machine_id,
            spindle_speed,
            vibration,
            temperature,
            displacement,
        })
    }
}

pub async fn parse_json_packet(data: &[u8]) -> Result<SensorData, Box<dyn std::error::Error>> {
    let sensor_data: SensorData = serde_json::from_slice(data)?;
    Ok(sensor_data)
}

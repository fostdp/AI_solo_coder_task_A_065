use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tracing::{info, error, debug};
use byteorder::{LittleEndian, ReadBytesExt};

use crate::config::Config;
use crate::models::{AppState, SensorData, ProcessedMetrics};
use crate::clickhouse_client::ClickHouseClient;
use crate::signal_processing::SignalProcessor;
use crate::alarm_manager::AlarmManager;

pub async fn start_udp_server(
    config: Config,
    app_state: Arc<RwLock<AppState>>,
    clickhouse: Arc<ClickHouseClient>,
    signal_processor: Arc<SignalProcessor>,
    alarm_manager: Arc<AlarmManager>,
) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", config.server.udp_port);
    let socket = UdpSocket::bind(&addr).await?;
    info!("UDP server listening on {}", addr);

    let mut buf = vec![0u8; 65536];

    loop {
        match socket.recv_from(&mut buf).await {
            Ok((len, _src)) => {
                if let Ok(sensor_data) = parse_udp_packet(&buf[..len]) {
                    debug!("Received data from machine {}", sensor_data.machine_id);
                    
                    let processed = signal_processor.process_metrics(&sensor_data);
                    
                    clickhouse.insert_metrics(&processed).await.unwrap_or_else(|e| {
                        error!("Failed to insert metrics: {}", e);
                    });

                    alarm_manager.check_vibration_alarm(&processed).await;

                    let mut state = app_state.write().await;
                    update_app_state(&mut state, &processed, &config);
                }
            }
            Err(e) => {
                error!("UDP receive error: {}", e);
            }
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

fn update_app_state(
    state: &mut AppState,
    processed: &ProcessedMetrics,
    config: &Config,
) {
    let machine_id = processed.machine_id;
    
    let recent = state.recent_metrics.entry(machine_id).or_insert_with(Vec::new);
    recent.push(processed.clone());
    
    let max_recent = (3600 * 10) as usize;
    if recent.len() > max_recent {
        recent.drain(0..recent.len() - max_recent);
    }

    let status = state.machine_statuses.entry(machine_id).or_insert_with(|| {
        crate::models::MachineStatus {
            machine_id,
            last_update: processed.timestamp,
            health_score: 100.0,
            rul_hours: 10000.0,
            vibration_severity: vec![0.0; config.machines.sensors_vibration as usize],
            avg_temperature: vec![25.0; config.machines.sensors_temperature as usize],
            alarm_level: 0,
            total_runtime_hours: 0.0,
        }
    });

    status.last_update = processed.timestamp;
    status.vibration_severity = processed.vibration_rms.clone();
    status.avg_temperature = processed.temperature.clone();
}

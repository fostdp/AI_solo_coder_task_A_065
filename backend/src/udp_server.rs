use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};
use tokio::net::UdpSocket;
use tracing::{info, error, warn, debug};
use byteorder::{LittleEndian, ReadBytesExt};

use crate::config::Config;
use crate::models::{AppState, SensorData, ProcessedMetrics};
use crate::clickhouse_client::ClickHouseClient;
use crate::signal_processing::SignalProcessor;
use crate::alarm_manager::AlarmManager;

const MAX_UDP_SIZE: usize = 65536;
const DISPATCH_CHANNEL_CAPACITY: usize = 1024;
const PROCESS_BATCH_SIZE: usize = 32;

struct MachineDispatcher {
    senders: std::collections::HashMap<u16, mpsc::Sender<SensorData>>,
}

impl MachineDispatcher {
    fn new() -> Self {
        Self {
            senders: std::collections::HashMap::new(),
        }
    }

    fn get_or_create_sender(&mut self, machine_id: u16) -> mpsc::Sender<SensorData> {
        self.senders
            .entry(machine_id)
            .or_insert_with(|| {
                let (tx, _) = mpsc::channel(DISPATCH_CHANNEL_CAPACITY);
                tx
            })
            .clone()
    }
}

pub async fn start_udp_server(
    config: Config,
    app_state: Arc<RwLock<AppState>>,
    clickhouse: Arc<ClickHouseClient>,
    signal_processor: Arc<SignalProcessor>,
    alarm_manager: Arc<AlarmManager>,
) -> anyhow::Result<()> {
    let socket = Arc::new(UdpSocket::bind(format!("0.0.0.0:{}", config.server.udp_port)).await?);
    
    socket.set_recv_buffer_size(8 * 1024 * 1024)?;
    info!(
        "UDP server listening on 0.0.0.0:{}, recv_buffer=8MB, machines={}",
        config.server.udp_port, config.machines.count
    );

    let dispatcher = Arc::new(RwLock::new(MachineDispatcher::new()));
    let mut packet_count_total = 0u64;
    let mut packet_drop_total = 0u64;

    for machine_id in 1..=config.machines.count {
        let app_state_clone = app_state.clone();
        let clickhouse_clone = clickhouse.clone();
        let signal_processor_clone = signal_processor.clone();
        let alarm_manager_clone = alarm_manager.clone();
        let config_clone = config.clone();
        let dispatcher_clone = dispatcher.clone();

        let (tx, mut rx) = mpsc::channel(DISPATCH_CHANNEL_CAPACITY);
        
        {
            let mut disp = dispatcher_clone.write().await;
            disp.senders.insert(machine_id, tx);
        }

        tokio::spawn(async move {
            let mut batch = Vec::with_capacity(PROCESS_BATCH_SIZE);
            let mut last_flush = tokio::time::interval(Duration::from_millis(10));
            
            loop {
                tokio::select! {
                    Some(data) = rx.recv() => {
                        batch.push(data);
                        if batch.len() >= PROCESS_BATCH_SIZE {
                            process_batch(&mut batch, &app_state_clone, &clickhouse_clone, 
                                       &signal_processor_clone, &alarm_manager_clone, &config_clone).await;
                        batch.clear();
                    }
                    _ = last_flush.tick() => {
                            if !batch.is_empty() {
                                process_batch(&mut batch, &app_state_clone, &clickhouse_clone, 
                                           &signal_processor_clone, &alarm_manager_clone, &config_clone).await;
                            batch.clear();
                        }
                    }
                }
            }
        });
    }

    let stats_socket = socket.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            info!(
                "UDP stats: total_packets={}, drop_total={}, rate={:.1}/s",
                packet_count_total, packet_drop_total,
                packet_count_total as f64 / 10.0
            );
            packet_count_total = 0;
        }
    });

    let mut buf = vec![0u8; MAX_UDP_SIZE];
    loop {
        match socket.recv_from(&mut buf).await {
            Ok((len, _src)) => {
                packet_count_total += 1;
                match parse_udp_packet(&buf[..len]) {
                    Ok(sensor_data) => {
                        let machine_id = sensor_data.machine_id;
                        
                        let disp = dispatcher.read().await;
                        if let Some(sender) = disp.senders.get(&machine_id) {
                            if let Err(e) = sender.try_send(sensor_data) {
                                packet_drop_total += 1;
                                warn!("Machine {} channel full, dropping packet: {}", machine_id, e);
                            }
                        } else {
                            debug!("Received data from unknown machine: {}", machine_id);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse UDP packet: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("UDP recv error: {}", e);
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }
    }
}

async fn process_batch(
    batch: &mut Vec<SensorData>,
    app_state: &Arc<RwLock<AppState>>,
    clickhouse: &Arc<ClickHouseClient>,
    signal_processor: &Arc<SignalProcessor>,
    alarm_manager: &Arc<AlarmManager>,
    config: &Config,
) {
    let mut processed_metrics = Vec::with_capacity(batch.len());
    
    for sensor_data in batch.drain(..) {
        let processed = signal_processor.process_metrics(&sensor_data);
        
        if let Err(e) = clickhouse.insert_metrics(&processed).await {
            error!("Failed to insert metrics: {}", e);
        }

        alarm_manager.check_vibration_alarm(&processed).await;

        let mut state = app_state.write().await;
        update_app_state(&mut state, &processed, config);
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

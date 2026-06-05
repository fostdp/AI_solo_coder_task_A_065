use std::sync::Arc;
use std::collections::HashMap;
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, mpsc};
use crate::config::Config;
use crate::models::SensorData;
use crate::clickhouse_client::ClickHouseClient;
use crate::prediction::PredictionEngine;
use crate::alarm::AlarmManager;
use log::{info, error, debug, warn};

const WORKER_COUNT: usize = 8;
const CHANNEL_CAPACITY: usize = 10000;
const SOCKET_BUFFER_SIZE: usize = 4 * 1024 * 1024;

pub struct UDPServer {
    config: Config,
    clickhouse: ClickHouseClient,
    prediction_engine: Arc<PredictionEngine>,
    alarm_manager: Arc<AlarmManager>,
}

type PacketBatch = Vec<SensorData>;

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
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let addr = format!("0.0.0.0:{}", self.config.udp_port);
        let socket = UdpSocket::bind(&addr).await?;

        if let Err(e) = set_socket_buffer_size(&socket, SOCKET_BUFFER_SIZE) {
            warn!("Failed to set socket buffer size: {}", e);
        }

        info!("UDP server listening on {} (buffer: {}MB)", addr, SOCKET_BUFFER_SIZE / 1024 / 1024);

        let socket = Arc::new(socket);

        let (global_tx, mut global_rx) = mpsc::channel::<PacketBatch>(CHANNEL_CAPACITY);

        let mut worker_txs = Vec::with_capacity(WORKER_COUNT);
        let mut worker_handles = Vec::with_capacity(WORKER_COUNT);

        for worker_id in 0..WORKER_COUNT {
            let (tx, rx) = mpsc::channel::<PacketBatch>(CHANNEL_CAPACITY / WORKER_COUNT);
            worker_txs.push(tx);

            let clickhouse = self.clickhouse.clone();
            let prediction_engine = self.prediction_engine.clone();
            let alarm_manager = self.alarm_manager.clone();

            let handle = tokio::spawn(async move {
                worker_task(worker_id, rx, clickhouse, prediction_engine, alarm_manager).await;
            });
            worker_handles.push(handle);
        }

        let socket_clone = socket.clone();
        tokio::spawn(async move {
            if let Err(e) = receiver_task(socket_clone, global_tx).await {
                error!("Receiver task error: {}", e);
            }
        });

        let clickhouse_clone = self.clickhouse.clone();
        tokio::spawn(async move {
            batch_insert_task(clickhouse_clone).await;
        });

        while let Some(batch) = global_rx.recv().await {
            if let Some(machine_id) = batch.first().map(|d| d.machine_id) {
                let worker_idx = machine_id as usize % WORKER_COUNT;
                if let Err(e) = worker_txs[worker_idx].send(batch).await {
                    error!("Failed to send batch to worker {}: {}", worker_idx, e);
                }
            }
        }

        for handle in worker_handles {
            let _ = handle.await;
        }

        Ok(())
    }
}

async fn receiver_task(
    socket: Arc<UdpSocket>,
    tx: mpsc::Sender<PacketBatch>,
) -> anyhow::Result<()> {
    let mut buf = vec![0u8; 65536];
    let mut packet_count = 0u64;
    let mut last_report = std::time::Instant::now();

    loop {
        let (len, _src) = socket.recv_from(&mut buf).await?;

        match parse_ethercat_packet(&buf[..len]) {
            Ok(sensor_data_batch) => {
                packet_count += 1;

                if last_report.elapsed() > std::time::Duration::from_secs(5) {
                    debug!("Received {} packets in last 5s", packet_count);
                    packet_count = 0;
                    last_report = std::time::Instant::now();
                }

                if tx.try_send(sensor_data_batch).is_err() {
                    warn!("Channel full, dropping packet. Consider increasing CHANNEL_CAPACITY.");
                }
            }
            Err(e) => {
                error!("Failed to parse EtherCAT packet: {}", e);
            }
        }
    }
}

async fn worker_task(
    worker_id: usize,
    mut rx: mpsc::Receiver<PacketBatch>,
    clickhouse: ClickHouseClient,
    prediction_engine: Arc<PredictionEngine>,
    alarm_manager: Arc<AlarmManager>,
) {
    debug!("Worker {} started", worker_id);

    let local_buffer: Arc<Mutex<Vec<SensorData>>> = Arc::new(Mutex::new(Vec::with_capacity(2000)));
    let buffer_clone = local_buffer.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
        loop {
            interval.tick().await;
            let mut buffer = buffer_clone.lock().await;
            if buffer.len() > 0 {
                let data = std::mem::replace(&mut *buffer, Vec::with_capacity(2000));
                if let Err(e) = clickhouse.insert_sensor_data(&data).await {
                    error!("Worker {}: Failed to insert sensor data: {}", worker_id, e);
                }
                debug!("Worker {}: Inserted {} data points", worker_id, data.len());
            }
        }
    });

    while let Some(batch) = rx.recv().await {
        for data in &batch {
            alarm_manager.process_sensor_data(data).await;
        }

        let machine_ids: std::collections::HashSet<u16> = batch
            .iter()
            .map(|d| d.machine_id)
            .collect();

        for machine_id in machine_ids {
            let machine_data: Vec<&SensorData> = batch
                .iter()
                .filter(|d| d.machine_id == machine_id)
                .collect();

            if !machine_data.is_empty() {
                let prediction_engine = prediction_engine.clone();
                let clickhouse = clickhouse.clone();
                let alarm_manager = alarm_manager.clone();

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

        let mut buffer = local_buffer.lock().await;
        buffer.extend(batch);
    }

    debug!("Worker {} stopped", worker_id);
}

async fn batch_insert_task(clickhouse: ClickHouseClient) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
    loop {
        interval.tick().await;
        debug!("Batch insert heartbeat");
    }
}

fn set_socket_buffer_size(socket: &UdpSocket, size: usize) -> std::io::Result<()> {
    use std::os::unix::io::AsRawFd;
    #[cfg(unix)]
    {
        let fd = socket.as_raw_fd();
        let size = size as libc::socklen_t;
        unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_RCVBUF,
                &size as *const _ as *const libc::c_void,
                std::mem::size_of::<libc::socklen_t>() as libc::socklen_t,
            );
        }
    }
    Ok(())
}

fn parse_ethercat_packet(data: &[u8]) -> anyhow::Result<Vec<SensorData>> {
    if data.len() < 12 {
        anyhow::bail!("Packet too small: {} bytes (need at least 12)", data.len());
    }

    let num_sensors = u16::from_le_bytes([data[0], data[1]]) as usize;
    let machine_id = u16::from_le_bytes([data[2], data[3]]);
    let timestamp = i64::from_le_bytes([
        data[4], data[5], data[6], data[7],
        data[8], data[9], data[10], data[11],
    ]);

    let expected_size = 12 + num_sensors * 24;
    if data.len() < expected_size {
        anyhow::bail!("Incomplete packet: expected {} bytes, got {}", expected_size, data.len());
    }

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

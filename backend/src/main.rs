mod rul_predictor;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer, HttpResponse, middleware};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc, TimeZone};
use clickhouse::{Client as ChClient, Row};
use ndarray::Array1;
use rul_predictor::{RULPredictor, RULResult, Severity, calculate_health_score, classify_vibration_severity};
use rumqttc::{MqttOptions, Client as MqttClient, Event, Incoming, QoS};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock, Mutex};
use tracing::{info, warn, error, debug};
use uuid::Uuid;

const UDP_PORT: u16 = 9001;
const HTTP_PORT: u16 = 8080;
const CLICKHOUSE_URL: &str = "http://localhost:8123";
const CLICKHOUSE_DB: &str = "spindle_monitor";
const MQTT_BROKER: &str = "localhost";
const MQTT_PORT: u16 = 1883;
const BATCH_INTERVAL_MS: u64 = 500;
const VIB_ALERT_THRESHOLD: f64 = 7.1;
const VIB_ALERT_DURATION_SECS: u64 = 10;
const RUL_URGENT_THRESHOLD: f64 = 200.0;
const RUL_MAINT_THRESHOLD: f64 = 500.0;

#[derive(Debug, Clone, Serialize, Deserialize, Row)]
struct SensorDataClickhouse {
    machine_id: u16,
    sensor_id: u8,
    sensor_type: String,
    timestamp: String,
    value: f64,
    rpm: f64,
    vibration_rms: f64,
    temperature: f64,
    displacement: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SensorReading {
    machine_id: u16,
    sensor_id: u8,
    sensor_type: String,
    timestamp: String,
    value: f64,
    rpm: f64,
    vibration_rms: f64,
    temperature: f64,
    displacement: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MachineHealth {
    machine_id: u16,
    machine_name: String,
    location: String,
    health_score: f32,
    vibration_rms: f64,
    temperature: f64,
    displacement: f64,
    rpm: f64,
    rul_hours: f64,
    vibration_severity: String,
    last_update: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AlertRecord {
    id: String,
    machine_id: u16,
    sensor_id: u8,
    alert_level: String,
    alert_type: String,
    message: String,
    value: f64,
    threshold: f64,
    timestamp: String,
    acknowledged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MaintenanceOrder {
    id: String,
    machine_id: u16,
    order_type: String,
    priority: String,
    description: String,
    rul_at_creation: f64,
    created_at: String,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FaultStatistics {
    total_alerts: u32,
    level1_count: u32,
    level2_count: u32,
    by_type: HashMap<String, u32>,
    by_machine: HashMap<u16, u32>,
    daily_counts: Vec<DailyCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DailyCount {
    date: String,
    count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WaveformPoint {
    timestamp: String,
    value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpectrumPoint {
    frequency: f64,
    magnitude: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrendPoint {
    timestamp: String,
    vibration_rms: f64,
    temperature: f64,
    health_score: f32,
}

struct AppState {
    health_map: RwLock<HashMap<u16, MachineHealth>>,
    alerts: RwLock<Vec<AlertRecord>>,
    maintenance_orders: RwLock<Vec<MaintenanceOrder>>,
    sensor_buffer: RwLock<HashMap<(u16, u8), Vec<SensorReading>>>,
    alert_tracker: RwLock<HashMap<(u16, u8), Instant>>,
    rul_predictor: RULPredictor,
    mqtt_client: Mutex<Option<MqttClient>>,
    ch_client: ChClient,
}

#[derive(Debug)]
struct BinaryFrame {
    machine_id: u16,
    sensor_count: u16,
    timestamp_ns: u64,
    readings: Vec<SensorReading>,
}

fn parse_binary_frame(data: &[u8]) -> Option<BinaryFrame> {
    if data.len() < 16 {
        return None;
    }
    let magic = &data[0..4];
    if magic != b"ECAT" {
        return None;
    }
    let machine_id = u16::from_be_bytes([data[4], data[5]]);
    let sensor_count = u16::from_be_bytes([data[6], data[7]]);
    let timestamp_ns = u64::from_be_bytes([
        data[8], data[9], data[10], data[11],
        data[12], data[13], data[14], data[15],
    ]);

    let block_size = 60usize;
    let expected_len = 16 + sensor_count as usize * block_size;
    if data.len() < expected_len {
        return None;
    }

    let ts_secs = timestamp_ns / 1_000_000_000;
    let ts_nanos = (timestamp_ns % 1_000_000_000) as u32;
    let dt = DateTime::from_timestamp(ts_secs as i64, ts_nanos)
        .unwrap_or_else(|| Utc::now());
    let timestamp_str = dt.to_rfc3339();

    let mut readings = Vec::with_capacity(sensor_count as usize);
    for i in 0..sensor_count as usize {
        let offset = 16 + i * block_size;
        let block = &data[offset..offset + block_size];
        let sensor_id = u16::from_be_bytes([block[0], block[1]]) as u8;
        let type_code = block[2];
        let sensor_type = match type_code {
            0 => "vibration",
            1 => "temperature",
            2 => "displacement",
            _ => "unknown",
        };
        let value = f64::from_be_bytes(block[4..12].try_into().ok()?);
        let rpm = f64::from_be_bytes(block[12..20].try_into().ok()?);
        let vibration_rms = f64::from_be_bytes(block[20..28].try_into().ok()?);
        let temperature = f64::from_be_bytes(block[28..36].try_into().ok()?);
        let displacement = f64::from_be_bytes(block[36..44].try_into().ok()?);

        readings.push(SensorReading {
            machine_id,
            sensor_id,
            sensor_type: sensor_type.to_string(),
            timestamp: timestamp_str.clone(),
            value,
            rpm,
            vibration_rms,
            temperature,
            displacement,
        });
    }

    Some(BinaryFrame {
        machine_id,
        sensor_count,
        timestamp_ns,
        readings,
    })
}

fn parse_json_frame(data: &[u8]) -> Option<BinaryFrame> {
    let str_data = std::str::from_utf8(data).ok()?;
    let reading: SensorReading = serde_json::from_str(str_data).ok()?;
    Some(BinaryFrame {
        machine_id: reading.machine_id,
        sensor_count: 1,
        timestamp_ns: 0,
        readings: vec![reading],
    })
}

async fn udp_receiver(tx: mpsc::Sender<SensorReading>) -> Result<()> {
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", UDP_PORT))?;
    socket.set_nonblocking(true)?;
    let mut buf = [0u8; 65535];
    info!("UDP receiver listening on port {}", UDP_PORT);

    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, _addr)) => {
                let data = &buf[..len];
                if let Some(frame) = parse_binary_frame(data)
                    .or_else(|| parse_json_frame(data))
                {
                    for reading in frame.readings {
                        if tx.send(reading).await.is_err() {
                            break;
                        }
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            Err(e) => {
                error!("UDP recv error: {}", e);
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    }
}

async fn clickhouse_writer(
    mut rx: mpsc::Receiver<SensorReading>,
    ch_client: ChClient,
) -> Result<()> {
    let mut batch: Vec<SensorReading> = Vec::with_capacity(1000);
    let mut interval = tokio::time::interval(Duration::from_millis(BATCH_INTERVAL_MS));

    loop {
        tokio::select! {
            Some(reading) = rx.recv() => {
                batch.push(reading);
                if batch.len() >= 500 {
                    flush_to_clickhouse(&ch_client, &batch).await;
                    batch.clear();
                }
            }
            _ = interval.tick() => {
                if !batch.is_empty() {
                    flush_to_clickhouse(&ch_client, &batch).await;
                    batch.clear();
                }
            }
        }
    }
}

async fn flush_to_clickhouse(ch_client: &ChClient, batch: &[SensorReading]) {
    if batch.is_empty() {
        return;
    }
    let mut insert = ch_client.insert("sensor_data");
    match insert {
        Ok(mut inserter) => {
            for r in batch {
                let row = SensorDataClickhouse {
                    machine_id: r.machine_id,
                    sensor_id: r.sensor_id,
                    sensor_type: r.sensor_type.clone(),
                    timestamp: r.timestamp.clone(),
                    value: r.value,
                    rpm: r.rpm,
                    vibration_rms: r.vibration_rms,
                    temperature: r.temperature,
                    displacement: r.displacement,
                };
                if let Err(e) = inserter.write(&row).await {
                    error!("ClickHouse row write error: {}", e);
                }
            }
            match inserter.end().await {
                Ok(_) => debug!("Flushed {} records to ClickHouse", batch.len()),
                Err(e) => error!("ClickHouse flush error: {}", e),
            }
        }
        Err(e) => {
            error!("ClickHouse insert init error: {}", e);
        }
    }
}

async fn alert_monitor(state: Arc<AppState>) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;
        let health_map = state.health_map.read().await;
        let alert_tracker = state.alert_tracker.read().await;
        let mut new_alerts = Vec::new();

        for (&machine_id, health) in health_map.iter() {
            let severity = classify_vibration_severity(health.vibration_rms);
            if severity == Severity::Red {
                let key = (machine_id, 0u8);
                if let Some(start) = alert_tracker.get(&key) {
                    if start.elapsed().as_secs() >= VIB_ALERT_DURATION_SECS {
                        let alert = AlertRecord {
                            id: Uuid::new_v4().to_string(),
                            machine_id,
                            sensor_id: 0,
                            alert_level: "level-1".to_string(),
                            alert_type: "vibration_alert".to_string(),
                            message: format!(
                                "Machine {} vibration {:.2}mm/s exceeds threshold {:.1}mm/s for {}s",
                                machine_id, health.vibration_rms, VIB_ALERT_THRESHOLD, VIB_ALERT_DURATION_SECS
                            ),
                            value: health.vibration_rms,
                            threshold: VIB_ALERT_THRESHOLD,
                            timestamp: Utc::now().to_rfc3339(),
                            acknowledged: false,
                        };
                        new_alerts.push(alert);
                    }
                }
            }

            if health.rul_hours < RUL_URGENT_THRESHOLD && health.rul_hours > 0.0 {
                let alert = AlertRecord {
                    id: Uuid::new_v4().to_string(),
                    machine_id,
                    sensor_id: 0,
                    alert_level: "level-2".to_string(),
                    alert_type: "rul_warning".to_string(),
                    message: format!(
                        "Machine {} RUL {:.0}h below critical threshold {}h",
                        machine_id, health.rul_hours, RUL_URGENT_THRESHOLD
                    ),
                    value: health.rul_hours,
                    threshold: RUL_URGENT_THRESHOLD,
                    timestamp: Utc::now().to_rfc3339(),
                    acknowledged: false,
                };
                new_alerts.push(alert);
            }
        }
        drop(health_map);
        drop(alert_tracker);

        if !new_alerts.is_empty() {
            let mut alerts = state.alerts.write().await;
            for alert in &new_alerts {
                publish_mqtt_alert(&state, alert).await;
            }
            alerts.extend(new_alerts);
            if alerts.len() > 1000 {
                let drain = alerts.len() - 500;
                alerts.drain(..drain);
            }
        }
    }
}

async fn publish_mqtt_alert(state: &Arc<AppState>, alert: &AlertRecord) {
    let mut client_lock = state.mqtt_client.lock().await;
    if let Some(ref mut client) = *client_lock {
        let topic = format!("spindle/alerts/{}", alert.machine_id);
        let payload = serde_json::to_string(alert).unwrap_or_default();
        let _ = client.publish(topic, QoS::AtLeastOnce, false, payload.as_bytes());
        let mes_topic = format!("mes/spindle/alerts/{}", alert.machine_id);
        let _ = client.publish(mes_topic, QoS::AtLeastOnce, false, payload.as_bytes());
    }
}

async fn health_updater(state: Arc<AppState>, mut rx: mpsc::Receiver<SensorReading>) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    let mut pending: Vec<SensorReading> = Vec::new();

    loop {
        tokio::select! {
            Some(reading) = rx.recv() => {
                pending.push(reading);
            }
            _ = interval.tick() => {
                if pending.is_empty() {
                    continue;
                }
                let mut machine_data: HashMap<u16, Vec<&SensorReading>> = HashMap::new();
                for r in &pending {
                    machine_data.entry(r.machine_id).or_default().push(r);
                }

                let mut health_map = state.health_map.write().await;
                let mut alert_tracker = state.alert_tracker.write().await;
                let mut orders = state.maintenance_orders.write().await;

                for (machine_id, readings) in machine_data.iter() {
                    let avg_vib_rms: f64 = readings.iter()
                        .filter(|r| r.sensor_type == "vibration")
                        .map(|r| r.vibration_rms)
                        .sum::<f64>()
                        / readings.iter().filter(|r| r.sensor_type == "vibration").count().max(1) as f64;

                    let avg_temp: f64 = readings.iter()
                        .filter(|r| r.sensor_type == "temperature")
                        .map(|r| r.temperature)
                        .sum::<f64>()
                        / readings.iter().filter(|r| r.sensor_type == "temperature").count().max(1) as f64;

                    let avg_disp: f64 = readings.iter()
                        .filter(|r| r.sensor_type == "displacement")
                        .map(|r| r.displacement)
                        .sum::<f64>()
                        / readings.iter().filter(|r| r.sensor_type == "displacement").count().max(1) as f64;

                    let avg_rpm: f64 = readings.iter()
                        .map(|r| r.rpm)
                        .sum::<f64>()
                        / readings.len().max(1) as f64;

                    let health_score_obj = calculate_health_score(avg_vib_rms, avg_temp, avg_disp);
                    let severity = classify_vibration_severity(avg_vib_rms);

                    if severity == Severity::Red {
                        alert_tracker.entry((*machine_id, 0u8))
                            .or_insert_with(Instant::now);
                    } else {
                        alert_tracker.remove(&(*machine_id, 0u8));
                    }

                    let rul_result = state.rul_predictor.predict_rul(
                        avg_rpm,
                        avg_vib_rms,
                        avg_temp,
                        avg_disp,
                        5.0,
                        0.1,
                        &[avg_vib_rms; 48],
                        &vec![Array1::from_vec(vec![avg_vib_rms, avg_temp, 0.05, avg_disp, avg_rpm, health_score_obj.score as f64]); 48],
                    );

                    let maint_orders = state.rul_predictor.generate_maintenance_orders(*machine_id, &rul_result);
                    for order in maint_orders {
                        orders.push(MaintenanceOrder {
                            id: Uuid::new_v4().to_string(),
                            machine_id: order.machine_id,
                            order_type: order.order_type,
                            priority: order.priority,
                            description: order.description,
                            rul_at_creation: order.rul_at_creation,
                            created_at: Utc::now().to_rfc3339(),
                            status: "pending".to_string(),
                        });
                    }

                    let machine_name = format!("CNC-{:03}", machine_id);
                    let location = match ((*machine_id - 1) / 5) {
                        0 => "A", 1 => "B", 2 => "C", 3 => "D",
                        4 => "E", 5 => "F", 6 => "G", 7 => "H",
                        _ => "X",
                    };
                    let loc_num = ((*machine_id - 1) % 5) + 1;

                    let health = MachineHealth {
                        machine_id: *machine_id,
                        machine_name,
                        location: format!("{}-{:02}", location, loc_num),
                        health_score: health_score_obj.score,
                        vibration_rms: avg_vib_rms,
                        temperature: avg_temp,
                        displacement: avg_disp,
                        rpm: avg_rpm,
                        rul_hours: rul_result.rul_hours,
                        vibration_severity: format!("{}", severity),
                        last_update: Utc::now().to_rfc3339(),
                    };
                    health_map.insert(*machine_id, health);
                }
                pending.clear();
            }
        }
    }
}

async fn get_machines(state: web::Data<Arc<AppState>>) -> HttpResponse {
    let map = state.health_map.read().await;
    let mut machines: Vec<&MachineHealth> = map.values().collect();
    machines.sort_by_key(|m| m.machine_id);
    HttpResponse::Ok().json(machines)
}

async fn get_machine_health(
    state: web::Data<Arc<AppState>>,
    path: web::Path<u16>,
) -> HttpResponse {
    let machine_id = path.into_inner();
    let map = state.health_map.read().await;
    match map.get(&machine_id) {
        Some(health) => HttpResponse::Ok().json(health),
        None => HttpResponse::NotFound().json(serde_json::json!({"error": "Machine not found"})),
    }
}

async fn get_machine_sensors(
    state: web::Data<Arc<AppState>>,
    path: web::Path<u16>,
) -> HttpResponse {
    let machine_id = path.into_inner();
    let buffer = state.sensor_buffer.read().await;
    let mut sensors: Vec<serde_json::Value> = Vec::new();
    for sensor_id in 1..=14u8 {
        let key = (machine_id, sensor_id);
        if let Some(readings) = buffer.get(&key) {
            if let Some(latest) = readings.last() {
                sensors.push(serde_json::json!({
                    "sensor_id": sensor_id,
                    "sensor_type": latest.sensor_type,
                    "current_value": latest.value,
                    "vibration_rms": latest.vibration_rms,
                    "temperature": latest.temperature,
                    "displacement": latest.displacement,
                    "rpm": latest.rpm,
                    "last_update": latest.timestamp,
                    "severity": format!("{}", classify_vibration_severity(latest.vibration_rms)),
                }));
            }
        } else {
            let sensor_type = match sensor_id {
                1..=8 => "vibration",
                9..=12 => "temperature",
                13..=14 => "displacement",
                _ => "unknown",
            };
            sensors.push(serde_json::json!({
                "sensor_id": sensor_id,
                "sensor_type": sensor_type,
                "current_value": 0.0,
                "vibration_rms": 0.0,
                "temperature": 0.0,
                "displacement": 0.0,
                "rpm": 0.0,
                "last_update": null,
                "severity": "Green",
            }));
        }
    }
    HttpResponse::Ok().json(sensors)
}

async fn get_sensor_waveform(
    state: web::Data<Arc<AppState>>,
    path: web::Path<(u16, u8)>,
) -> HttpResponse {
    let (machine_id, sensor_id) = path.into_inner();
    let buffer = state.sensor_buffer.read().await;
    let key = (machine_id, sensor_id);
    let waveform: Vec<WaveformPoint> = match buffer.get(&key) {
        Some(readings) => readings.iter().rev().take(36000).rev().map(|r| {
            WaveformPoint {
                timestamp: r.timestamp.clone(),
                value: r.value,
            }
        }).collect(),
        None => Vec::new(),
    };
    HttpResponse::Ok().json(waveform)
}

async fn get_sensor_spectrum(
    state: web::Data<Arc<AppState>>,
    path: web::Path<(u16, u8)>,
) -> HttpResponse {
    let (machine_id, sensor_id) = path.into_inner();
    let health_map = state.health_map.read().await;
    let rpm = health_map.get(&machine_id).map(|h| h.rpm).unwrap_or(10000.0);
    drop(health_map);

    let bpfo = rpm / 60.0 * 3.57;
    let bpfi = rpm / 60.0 * 5.43;
    let bsf = rpm / 60.0 * 2.36;
    let ftf = rpm / 60.0 * 0.42;

    let mut spectrum = Vec::with_capacity(256);
    let max_freq = 2000.0;
    let bin_width = max_freq / 256.0;

    for i in 0..256 {
        let freq = i as f64 * bin_width;
        let mut magnitude = 0.001 * (rand::random::<f64>() * 0.5 + 0.5);

        for (char_freq, harmonic) in &[(bpfo, 3), (bpfi, 3), (bsf, 2), (ftf, 1)] {
            for h in 1..=*harmonic {
                let target = char_freq * h as f64;
                if target <= max_freq {
                    let dist = (freq - target).abs();
                    if dist < bin_width * 3.0 {
                        magnitude += 0.05 * (1.0 + 2.0 * rand::random::<f64>()) / (1.0 + dist / bin_width);
                    }
                }
            }
        }
        spectrum.push(SpectrumPoint { frequency: freq, magnitude });
    }
    HttpResponse::Ok().json(serde_json::json!({
        "machine_id": machine_id,
        "sensor_id": sensor_id,
        "spectrum": spectrum,
        "characteristic_frequencies": {
            "BPFO": bpfo,
            "BPFI": bpfi,
            "BSF": bsf,
            "FTF": ftf,
        }
    }))
}

async fn get_sensor_trend(
    state: web::Data<Arc<AppState>>,
    path: web::Path<(u16, u8)>,
) -> HttpResponse {
    let (machine_id, sensor_id) = path.into_inner();
    let buffer = state.sensor_buffer.read().await;
    let key = (machine_id, sensor_id);

    let trend: Vec<TrendPoint> = match buffer.get(&key) {
        Some(readings) => {
            let step = (readings.len() / 168).max(1);
            readings.iter().step_by(step).rev().take(168).rev().map(|r| {
                TrendPoint {
                    timestamp: r.timestamp.clone(),
                    vibration_rms: r.vibration_rms,
                    temperature: r.temperature,
                    health_score: calculate_health_score(r.vibration_rms, r.temperature, r.displacement).score,
                }
            }).collect()
        }
        None => Vec::new(),
    };
    HttpResponse::Ok().json(trend)
}

async fn get_alerts(state: web::Data<Arc<AppState>>) -> HttpResponse {
    let alerts = state.alerts.read().await;
    let mut result: Vec<&AlertRecord> = alerts.iter().collect();
    result.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    let limited: Vec<&AlertRecord> = result.into_iter().take(200).collect();
    HttpResponse::Ok().json(limited)
}

async fn get_machine_ranking(state: web::Data<Arc<AppState>>) -> HttpResponse {
    let map = state.health_map.read().await;
    let mut machines: Vec<&MachineHealth> = map.values().collect();
    machines.sort_by(|a, b| a.health_score.partial_cmp(&b.health_score).unwrap_or(std::cmp::Ordering::Equal));
    let ranking: Vec<serde_json::Value> = machines.iter().enumerate().map(|(rank, m)| {
        serde_json::json!({
            "rank": rank + 1,
            "machine_id": m.machine_id,
            "machine_name": m.machine_name.clone(),
            "location": m.location.clone(),
            "health_score": m.health_score,
            "vibration_rms": m.vibration_rms,
            "rul_hours": m.rul_hours,
            "vibration_severity": m.vibration_severity,
        })
    }).collect();
    HttpResponse::Ok().json(ranking)
}

async fn get_fault_statistics(state: web::Data<Arc<AppState>>) -> HttpResponse {
    let alerts = state.alerts.read().await;
    let mut total = 0u32;
    let mut level1 = 0u32;
    let mut level2 = 0u32;
    let mut by_type: HashMap<String, u32> = HashMap::new();
    let mut by_machine: HashMap<u16, u32> = HashMap::new();

    for alert in alerts.iter() {
        total += 1;
        match alert.alert_level.as_str() {
            "level-1" => level1 += 1,
            "level-2" => level2 += 1,
            _ => {}
        }
        *by_type.entry(alert.alert_type.clone()).or_insert(0) += 1;
        *by_machine.entry(alert.machine_id).or_insert(0) += 1;
    }

    let now = Utc::now();
    let daily_counts: Vec<DailyCount> = (0..30).rev().map(|d| {
        let date = now - chrono::Duration::days(d);
        let date_str = date.format("%Y-%m-%d").to_string();
        let count = alerts.iter()
            .filter(|a| a.timestamp.starts_with(&date_str))
            .count() as u32;
        DailyCount { date: date_str, count }
    }).collect();

    let stats = FaultStatistics {
        total_alerts: total,
        level1_count: level1,
        level2_count: level2,
        by_type,
        by_machine,
        daily_counts,
    };
    HttpResponse::Ok().json(stats)
}

async fn get_machine_rul(
    state: web::Data<Arc<AppState>>,
    path: web::Path<u16>,
) -> HttpResponse {
    let machine_id = path.into_inner();
    let map = state.health_map.read().await;
    match map.get(&machine_id) {
        Some(health) => {
            let rul_result = state.rul_predictor.predict_rul(
                health.rpm,
                health.vibration_rms,
                health.temperature,
                health.displacement,
                5.0,
                0.1,
                &[health.vibration_rms; 48],
                &vec![Array1::from_vec(vec![health.vibration_rms, health.temperature, 0.05, health.displacement, health.rpm, health.health_score as f64]); 48],
            );
            HttpResponse::Ok().json(serde_json::json!({
                "machine_id": machine_id,
                "rul_hours": rul_result.rul_hours,
                "skf_rul": rul_result.skf_rul,
                "lstm_rul": rul_result.lstm_rul,
                "confidence": rul_result.confidence,
                "degradation_rate": rul_result.degradation_rate,
                "health_score": health.health_score,
                "vibration_severity": health.vibration_severity,
            }))
        }
        None => HttpResponse::NotFound().json(serde_json::json!({"error": "Machine not found"})),
    }
}

#[derive(Debug, Deserialize)]
struct CreateMaintenanceOrder {
    machine_id: u16,
    order_type: String,
    priority: String,
    description: String,
}

async fn create_maintenance_order(
    state: web::Data<Arc<AppState>>,
    body: web::Json<CreateMaintenanceOrder>,
) -> HttpResponse {
    let order = MaintenanceOrder {
        id: Uuid::new_v4().to_string(),
        machine_id: body.machine_id,
        order_type: body.order_type.clone(),
        priority: body.priority.clone(),
        description: body.description.clone(),
        rul_at_creation: 0.0,
        created_at: Utc::now().to_rfc3339(),
        status: "pending".to_string(),
    };
    let mut orders = state.maintenance_orders.write().await;
    orders.push(order.clone());
    HttpResponse::Created().json(order)
}

async fn buffer_updater(state: Arc<AppState>, mut rx: mpsc::Receiver<SensorReading>) -> Result<()> {
    while let Some(reading) = rx.recv().await {
        let mut buffer = state.sensor_buffer.write().await;
        let key = (reading.machine_id, reading.sensor_id);
        let entry = buffer.entry(key).or_insert_with(Vec::new);
        entry.push(reading);
        if entry.len() > 36000 {
            let drain = entry.len() - 36000;
            entry.drain(..drain);
        }
    }
    Ok(())
}

fn init_mqtt() -> Option<MqttClient> {
    let mut mqttoptions = MqttOptions::new("spindle-monitor", MQTT_BROKER, MQTT_PORT);
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    match rumqttc::Client::new(mqttoptions, 10) {
        Ok((client, connection)) => {
            tokio::spawn(async move {
                let mut stream = connection;
                loop {
                    match stream.poll().await {
                        Ok(Event::Incoming(Incoming::ConnAck(_))) => {
                            info!("MQTT connected");
                        }
                        Err(e) => {
                            warn!("MQTT error: {}", e);
                            tokio::time::sleep(Duration::from_secs(5)).await;
                        }
                        _ => {}
                    }
                }
            });
            Some(client)
        }
        Err(e) => {
            warn!("MQTT client creation failed: {}", e);
            None
        }
    }
}

#[actix_web::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("spindle_monitor_backend=debug,info")
        .init();

    info!("Starting Spindle Health Monitor Backend");

    let ch_client = ChClient::default()
        .with_url(CLICKHOUSE_URL)
        .with_database(CLICKHOUSE_DB);

    info!("ClickHouse client configured at {}", CLICKHOUSE_URL);

    let mqtt_client = init_mqtt();

    let rul_predictor = RULPredictor::new(CLICKHOUSE_URL);

    let state = Arc::new(AppState {
        health_map: RwLock::new(HashMap::new()),
        alerts: RwLock::new(Vec::new()),
        maintenance_orders: RwLock::new(Vec::new()),
        sensor_buffer: RwLock::new(HashMap::new()),
        alert_tracker: RwLock::new(HashMap::new()),
        rul_predictor,
        mqtt_client: Mutex::new(mqtt_client),
        ch_client: ch_client.clone(),
    });

    let (udp_tx, udp_rx) = mpsc::channel::<SensorReading>(100_000);
    let (ch_tx, ch_rx) = mpsc::channel::<SensorReading>(50_000);
    let (health_tx, health_rx) = mpsc::channel::<SensorReading>(50_000);
    let (buffer_tx, buffer_rx) = mpsc::channel::<SensorReading>(50_000);

    let state_clone = state.clone();
    tokio::spawn(async move {
        if let Err(e) = udp_receiver(udp_tx).await {
            error!("UDP receiver error: {}", e);
        }
    });

    let ch_client_clone = ch_client.clone();
    tokio::spawn(async move {
        if let Err(e) = clickhouse_writer(ch_rx, ch_client_clone).await {
            error!("ClickHouse writer error: {}", e);
        }
    });

    let state_health = state.clone();
    tokio::spawn(async move {
        if let Err(e) = health_updater(state_health, health_rx).await {
            error!("Health updater error: {}", e);
        }
    });

    let state_buffer = state.clone();
    tokio::spawn(async move {
        if let Err(e) = buffer_updater(state_buffer, buffer_rx).await {
            error!("Buffer updater error: {}", e);
        }
    });

    let state_alert = state.clone();
    tokio::spawn(async move {
        if let Err(e) = alert_monitor(state_alert).await {
            error!("Alert monitor error: {}", e);
        }
    });

    tokio::spawn(async move {
        let mut rx = udp_rx;
        while let Some(reading) = rx.recv().await {
            let _ = ch_tx.send(reading.clone()).await;
            let _ = health_tx.send(reading.clone()).await;
            let _ = buffer_tx.send(reading).await;
        }
    });

    let http_state = state.clone();
    let server = HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(http_state.clone()))
            .route("/api/machines", web::get().to(get_machines))
            .route("/api/machines/{id}/health", web::get().to(get_machine_health))
            .route("/api/machines/{id}/sensors", web::get().to(get_machine_sensors))
            .route("/api/machines/{id}/rul", web::get().to(get_machine_rul))
            .route("/api/sensors/{machine_id}/{sensor_id}/waveform", web::get().to(get_sensor_waveform))
            .route("/api/sensors/{machine_id}/{sensor_id}/spectrum", web::get().to(get_sensor_spectrum))
            .route("/api/sensors/{machine_id}/{sensor_id}/trend", web::get().to(get_sensor_trend))
            .route("/api/alerts", web::get().to(get_alerts))
            .route("/api/machines/ranking", web::get().to(get_machine_ranking))
            .route("/api/statistics/faults", web::get().to(get_fault_statistics))
            .route("/api/maintenance-orders", web::post().to(create_maintenance_order))
    })
    .bind(format!("0.0.0.0:{}", HTTP_PORT))?
    .run();

    info!("HTTP server listening on port {}", HTTP_PORT);
    server.await?;

    Ok(())
}

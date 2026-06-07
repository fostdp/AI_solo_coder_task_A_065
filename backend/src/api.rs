use crate::config::Config;
use crate::models::*;
use crate::clickhouse_client::ClickHouseClient;
use axum::{
    extract::{Query, Path, State},
    routing::{get, post},
    Json, Router,
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use std::sync::Arc;
use tower_http::cors::{CorsLayer, Any};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub clickhouse: ClickHouseClient,
    pub machine_status_cache: Arc<dashmap::DashMap<u16, MachineStatus>>,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health_check))
        .route("/api/machines", get(get_all_machines))
        .route("/api/machines/:id", get(get_machine_detail))
        .route("/api/machines/:id/status", get(get_machine_status))
        .route("/api/machines/:id/sensors/:sensor_type/:sensor_id/history", get(get_sensor_history))
        .route("/api/sensors/positions", get(get_sensor_positions))
        .route("/api/alarms", get(get_recent_alarms))
        .route("/api/stats/monthly", get(get_monthly_stats))
        .route("/api/ranking", get(get_health_ranking))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state)
}

async fn health_check() -> &'static str {
    "Spindle Monitor API - OK"
}

#[derive(Debug, Deserialize)]
struct PaginationParams {
    limit: Option<u32>,
    offset: Option<u32>,
}

async fn get_all_machines(
    State(state): State<AppState>,
) -> Result<Json<Vec<MachineStatus>>, AppError> {
    let statuses: Vec<MachineStatus> = state.machine_status_cache
        .iter()
        .map(|entry| entry.value().clone())
        .collect();
    
    if statuses.is_empty() {
        match state.clickhouse.get_all_machine_status().await {
            Ok(s) => Ok(Json(s)),
            Err(e) => {
                let mut mock = Vec::new();
                for i in 1..=40 {
                    mock.push(MachineStatus {
                        machine_id: i,
                        health_score: 85.0 + (rand::random::<f64>() - 0.5) * 20.0,
                        rul_hours: 5000.0 + rand::random::<f64>() * 10000.0,
                        max_vibration_rms: 1.0 + rand::random::<f64>() * 3.0,
                        max_temperature: 35.0 + rand::random::<f64>() * 20.0,
                        alarm_status: if rand::random::<f64>() > 0.9 {
                            AlarmLevel::Warning
                        } else {
                            AlarmLevel::Normal
                        },
                        last_update: chrono::Utc::now(),
                    });
                }
                Ok(Json(mock))
            }
        }
    } else {
        Ok(Json(statuses))
    }
}

async fn get_machine_detail(
    State(state): State<AppState>,
    Path(id): Path<u16>,
) -> Result<Json<MachineStatus>, AppError> {
    if let Some(cached) = state.machine_status_cache.get(&id) {
        Ok(Json(cached.clone()))
    } else {
        Ok(Json(MachineStatus {
            machine_id: id,
            health_score: 85.0,
            rul_hours: 8000.0,
            max_vibration_rms: 2.0,
            max_temperature: 45.0,
            alarm_status: AlarmLevel::Normal,
            last_update: chrono::Utc::now(),
        }))
    }
}

async fn get_machine_status(
    State(state): State<AppState>,
    Path(id): Path<u16>,
) -> Result<Json<MachineStatus>, AppError> {
    get_machine_detail(State(state), Path(id)).await
}

#[derive(Debug, Deserialize)]
struct HistoryParams {
    hours: Option<u32>,
}

async fn get_sensor_history(
    State(state): State<AppState>,
    Path((id, sensor_type, sensor_id)): Path<(u16, String, u8)>,
    Query(params): Query<HistoryParams>,
) -> Result<Json<Vec<SensorHistoryPoint>>, AppError> {
    let hours = params.hours.unwrap_or(1);
    
    match state.clickhouse.get_sensor_history(id, &sensor_type, sensor_id, hours).await {
        Ok(data) => Ok(Json(data)),
        Err(_) => {
            let mut mock = Vec::new();
            let now = chrono::Utc::now();
            for i in 0..60 {
                mock.push(SensorHistoryPoint {
                    timestamp: klickhouse::DateTime64::new((now - chrono::Duration::minutes(i as i64)).timestamp_millis()),
                    value: 1.5 + (rand::random::<f64>() - 0.5) * 1.0,
                });
            }
            mock.reverse();
            Ok(Json(mock))
        }
    }
}

async fn get_sensor_positions(
    State(state): State<AppState>,
) -> Result<Json<Vec<SensorPosition>>, AppError> {
    match state.clickhouse.get_sensor_positions().await {
        Ok(positions) => Ok(Json(positions)),
        Err(_) => {
            let positions = vec![
                SensorPosition { id: 1, name: "前轴承径向X".to_string(), x: 120.0, y: 80.0, location: "前端轴承座".to_string() },
                SensorPosition { id: 2, name: "前轴承径向Y".to_string(), x: 120.0, y: 120.0, location: "前端轴承座".to_string() },
                SensorPosition { id: 3, name: "前轴承轴向".to_string(), x: 80.0, y: 100.0, location: "前端轴承座".to_string() },
                SensorPosition { id: 4, name: "中轴承径向X".to_string(), x: 250.0, y: 80.0, location: "中间支撑".to_string() },
                SensorPosition { id: 5, name: "中轴承径向Y".to_string(), x: 250.0, y: 120.0, location: "中间支撑".to_string() },
                SensorPosition { id: 6, name: "后轴承径向X".to_string(), x: 380.0, y: 80.0, location: "后端轴承座".to_string() },
                SensorPosition { id: 7, name: "后轴承径向Y".to_string(), x: 380.0, y: 120.0, location: "后端轴承座".to_string() },
                SensorPosition { id: 8, name: "刀柄位置".to_string(), x: 40.0, y: 100.0, location: "刀具接口".to_string() },
            ];
            Ok(Json(positions))
        }
    }
}

async fn get_recent_alarms(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<AlarmEvent>>, AppError> {
    let limit = params.limit.unwrap_or(20);
    match state.clickhouse.get_recent_alarms(limit).await {
        Ok(alarms) => Ok(Json(alarms)),
        Err(_) => {
            let mut mock = Vec::new();
            for i in 0..5 {
                mock.push(AlarmEvent {
                    id: format!("ALARM-{}", i),
                    timestamp: chrono::Utc::now() - chrono::Duration::hours(i as i64),
                    machine_id: (rand::random::<u16>() % 40) + 1,
                    sensor_type: if i % 2 == 0 { "vibration".to_string() } else { "rul".to_string() },
                    sensor_id: if i % 2 == 0 { Some((i % 8 + 1) as u8) } else { None },
                    level: if i < 2 { AlarmLevel::Critical } else { AlarmLevel::Warning },
                    message: format!("测试告警消息 {}", i),
                    value: 8.0 + rand::random::<f64>(),
                    threshold: 7.1,
                    acknowledged: false,
                });
            }
            Ok(Json(mock))
        }
    }
}

async fn get_monthly_stats(
    State(state): State<AppState>,
) -> Result<Json<MonthlyStats>, AppError> {
    match state.clickhouse.get_monthly_stats().await {
        Ok(stats) => Ok(Json(stats)),
        Err(_) => {
            Ok(Json(MonthlyStats {
                month: chrono::Local::now().format("%Y-%m").to_string(),
                total_alarms: 47,
                critical_alarms: 8,
                warning_alarms: 39,
                avg_health_score: 87.3,
                machines_maintained: 5,
            }))
        }
    }
}

async fn get_health_ranking(
    State(state): State<AppState>,
) -> Result<Json<Vec<MachineStatus>>, AppError> {
    let mut statuses: Vec<MachineStatus> = state.machine_status_cache
        .iter()
        .map(|entry| entry.value().clone())
        .collect();
    
    if statuses.is_empty() {
        for i in 1..=40 {
            statuses.push(MachineStatus {
                machine_id: i,
                health_score: 60.0 + rand::random::<f64>() * 40.0,
                rul_hours: 1000.0 + rand::random::<f64>() * 15000.0,
                max_vibration_rms: 0.5 + rand::random::<f64>() * 8.0,
                max_temperature: 30.0 + rand::random::<f64>() * 35.0,
                alarm_status: AlarmLevel::Normal,
                last_update: chrono::Utc::now(),
            });
        }
    }

    statuses.sort_by(|a, b| b.health_score.partial_cmp(&a.health_score).unwrap_or(std::cmp::Ordering::Equal));
    Ok(Json(statuses))
}

pub struct AppError(Box<dyn std::error::Error>);

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Internal server error: {}", self.0),
        ).into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<Box<dyn std::error::Error>>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

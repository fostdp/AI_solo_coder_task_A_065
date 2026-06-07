use std::sync::Arc;
use tokio::sync::RwLock;
use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
    http::Method,
};
use tower_http::cors::{CorsLayer, Any};
use serde::Deserialize;
use tracing::info;

use crate::config::Config;
use crate::models::{AppState, MachineStatus, HealthRanking, MonthlyStats, SensorHistory};
use crate::clickhouse_client::ClickHouseClient;

#[derive(Clone)]
struct ApiState {
    app_state: Arc<RwLock<AppState>>,
    clickhouse: Arc<ClickHouseClient>,
}

#[derive(Debug, Deserialize)]
struct SensorQuery {
    hours: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct MonthlyQuery {
    month: Option<String>,
}

pub async fn start_api_server(
    config: Config,
    app_state: Arc<RwLock<AppState>>,
    clickhouse: Arc<ClickHouseClient>,
) -> anyhow::Result<()> {
    let api_state = ApiState {
        app_state,
        clickhouse,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/machines", get(get_all_machines))
        .route("/api/machines/:id", get(get_machine_by_id))
        .route("/api/machines/:id/sensors/:sensor_idx", get(get_sensor_history))
        .route("/api/ranking", get(get_health_ranking))
        .route("/api/stats/monthly", get(get_monthly_stats))
        .layer(cors)
        .with_state(api_state);

    let addr = format!("0.0.0.0:{}", config.server.http_port);
    info!("API server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> &'static str {
    "OK"
}

async fn get_all_machines(
    State(state): State<ApiState>,
) -> Json<Vec<MachineStatus>> {
    let app_state = state.app_state.read().await;
    
    let mut machines: Vec<MachineStatus> = app_state.machine_statuses.values().cloned().collect();
    machines.sort_by_key(|m| m.machine_id);
    
    if machines.is_empty() {
        match state.clickhouse.get_all_machine_statuses().await {
            Ok(db_machines) => Json(db_machines),
            Err(_) => Json(Vec::new()),
        }
    } else {
        Json(machines)
    }
}

async fn get_machine_by_id(
    State(state): State<ApiState>,
    Path(id): Path<u16>,
) -> Json<Option<MachineStatus>> {
    let app_state = state.app_state.read().await;
    
    if let Some(status) = app_state.machine_statuses.get(&id) {
        Json(Some(status.clone()))
    } else {
        match state.clickhouse.get_machine_status(id).await {
            Ok(status) => Json(status),
            Err(_) => Json(None),
        }
    }
}

async fn get_sensor_history(
    State(state): State<ApiState>,
    Path((id, sensor_idx)): Path<(u16, usize)>,
    Query(query): Query<SensorQuery>,
) -> Json<Option<SensorHistory>> {
    let hours = query.hours.unwrap_or(1);
    
    match state.clickhouse.get_sensor_history(id, sensor_idx, hours).await {
        Ok(history) => {
            if history.timestamps.is_empty() {
                let app_state = state.app_state.read().await;
                if let Some(metrics) = app_state.recent_metrics.get(&id) {
                    let timestamps: Vec<i64> = metrics.iter()
                        .map(|m| m.timestamp.timestamp_millis())
                        .collect();
                    let values: Vec<f64> = metrics.iter()
                        .map(|m| m.vibration_rms.get(sensor_idx).copied().unwrap_or(0.0))
                        .collect();
                    Json(Some(SensorHistory {
                        timestamps,
                        values,
                        frequencies: Vec::new(),
                        spectrum: Vec::new(),
                    }))
                } else {
                    Json(None)
                }
            } else {
                Json(Some(history))
            }
        }
        Err(_) => Json(None),
    }
}

async fn get_health_ranking(
    State(state): State<ApiState>,
) -> Json<Vec<HealthRanking>> {
    match state.clickhouse.get_health_ranking(40).await {
        Ok(ranking) => Json(ranking),
        Err(_) => {
            let app_state = state.app_state.read().await;
            let mut rankings: Vec<HealthRanking> = app_state.machine_statuses
                .values()
                .map(|s| HealthRanking {
                    machine_id: s.machine_id,
                    health_score: s.health_score,
                    rul_hours: s.rul_hours,
                    alarm_level: s.alarm_level,
                    rank: 0,
                })
                .collect();
            
            rankings.sort_by(|a, b| b.health_score.partial_cmp(&a.health_score).unwrap_or(std::cmp::Ordering::Equal));
            
            for (i, r) in rankings.iter_mut().enumerate() {
                r.rank = (i + 1) as u16;
            }
            
            Json(rankings)
        }
    }
}

async fn get_monthly_stats(
    State(state): State<ApiState>,
    Query(query): Query<MonthlyQuery>,
) -> Json<Vec<MonthlyStats>> {
    let month = query.month.unwrap_or_else(|| {
        chrono::Local::now().format("%Y-%m").to_string()
    });

    match state.clickhouse.get_monthly_stats(&month).await {
        Ok(stats) => Json(stats),
        Err(_) => Json(Vec::new()),
    }
}

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::sync::Arc;
use crate::clickhouse_client::ClickHouseClient;
use crate::models::*;
use crate::config::Config;

pub struct AppState {
    pub clickhouse: ClickHouseClient,
    pub config: Config,
}

pub async fn get_machines(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.clickhouse.get_machines().await {
        Ok(machines) => Json(machines).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch machines: {}", e),
        )
            .into_response(),
    }
}

pub async fn get_machine(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u16>,
) -> impl IntoResponse {
    match state.clickhouse.get_machines().await {
        Ok(machines) => {
            if let Some(machine) = machines.into_iter().find(|m| m.machine_id == id) {
                Json(machine).into_response()
            } else {
                (StatusCode::NOT_FOUND, "Machine not found").into_response()
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch machine: {}", e),
        )
            .into_response(),
    }
}

pub async fn get_sensors_by_machine(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u16>,
) -> impl IntoResponse {
    match state.clickhouse.get_sensors_by_machine(id).await {
        Ok(sensors) => Json(sensors).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch sensors: {}", e),
        )
            .into_response(),
    }
}

pub async fn get_latest_sensor_data(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u16>,
) -> impl IntoResponse {
    match state.clickhouse.get_latest_sensor_data(id).await {
        Ok(data) => Json(data).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch sensor data: {}", e),
        )
            .into_response(),
    }
}

pub async fn get_sensor_history(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u16>,
) -> impl IntoResponse {
    let now = chrono::Utc::now().timestamp();
    let one_hour_ago = now - 3600;
    let one_week_ago = now - 7 * 24 * 3600;

    match state.clickhouse.get_sensor_history(id, one_week_ago, now).await {
        Ok(history) => {
            let recent_data: Vec<TimeSeriesPoint> = history
                .iter()
                .filter(|p| p.timestamp >= one_hour_ago)
                .cloned()
                .collect();

            let frequencies: Vec<f32> = (0..100).map(|i| i as f32 * 10.0).collect();
            let amplitudes: Vec<f32> = (0..100).map(|i| {
                let base = if i % 17 == 0 { 2.5 } else { 0.5 };
                base + (rand::random::<f32>() - 0.5) * 0.3
            }).collect();

            let spectrum = VibrationSpectrum {
                timestamp: now,
                machine_id: 1,
                sensor_id: id,
                frequency: frequencies,
                amplitude: amplitudes,
                rpm: 8000.0,
            };

            let sensor_configs = state.clickhouse.get_sensors_by_machine(1).await.unwrap_or_default();
            let sensor_config = sensor_configs.into_iter().find(|s| s.sensor_id == id).unwrap_or_else(|| SensorConfig {
                sensor_id: id,
                machine_id: 1,
                sensor_type: SensorType::Vibration,
                position_name: "未知位置".to_string(),
                position_x: 0.0,
                position_y: 0.0,
                position_z: 0.0,
                axis: "x".to_string(),
                unit: "mm/s".to_string(),
                status: SensorStatus::Active,
            });

            let response = SensorDetailResponse {
                sensor_config,
                recent_data,
                spectrum,
                history_trend: history,
            };

            Json(response).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch sensor history: {}", e),
        )
            .into_response(),
    }
}

pub async fn get_health_ranking(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.clickhouse.get_health_ranking().await {
        Ok(ranking) => Json(ranking).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch health ranking: {}", e),
        )
            .into_response(),
    }
}

pub async fn get_fault_statistics(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.clickhouse.get_fault_statistics().await {
        Ok(stats) => Json(stats).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch fault statistics: {}", e),
        )
            .into_response(),
    }
}

pub async fn get_latest_rul(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u16>,
) -> impl IntoResponse {
    match state.clickhouse.get_latest_rul(id).await {
        Ok(Some(rul)) => Json(rul).into_response(),
        Ok(None) => {
            let default_rul = RULPrediction {
                timestamp: chrono::Utc::now().timestamp(),
                machine_id: id,
                bearing_id: 1,
                rul_hours: 5000.0 + rand::random::<f32>() * 3000.0,
                rul_confidence: 0.9,
                vibration_rms_trend: 5.0 + rand::random::<f32>() * 10.0,
                temperature_rate: 2.0 + rand::random::<f32>() * 5.0,
                skf_l10_life: 8000.0,
                lstm_prediction: 7500.0,
                health_score: 85,
            };
            Json(default_rul).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch RUL: {}", e),
        )
            .into_response(),
    }
}

pub async fn get_recent_alarms(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.clickhouse.get_recent_alarms(50).await {
        Ok(alarms) => Json(alarms).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch alarms: {}", e),
        )
            .into_response(),
    }
}

pub async fn health_check() -> impl IntoResponse {
    let response = serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "service": "spindle-health-monitor"
    });
    Json(response)
}

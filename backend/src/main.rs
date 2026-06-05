mod config;
mod models;
mod clickhouse_client;
mod udp_server;
mod prediction;
mod alarm;
mod mqtt_client;
mod handlers;

use std::sync::Arc;
use tokio;
use axum::{
    routing::get,
    Router,
    http::Method,
};
use tower_http::cors::{CorsLayer, Any};
use log::info;

use config::Config;
use clickhouse_client::ClickHouseClient;
use prediction::PredictionEngine;
use alarm::AlarmManager;
use mqtt_client::MQTTClient;
use udp_server::UDPServer;
use handlers::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    
    let config = Config::from_env();
    info!("Starting spindle health monitoring system...");
    
    let clickhouse = ClickHouseClient::new(&config).await?;
    info!("Connected to ClickHouse");
    
    let prediction_engine = Arc::new(PredictionEngine::new(clickhouse.clone()));
    info!("Prediction engine initialized");
    
    let mqtt_client = MQTTClient::new(&config).await.map(Arc::new);
    info!("MQTT client initialized");
    
    let alarm_manager = Arc::new(AlarmManager::new(
        config.clone(),
        clickhouse.clone(),
        mqtt_client.clone(),
    ));
    info!("Alarm manager initialized");
    
    let udp_server = UDPServer::new(
        config.clone(),
        clickhouse.clone(),
        prediction_engine.clone(),
        alarm_manager.clone(),
    );
    
    let udp_handle = tokio::spawn(async move {
        if let Err(e) = udp_server.run().await {
            log::error!("UDP server error: {}", e);
        }
    });
    
    let app_state = Arc::new(AppState {
        clickhouse: clickhouse.clone(),
        config: config.clone(),
    });
    
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(Any);
    
    let app = Router::new()
        .route("/api/health", get(handlers::health_check))
        .route("/api/machines", get(handlers::get_machines))
        .route("/api/machines/:id", get(handlers::get_machine))
        .route("/api/machines/:id/sensors", get(handlers::get_sensors_by_machine))
        .route("/api/machines/:id/sensors/data", get(handlers::get_latest_sensor_data))
        .route("/api/sensors/:id/history", get(handlers::get_sensor_history))
        .route("/api/ranking", get(handlers::get_health_ranking))
        .route("/api/statistics", get(handlers::get_fault_statistics))
        .route("/api/machines/:id/rul", get(handlers::get_latest_rul))
        .route("/api/alarms", get(handlers::get_recent_alarms))
        .with_state(app_state)
        .layer(cors);
    
    let addr = format!("0.0.0.0:{}", config.http_port);
    info!("HTTP server listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let http_handle = axum::serve(listener, app);
    
    info!("System started successfully!");
    
    tokio::select! {
        _ = http_handle => {},
        _ = udp_handle => {},
    }
    
    Ok(())
}

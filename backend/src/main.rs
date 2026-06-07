mod config;
mod models;
mod udp_server;
mod clickhouse_client;
mod signal_processing;
mod rul_predictor;
mod alarm_manager;
mod mqtt_client;
mod api_server;
mod websocket_server;

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};
use tracing_subscriber::EnvFilter;

use config::Config;
use models::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    info!("Starting CNC Spindle Monitor Backend...");

    let config = Config::load("config.toml")?;
    info!("Configuration loaded successfully");

    let clickhouse = Arc::new(clickhouse_client::ClickHouseClient::new(&config).await?);
    info!("ClickHouse client initialized");

    let mqtt = Arc::new(mqtt_client::MqttClient::new(&config).await?);
    info!("MQTT client initialized");

    let signal_processor = Arc::new(signal_processing::SignalProcessor::new());
    let rul_predictor = Arc::new(rul_predictor::RULPredictor::new(&config));
    let alarm_manager = Arc::new(alarm_manager::AlarmManager::new(&config, mqtt.clone(), clickhouse.clone()));

    let app_state = Arc::new(RwLock::new(AppState {
        config: config.clone(),
        machine_statuses: std::collections::HashMap::new(),
        recent_metrics: std::collections::HashMap::new(),
    }));

    let udp_handle = tokio::spawn(udp_server::start_udp_server(
        config.clone(),
        app_state.clone(),
        clickhouse.clone(),
        signal_processor.clone(),
        alarm_manager.clone(),
    ));

    let rul_handle = tokio::spawn(rul_predictor::start_rul_prediction_loop(
        config.clone(),
        app_state.clone(),
        clickhouse.clone(),
        rul_predictor.clone(),
        alarm_manager.clone(),
    ));

    let api_handle = tokio::spawn(api_server::start_api_server(
        config.clone(),
        app_state.clone(),
        clickhouse.clone(),
    ));

    let ws_handle = tokio::spawn(websocket_server::start_websocket_server(
        config.clone(),
        app_state.clone(),
    ));

    info!("All services started successfully");

    tokio::select! {
        result = udp_handle => {
            if let Err(e) = result {
                error!("UDP server error: {}", e);
            }
        }
        result = rul_handle => {
            if let Err(e) = result {
                error!("RUL prediction loop error: {}", e);
            }
        }
        result = api_handle => {
            if let Err(e) = result {
                error!("API server error: {}", e);
            }
        }
        result = ws_handle => {
            if let Err(e) = result {
                error!("WebSocket server error: {}", e);
            }
        }
    }

    Ok(())
}

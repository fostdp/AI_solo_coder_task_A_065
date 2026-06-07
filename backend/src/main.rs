mod config;
mod models;
mod ethercat_driver;
mod vibration_analyzer;
mod rul_predictor;
mod alarm_dispatcher;
mod clickhouse_client;
mod mqtt_client;
mod api_server;
mod websocket_server;
mod iso22400_adapter;
mod metrics;

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};
use tracing_subscriber::EnvFilter;

use config::Config;
use models::{AppState, AnalyzedMetrics, RULPredictionResult, MachineStatus};
use ethercat_driver::EthercatDriver;
use vibration_analyzer::VibrationAnalyzer;
use rul_predictor::RULPredictor;
use alarm_dispatcher::{AlarmDispatcher, AlarmInput};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    info!("Starting CNC Spindle Monitor Backend (Modular Architecture)...");
    info!("Module Architecture: EtherCAT Driver → Vibration Analyzer → RUL Predictor → Alarm Dispatcher");

    let config = Config::load("config.toml")?;
    info!("Configuration loaded successfully");

    let clickhouse = Arc::new(clickhouse_client::ClickHouseClient::new(&config).await?);
    info!("ClickHouse client initialized");

    let mqtt = Arc::new(mqtt_client::MqttClient::new(&config).await?);
    info!("MQTT client initialized");

    let app_state = Arc::new(RwLock::new(AppState {
        config: config.clone(),
        machine_statuses: std::collections::HashMap::new(),
        recent_metrics: std::collections::HashMap::new(),
    }));

    info!("Building module pipeline...");

    let (ethercat_driver, raw_data_rx) = EthercatDriver::new(&config);
    info!("  [1/4] EtherCAT Driver ready - UDP port: {}", config.server.udp_port);

    let (_, vibration_tx, analyzed_rx) = VibrationAnalyzer::new(&config);
    info!("  [2/4] Vibration Analyzer ready - FFT + Severity calculation");

    let (_, rul_tx, prediction_rx) = RULPredictor::new(&config);
    info!("  [3/4] RUL Predictor ready - SKF + LSTM hybrid model");

    let (alarm_dispatcher, alarm_tx) = AlarmDispatcher::new(&config, mqtt.clone(), clickhouse.clone());
    info!("  [4/4] Alarm Dispatcher ready - ISO 22400 + MQTT push");

    info!("Connecting module pipelines with tokio mpsc channels...");

    let ethercat_handle = tokio::spawn(async move {
        if let Err(e) = ethercat_driver.start().await {
            error!("EtherCAT Driver error: {}", e);
        }
    });

    let pipeline1_handle = tokio::spawn(async move {
        let mut rx = raw_data_rx;
        let tx = vibration_tx;
        while let Some(data) = rx.recv().await {
            if let Err(e) = tx.send(data).await {
                error!("Pipeline [1→2] error: {}", e);
            }
        }
    });

    let pipeline2_handle = tokio::spawn(async move {
        let mut rx = analyzed_rx;
        let tx = rul_tx;
        while let Some(metrics) = rx.recv().await {
            if let Err(e) = tx.send(metrics.clone()).await {
                error!("Pipeline [2→3] error: {}", e);
            }
            
            let alarm_input = AlarmInput::VibrationMetrics(metrics);
            if let Err(e) = alarm_tx.send(alarm_input).await {
                error!("Pipeline [2→4] error: {}", e);
            }
        }
    });

    let pipeline3_handle = tokio::spawn(async move {
        let mut rx = prediction_rx;
        let tx_clone = alarm_tx.clone();
        while let Some(prediction) = rx.recv().await {
            let alarm_input = AlarmInput::RULPrediction(prediction);
            if let Err(e) = tx_clone.send(alarm_input).await {
                error!("Pipeline [3→4] error: {}", e);
            }
        }
    });

    let rul_update_handle = tokio::spawn(rul_predictor::start_rul_prediction_loop(
        config.clone(),
        app_state.clone(),
        clickhouse.clone(),
    ));

    let status_sync_handle = {
        let app_state = app_state.clone();
        let config = config.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                
                let state = app_state.read().await;
                for status in state.machine_statuses.values() {
                    alarm_dispatcher.publish_status_update(status).await;
                }
            }
        })
    };

    let api_handle = tokio::spawn(api_server::start_api_server(
        config.clone(),
        app_state.clone(),
        clickhouse.clone(),
    ));

    let ws_handle = tokio::spawn(websocket_server::start_websocket_server(
        config.clone(),
        app_state.clone(),
    ));

    let metrics_handle = tokio::spawn(async move {
        if let Err(e) = metrics::start_metrics_server(9090).await {
            error!("Metrics server error: {}", e);
        }
    });

    info!("All modules connected and running!");
    info!("Pipeline: UDP → EtherCAT Driver → Vibration Analyzer → RUL Predictor → Alarm Dispatcher → MQTT/MES");
    info!("Metrics endpoint: http://0.0.0.0:9090/metrics");

    tokio::select! {
        result = ethercat_handle => {
            if let Err(e) = result {
                error!("EtherCAT driver task error: {}", e);
            }
        }
        result = pipeline1_handle => {
            if let Err(e) = result {
                error!("Pipeline 1 task error: {}", e);
            }
        }
        result = pipeline2_handle => {
            if let Err(e) = result {
                error!("Pipeline 2 task error: {}", e);
            }
        }
        result = pipeline3_handle => {
            if let Err(e) = result {
                error!("Pipeline 3 task error: {}", e);
            }
        }
        result = rul_update_handle => {
            if let Err(e) = result {
                error!("RUL update task error: {}", e);
            }
        }
        result = status_sync_handle => {
            if let Err(e) = result {
                error!("Status sync task error: {}", e);
            }
        }
        result = api_handle => {
            if let Err(e) = result {
                error!("API server task error: {}", e);
            }
        }
        result = ws_handle => {
            if let Err(e) = result {
                error!("WebSocket server task error: {}", e);
            }
        }
        result = metrics_handle => {
            if let Err(e) = result {
                error!("Metrics server task error: {}", e);
            }
        }
    }

    Ok(())
}

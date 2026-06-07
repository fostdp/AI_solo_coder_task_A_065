mod config;
mod models;
mod udp_server;
mod clickhouse_client;
mod rul_predictor;
mod alarm_engine;
mod api;
mod data_processor;

use config::Config;
use udp_server::UdpServer;
use clickhouse_client::ClickHouseClient;
use rul_predictor::RULPredictor;
use alarm_engine::AlarmEngine;
use api::{AppState, create_router};
use data_processor::DataProcessor;

use std::sync::Arc;
use tokio::sync::mpsc;
use log::{info, warn, error};
use dashmap::DashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    info!("========================================");
    info!("精密数控机床主轴健康监控与剩余寿命预测系统");
    info!("后端服务启动中...");
    info!("========================================");

    let config = Arc::new(Config::default());
    info!("配置加载完成: UDP端口={}, API端口={}", config.udp_port, config.api_port);

    let status_cache = Arc::new(DashMap::<u16, models::MachineStatus>::new());

    let (data_sender, data_receiver) = mpsc::channel::<models::SensorData>(10000);

    let clickhouse = match ClickHouseClient::new(config.clone()).await {
        Ok(client) => {
            info!("ClickHouse连接成功");
            client
        }
        Err(e) => {
            warn!("ClickHouse连接失败，将使用内存模式: {}", e);
            return Err(e);
        }
    };

    let rul_predictor = RULPredictor::new(config.clone());
    info!("RUL预测模块初始化完成");

    let mut alarm_engine = AlarmEngine::new(config.clone());
    #[cfg(feature = "mqtt")]
    {
        if let Err(e) = alarm_engine.init_mqtt().await {
            warn!("MQTT初始化失败: {}", e);
        }
    }
    info!("告警引擎初始化完成");

    let data_processor = DataProcessor::new(
        config.clone(),
        clickhouse.clone(),
        rul_predictor,
        alarm_engine,
        status_cache.clone(),
    );

    let udp_server = UdpServer::new(config.clone(), data_sender);
    info!("UDP服务器初始化完成");

    let app_state = AppState {
        config: config.clone(),
        clickhouse: clickhouse.clone(),
        machine_status_cache: status_cache,
    };

    let router = create_router(app_state);
    let api_addr = format!("0.0.0.0:{}", config.api_port);
    info!("REST API 服务将监听: {}", api_addr);

    let processor_handle = tokio::spawn(data_processor.run(data_receiver));
    let udp_handle = tokio::spawn(async move {
        if let Err(e) = udp_server.run().await {
            error!("UDP服务器错误: {}", e);
        }
    });

    let api_handle = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(&api_addr).await.unwrap();
        info!("API服务器已启动: http://{}", api_addr);
        axum::serve(listener, router).await.unwrap();
    });

    info!("所有服务已启动，正在运行...");

    tokio::select! {
        _ = processor_handle => {
            error!("数据处理器已退出");
        }
        _ = udp_handle => {
            error!("UDP服务器已退出");
        }
        _ = api_handle => {
            error!("API服务器已退出");
        }
    }

    Ok(())
}

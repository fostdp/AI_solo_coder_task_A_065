use std::sync::Arc;
use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
use once_cell::sync::Lazy;
use prometheus::{
    register_counter, register_gauge, register_histogram,
    Counter, Gauge, Histogram, Encoder, TextEncoder,
    Opts, Registry,
};

pub static REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);

pub static UDP_PACKETS_RECEIVED: Lazy<Counter> = Lazy::new(|| {
    let opts = Opts::new(
        "cnc_udp_packets_received_total",
        "Total number of UDP packets received from EtherCAT"
    ).namespace("cnc_monitor");
    let counter = Counter::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(counter.clone())).unwrap();
    counter
});

pub static UDP_BYTES_RECEIVED: Lazy<Counter> = Lazy::new(|| {
    let opts = Opts::new(
        "cnc_udp_bytes_received_total",
        "Total bytes received via UDP"
    ).namespace("cnc_monitor");
    let counter = Counter::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(counter.clone())).unwrap();
    counter
});

pub static UDP_PACKET_ERRORS: Lazy<Counter> = Lazy::new(|| {
    let opts = Opts::new(
        "cnc_udp_packet_errors_total",
        "Total UDP packet parsing errors"
    ).namespace("cnc_monitor");
    let counter = Counter::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(counter.clone())).unwrap();
    counter
});

pub static ACTIVE_MACHINES: Lazy<Gauge> = Lazy::new(|| {
    let opts = Opts::new(
        "cnc_active_machines",
        "Number of actively reporting machines"
    ).namespace("cnc_monitor");
    let gauge = Gauge::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(gauge.clone())).unwrap();
    gauge
});

pub static VIBRATION_ALARMS: Lazy<Counter> = Lazy::new(|| {
    let opts = Opts::new(
        "cnc_vibration_alarms_total",
        "Total vibration alarms triggered"
    ).namespace("cnc_monitor");
    let counter = Counter::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(counter.clone())).unwrap();
    counter
});

pub static RUL_ALARMS: Lazy<Counter> = Lazy::new(|| {
    let opts = Opts::new(
        "cnc_rul_alarms_total",
        "Total RUL critical alarms triggered"
    ).namespace("cnc_monitor");
    let counter = Counter::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(counter.clone())).unwrap();
    counter
});

pub static FFT_PROCESSING_TIME: Lazy<Histogram> = Lazy::new(|| {
    let opts = Opts::new(
        "cnc_fft_processing_seconds",
        "Time spent processing FFT analysis"
    ).namespace("cnc_monitor");
    let histogram = Histogram::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(histogram.clone())).unwrap();
    histogram
});

pub static RUL_PREDICTION_TIME: Lazy<Histogram> = Lazy::new(|| {
    let opts = Opts::new(
        "cnc_rul_prediction_seconds",
        "Time spent running RUL prediction"
    ).namespace("cnc_monitor");
    let histogram = Histogram::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(histogram.clone())).unwrap();
    histogram
});

pub static CLICKHOUSE_INSERTS: Lazy<Counter> = Lazy::new(|| {
    let opts = Opts::new(
        "cnc_clickhouse_inserts_total",
        "Total ClickHouse insert operations"
    ).namespace("cnc_monitor");
    let counter = Counter::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(counter.clone())).unwrap();
    counter
});

pub static CLICKHOUSE_INSERT_ERRORS: Lazy<Counter> = Lazy::new(|| {
    let opts = Opts::new(
        "cnc_clickhouse_insert_errors_total",
        "Total ClickHouse insert errors"
    ).namespace("cnc_monitor");
    let counter = Counter::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(counter.clone())).unwrap();
    counter
});

pub static MQTT_MESSAGES_SENT: Lazy<Counter> = Lazy::new(|| {
    let opts = Opts::new(
        "cnc_mqtt_messages_sent_total",
        "Total MQTT messages published"
    ).namespace("cnc_monitor");
    let counter = Counter::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(counter.clone())).unwrap();
    counter
});

pub static MQTT_MESSAGE_ERRORS: Lazy<Counter> = Lazy::new(|| {
    let opts = Opts::new(
        "cnc_mqtt_message_errors_total",
        "Total MQTT publish errors"
    ).namespace("cnc_monitor");
    let counter = Counter::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(counter.clone())).unwrap();
    counter
});

pub static AVG_HEALTH_SCORE: Lazy<Gauge> = Lazy::new(|| {
    let opts = Opts::new(
        "cnc_avg_health_score",
        "Average health score across all machines"
    ).namespace("cnc_monitor");
    let gauge = Gauge::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(gauge.clone())).unwrap();
    gauge
});

pub static WEBSOCKET_CONNECTIONS: Lazy<Gauge> = Lazy::new(|| {
    let opts = Opts::new(
        "cnc_websocket_connections",
        "Current WebSocket client connections"
    ).namespace("cnc_monitor");
    let gauge = Gauge::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(gauge.clone())).unwrap();
    gauge
});

pub static PIPELINE_CHANNEL_DEPTH: Lazy<Gauge> = Lazy::new(|| {
    let opts = Opts::new(
        "cnc_pipeline_channel_depth",
        "Current depth of each pipeline channel"
    ).namespace("cnc_monitor");
    let gauge = Gauge::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(gauge.clone())).unwrap();
    gauge
});

pub async fn start_metrics_server(port: u16) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler));

    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("Metrics server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn metrics_handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();

    match encoder.encode(&metric_families, &mut buffer) {
        Ok(_) => (
            StatusCode::OK,
            [("Content-Type", encoder.format_type())],
            buffer
        ).into_response(),
        Err(e) => {
            tracing::error!("Failed to encode metrics: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to encode metrics"
            ).into_response()
        }
    }
}

async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "OK").into_response()
}

#[inline]
pub fn increment_udp_packets() {
    UDP_PACKETS_RECEIVED.inc();
}

#[inline]
pub fn add_udp_bytes(bytes: u64) {
    UDP_BYTES_RECEIVED.inc_by(bytes as f64);
}

#[inline]
pub fn increment_udp_errors() {
    UDP_PACKET_ERRORS.inc();
}

#[inline]
pub fn set_active_machines(count: f64) {
    ACTIVE_MACHINES.set(count);
}

#[inline]
pub fn increment_vibration_alarms() {
    VIBRATION_ALARMS.inc();
}

#[inline]
pub fn increment_rul_alarms() {
    RUL_ALARMS.inc();
}

#[inline]
pub fn observe_fft_time(seconds: f64) {
    FFT_PROCESSING_TIME.observe(seconds);
}

#[inline]
pub fn observe_rul_time(seconds: f64) {
    RUL_PREDICTION_TIME.observe(seconds);
}

#[inline]
pub fn increment_clickhouse_inserts() {
    CLICKHOUSE_INSERTS.inc();
}

#[inline]
pub fn increment_clickhouse_errors() {
    CLICKHOUSE_INSERT_ERRORS.inc();
}

#[inline]
pub fn increment_mqtt_sent() {
    MQTT_MESSAGES_SENT.inc();
}

#[inline]
pub fn increment_mqtt_errors() {
    MQTT_MESSAGE_ERRORS.inc();
}

#[inline]
pub fn set_avg_health_score(score: f64) {
    AVG_HEALTH_SCORE.set(score);
}

#[inline]
pub fn set_websocket_connections(count: f64) {
    WEBSOCKET_CONNECTIONS.set(count);
}

#[inline]
pub fn set_channel_depth(depth: f64) {
    PIPELINE_CHANNEL_DEPTH.set(depth);
}

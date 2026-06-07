-- CNC Spindle Monitor Database Schema
-- ClickHouse initialization with partitioning and TTL

CREATE DATABASE IF NOT EXISTS cnc_monitor;

USE cnc_monitor;

-- Raw sensor data table (time-series, partitioned by day, TTL 30 days)
CREATE TABLE IF NOT EXISTS sensor_raw (
    timestamp DateTime64(6, 'Asia/Shanghai'),
    machine_id UInt16,
    sensor_type Enum8('vibration'=1, 'temperature'=2, 'displacement'=3),
    sensor_index UInt8,
    value Float32,
    sample_index UInt8
) ENGINE = MergeTree()
PARTITION BY toYYYYMMDD(timestamp)
ORDER BY (machine_id, sensor_type, sensor_index, timestamp)
TTL toDateTime(timestamp) + INTERVAL 30 DAY
SETTINGS index_granularity = 8192;

-- Aggregated sensor metrics (1 minute rollup, TTL 1 year)
CREATE TABLE IF NOT EXISTS sensor_metrics_1m (
    timestamp DateTime('Asia/Shanghai'),
    machine_id UInt16,
    sensor_type Enum8('vibration'=1, 'temperature'=2, 'displacement'=3),
    sensor_index UInt8,
    min_val Float32,
    max_val Float32,
    avg_val Float32,
    std_val Float32,
    rms_val Float32,
    peak_val Float32,
    sample_count UInt32
) ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, sensor_type, sensor_index, timestamp)
TTL toDateTime(timestamp) + INTERVAL 12 MONTH
SETTINGS index_granularity = 4096;

-- Vibration severity table (processed metrics, TTL 1 year)
CREATE TABLE IF NOT EXISTS vibration_severity (
    timestamp DateTime64(3, 'Asia/Shanghai'),
    machine_id UInt16,
    sensor_index UInt8,
    rms Float32,
    peak Float32,
    crest_factor Float32,
    kurtosis Float32,
    skewness Float32,
    severity_level Enum8('normal'=0, 'warning'=1, 'alarm'=2),
    rpm UInt16,
    condition Enum8('low'=0, 'medium'=1, 'high'=2, 'unknown'=3)
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, sensor_index, timestamp)
TTL toDateTime(timestamp) + INTERVAL 12 MONTH
SETTINGS index_granularity = 8192;

-- FFT spectrum data (frequency domain, TTL 90 days)
CREATE TABLE IF NOT EXISTS fft_spectrum (
    timestamp DateTime64(3, 'Asia/Shanghai'),
    machine_id UInt16,
    sensor_index UInt8,
    frequency_bin UInt16,
    amplitude Float32,
    phase Float32
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, sensor_index, timestamp, frequency_bin)
TTL toDateTime(timestamp) + INTERVAL 90 DAY
SETTINGS index_granularity = 16384;

-- RUL prediction results (TLL 3 years)
CREATE TABLE IF NOT EXISTS rul_predictions (
    timestamp DateTime64(3, 'Asia/Shanghai'),
    machine_id UInt16,
    rul_hours Float64,
    rul_confidence Float32,
    skf_rul_hours Float64,
    lstm_rul_hours Float64,
    health_score Float32,
    operating_condition Enum8('low'=0, 'medium'=1, 'high'=2, 'unknown'=3),
    feature_vector Array(Float32)
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, timestamp)
TTL toDateTime(timestamp) + INTERVAL 36 MONTH
SETTINGS index_granularity = 1024;

-- Alarm events table (TTL 5 years)
CREATE TABLE IF NOT EXISTS alarms (
    timestamp DateTime64(3, 'Asia/Shanghai'),
    machine_id UInt16,
    alarm_type String,
    alarm_level UInt8,
    message String,
    sensor_index UInt8,
    value Float64,
    threshold Float64,
    acknowledged UInt8 DEFAULT 0,
    acknowledged_at DateTime64(3) DEFAULT NULL,
    acknowledged_by String DEFAULT ''
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, alarm_level, timestamp)
TTL toDateTime(timestamp) + INTERVAL 60 MONTH
SETTINGS index_granularity = 512;

-- Machine status snapshots (1 minute, TTL 90 days)
CREATE TABLE IF NOT EXISTS machine_status (
    timestamp DateTime('Asia/Shanghai'),
    machine_id UInt16,
    health_score Float32,
    rul_hours Float64,
    avg_vibration Float32,
    max_vibration Float32,
    avg_temperature Float32,
    max_temperature Float32,
    alarm_level UInt8,
    runtime_hours Float64,
    rpm UInt16
) ENGINE = ReplacingMergeTree(timestamp)
PARTITION BY toYYYYMMDD(timestamp)
ORDER BY (machine_id, timestamp)
TTL toDateTime(timestamp) + INTERVAL 90 DAY
SETTINGS index_granularity = 1024;

-- ISO 22400 message log (TLL 1 year)
CREATE TABLE IF NOT EXISTS iso22400_messages (
    timestamp DateTime64(3, 'Asia/Shanghai'),
    message_id UUID,
    message_type Enum8('Alarm'=1, 'KPI'=2, 'EquipmentStatus'=3),
    machine_id UInt16,
    payload String,
    mqtt_topic String,
    qos UInt8,
    delivery_status Enum8('pending'=0, 'sent'=1, 'failed'=2)
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (message_type, machine_id, timestamp)
TTL toDateTime(timestamp) + INTERVAL 12 MONTH
SETTINGS index_granularity = 2048;

-- Materialized view for 1-minute aggregation
CREATE MATERIALIZED VIEW IF NOT EXISTS sensor_metrics_1m_mv
TO sensor_metrics_1m
AS
SELECT
    toStartOfMinute(timestamp) AS timestamp,
    machine_id,
    sensor_type,
    sensor_index,
    min(value) AS min_val,
    max(value) AS max_val,
    avg(value) AS avg_val,
    stddevPop(value) AS std_val,
    sqrt(avg(value * value)) AS rms_val,
    max(abs(value)) AS peak_val,
    count() AS sample_count
FROM sensor_raw
GROUP BY
    timestamp,
    machine_id,
    sensor_type,
    sensor_index;

-- Create dictionary for machine metadata
CREATE DICTIONARY IF NOT EXISTS machine_metadata (
    machine_id UInt16,
    model String,
    location String,
    install_date Date,
    rated_rpm UInt16,
    bearing_type String
)
PRIMARY KEY machine_id
SOURCE(CLICKHOUSE(HOST 'localhost' PORT 9000 USER 'default' DB 'cnc_monitor' TABLE 'machine_metadata'))
LIFETIME(3600)
LAYOUT(HASHED());

-- Create default machine metadata if not exists
CREATE TABLE IF NOT EXISTS machine_metadata (
    machine_id UInt16,
    model String DEFAULT 'DMG-MORI-5AXIS',
    location String DEFAULT 'Shop-Floor-A',
    install_date Date DEFAULT '2023-01-01',
    rated_rpm UInt16 DEFAULT 6000,
    bearing_type String DEFAULT 'SKF-7014CE'
) ENGINE = MergeTree()
ORDER BY machine_id;

-- Insert 40 machine metadata records
INSERT INTO machine_metadata (machine_id)
SELECT number FROM system.numbers WHERE number BETWEEN 1 AND 40;

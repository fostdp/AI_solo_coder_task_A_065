-- 精密数控机床主轴健康监控系统 - ClickHouse 初始化脚本
-- 创建时间: 2026-06-07

-- 创建数据库
CREATE DATABASE IF NOT EXISTS spindle_monitor
ENGINE = Atomic;

USE spindle_monitor;

-- ==================== 原始数据表 ====================

-- 振动传感器时序数据表
CREATE TABLE IF NOT EXISTS vibration_data
(
    timestamp DateTime64(3, 'UTC') CODEC(DoubleDelta, LZ4),
    machine_id UInt16 CODEC(LZ4),
    sensor_id UInt8 CODEC(LZ4),
    x_axis Float64 CODEC(Gorilla, LZ4),
    y_axis Float64 CODEC(Gorilla, LZ4),
    z_axis Float64 CODEC(Gorilla, LZ4),
    rms Float64 CODEC(Gorilla, LZ4),
    peak Float64 CODEC(Gorilla, LZ4),
    crest_factor Float64 CODEC(Gorilla, LZ4),
    spindle_speed Float64 CODEC(Gorilla, LZ4)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, sensor_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR
SETTINGS index_granularity = 8192;

-- 温度传感器时序数据表
CREATE TABLE IF NOT EXISTS temperature_data
(
    timestamp DateTime64(3, 'UTC') CODEC(DoubleDelta, LZ4),
    machine_id UInt16 CODEC(LZ4),
    sensor_id UInt8 CODEC(LZ4),
    value Float64 CODEC(Gorilla, LZ4),
    spindle_speed Float64 CODEC(Gorilla, LZ4)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, sensor_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR
SETTINGS index_granularity = 8192;

-- 位移传感器时序数据表
CREATE TABLE IF NOT EXISTS displacement_data
(
    timestamp DateTime64(3, 'UTC') CODEC(DoubleDelta, LZ4),
    machine_id UInt16 CODEC(LZ4),
    sensor_id UInt8 CODEC(LZ4),
    axial Float64 CODEC(Gorilla, LZ4),
    radial Float64 CODEC(Gorilla, LZ4),
    spindle_speed Float64 CODEC(Gorilla, LZ4)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, sensor_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR
SETTINGS index_granularity = 8192;

-- ==================== 聚合表 ====================

-- 振动数据每分钟聚合表
CREATE TABLE IF NOT EXISTS vibration_1min
(
    timestamp DateTime CODEC(DoubleDelta),
    machine_id UInt16,
    sensor_id UInt8,
    avg_rms Float64,
    max_rms Float64,
    min_rms Float64,
    avg_peak Float64,
    max_peak Float64,
    avg_crest Float64,
    sample_count UInt32
)
ENGINE = AggregatingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, sensor_id, timestamp)
TTL timestamp + INTERVAL 3 YEAR
SETTINGS index_granularity = 8192;

-- 温度数据每分钟聚合表
CREATE TABLE IF NOT EXISTS temperature_1min
(
    timestamp DateTime CODEC(DoubleDelta),
    machine_id UInt16,
    sensor_id UInt8,
    avg_temp Float64,
    max_temp Float64,
    min_temp Float64,
    sample_count UInt32
)
ENGINE = AggregatingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, sensor_id, timestamp)
TTL timestamp + INTERVAL 3 YEAR
SETTINGS index_granularity = 8192;

-- 机床状态汇总表
CREATE TABLE IF NOT EXISTS machine_status
(
    timestamp DateTime64(3, 'UTC') CODEC(DoubleDelta),
    machine_id UInt16,
    health_score Float64,
    rul_hours Float64,
    max_vibration_rms Float64,
    max_temperature Float64,
    alarm_status UInt8,
    avg_spindle_speed Float64
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR
SETTINGS index_granularity = 8192;

-- ==================== RUL预测结果表 ====================
CREATE TABLE IF NOT EXISTS rul_predictions
(
    timestamp DateTime64(3, 'UTC') CODEC(DoubleDelta),
    machine_id UInt16,
    rul_hours Float64,
    confidence Float64,
    avg_rms Float64,
    temp_rate Float64,
    bearing_life_hours Float64,
    model_version String DEFAULT 'v1.0'
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, timestamp)
TTL timestamp + INTERVAL 5 YEAR
SETTINGS index_granularity = 8192;

-- ==================== 告警事件表 ====================
CREATE TABLE IF NOT EXISTS alarm_events
(
    id String,
    timestamp DateTime64(3, 'UTC') CODEC(DoubleDelta),
    machine_id UInt16,
    sensor_type String,
    sensor_id Nullable(UInt8),
    level UInt8,
    message String,
    value Float64,
    threshold Float64,
    acknowledged UInt8 DEFAULT 0,
    acknowledged_at Nullable(DateTime64(3, 'UTC')),
    acknowledged_by Nullable(String)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (timestamp, machine_id, level)
TTL timestamp + INTERVAL 5 YEAR
SETTINGS index_granularity = 8192;

-- ==================== 维护工单表 ====================
CREATE TABLE IF NOT EXISTS work_orders
(
    id String,
    machine_id UInt16,
    created_at DateTime64(3, 'UTC'),
    rul_hours Float64,
    priority String,
    description String,
    status String DEFAULT 'PENDING',
    completed_at Nullable(DateTime64(3, 'UTC')),
    technician Nullable(String)
)
ENGINE = MergeTree()
ORDER BY (created_at, machine_id, status)
TTL created_at + INTERVAL 10 YEAR
SETTINGS index_granularity = 8192;

-- ==================== 物化视图 ====================

-- 振动1分钟聚合视图
CREATE MATERIALIZED VIEW IF NOT EXISTS vibration_1min_mv
TO vibration_1min
AS
SELECT
    toStartOfMinute(timestamp) AS timestamp,
    machine_id,
    sensor_id,
    avg(rms) AS avg_rms,
    max(rms) AS max_rms,
    min(rms) AS min_rms,
    avg(peak) AS avg_peak,
    max(peak) AS max_peak,
    avg(crest_factor) AS avg_crest,
    count() AS sample_count
FROM vibration_data
GROUP BY timestamp, machine_id, sensor_id;

-- 温度1分钟聚合视图
CREATE MATERIALIZED VIEW IF NOT EXISTS temperature_1min_mv
TO temperature_1min
AS
SELECT
    toStartOfMinute(timestamp) AS timestamp,
    machine_id,
    sensor_id,
    avg(value) AS avg_temp,
    max(value) AS max_temp,
    min(value) AS min_temp,
    count() AS sample_count
FROM temperature_data
GROUP BY timestamp, machine_id, sensor_id;

-- ==================== 机床信息维度表 ====================
CREATE TABLE IF NOT EXISTS machines
(
    machine_id UInt16,
    machine_name String,
    model String,
    location String,
    install_date Date,
    bearing_model String,
    rated_speed Float64,
    rated_power Float64
)
ENGINE = ReplacingMergeTree()
ORDER BY machine_id
SETTINGS index_granularity = 8192;

-- 插入40台机床的基础数据
INSERT INTO machines (machine_id, machine_name, model, location, install_date, bearing_model, rated_speed, rated_power)
SELECT
    number AS machine_id,
    concat('CNC-', toString(number)) AS machine_name,
    'DMG MORI DMU 50' AS model,
    concat('车间A-', toString(floor(number / 10) + 1)) AS location,
    toDate('2022-01-01') + (number * 7) AS install_date,
    'SKF 7014ACDGA/P4A' AS bearing_model,
    18000.0 AS rated_speed,
    22.0 AS rated_power
FROM numbers(1, 40);

-- ==================== 传感器位置配置表 ====================
CREATE TABLE IF NOT EXISTS sensor_positions
(
    sensor_type String,
    sensor_id UInt8,
    name String,
    x Float64,
    y Float64,
    location String
)
ENGINE = ReplacingMergeTree()
ORDER BY (sensor_type, sensor_id);

-- 插入8个振动传感器位置
INSERT INTO sensor_positions (sensor_type, sensor_id, name, x, y, location) VALUES
('vibration', 1, '前轴承径向X', 120, 80, '前端轴承座'),
('vibration', 2, '前轴承径向Y', 120, 120, '前端轴承座'),
('vibration', 3, '前轴承轴向', 80, 100, '前端轴承座'),
('vibration', 4, '中轴承径向X', 250, 80, '中间支撑'),
('vibration', 5, '中轴承径向Y', 250, 120, '中间支撑'),
('vibration', 6, '后轴承径向X', 380, 80, '后端轴承座'),
('vibration', 7, '后轴承径向Y', 380, 120, '后端轴承座'),
('vibration', 8, '刀柄位置', 40, 100, '刀具接口');

-- 插入4个温度传感器位置
INSERT INTO sensor_positions (sensor_type, sensor_id, name, x, y, location) VALUES
('temperature', 1, '前轴承温度', 130, 70, '前端轴承座'),
('temperature', 2, '中轴承温度', 260, 70, '中间支撑'),
('temperature', 3, '后轴承温度', 390, 70, '后端轴承座'),
('temperature', 4, '定子绕组温度', 300, 130, '电机定子');

-- 插入2个位移传感器位置
INSERT INTO sensor_positions (sensor_type, sensor_id, name, x, y, location) VALUES
('displacement', 1, '主轴轴向位移', 50, 100, '前端'),
('displacement', 2, '主轴径向跳动', 100, 110, '前端轴承座');

-- ==================== 索引优化 ====================
ALTER TABLE vibration_data ADD INDEX IF NOT EXISTS idx_rms rms TYPE minmax GRANULARITY 4;
ALTER TABLE temperature_data ADD INDEX IF NOT EXISTS idx_temp value TYPE minmax GRANULARITY 4;
ALTER TABLE alarm_events ADD INDEX IF NOT EXISTS idx_level level TYPE set(4) GRANULARITY 4;

-- ==================== 查询示例视图 ====================
-- 获取当前所有机床状态的最新视图
CREATE VIEW IF NOT EXISTS latest_machine_status AS
SELECT
    m.machine_id,
    m.machine_name,
    m.model,
    m.location,
    ms.health_score,
    ms.rul_hours,
    ms.max_vibration_rms,
    ms.max_temperature,
    ms.alarm_status,
    ms.timestamp AS last_update
FROM machines m
LEFT JOIN (
    SELECT
        machine_id,
        argMax(health_score, timestamp) AS health_score,
        argMax(rul_hours, timestamp) AS rul_hours,
        argMax(max_vibration_rms, timestamp) AS max_vibration_rms,
        argMax(max_temperature, timestamp) AS max_temperature,
        argMax(alarm_status, timestamp) AS alarm_status,
        max(timestamp) AS timestamp
    FROM machine_status
    WHERE timestamp > now() - INTERVAL 5 MINUTE
    GROUP BY machine_id
) ms ON m.machine_id = ms.machine_id
ORDER BY m.machine_id;

-- 本月告警统计视图
CREATE VIEW IF NOT EXISTS monthly_alarm_stats AS
SELECT
    toYYYYMM(timestamp) AS month,
    count() AS total_alarms,
    countIf(level = 2) AS critical_alarms,
    countIf(level = 1) AS warning_alarms,
    countDistinct(machine_id) AS affected_machines
FROM alarm_events
WHERE timestamp >= toStartOfMonth(now())
GROUP BY month
ORDER BY month DESC;

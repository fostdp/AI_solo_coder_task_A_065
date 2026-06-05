-- 精密数控机床主轴健康监控系统数据库初始化脚本
-- ClickHouse 23.x+

CREATE DATABASE IF NOT EXISTS spindle_monitor ENGINE = Atomic;

USE spindle_monitor;

-- 机床信息表
CREATE TABLE IF NOT EXISTS machine_info (
    machine_id UInt16,
    machine_name String,
    model String,
    install_date Date,
    location String,
    operator String,
    status Enum8('running' = 1, 'idle' = 2, 'maintenance' = 3, 'fault' = 4) DEFAULT 'running',
    created_at DateTime DEFAULT now(),
    updated_at DateTime DEFAULT now()
) ENGINE = ReplacingMergeTree(updated_at)
ORDER BY machine_id
PRIMARY KEY machine_id;

-- 传感器配置表
CREATE TABLE IF NOT EXISTS sensor_config (
    sensor_id UInt16,
    machine_id UInt16,
    sensor_type Enum8('vibration' = 1, 'temperature' = 2, 'displacement' = 3),
    position_name String,
    position_x Float32,
    position_y Float32,
    position_z Float32,
    axis Enum8('x' = 1, 'y' = 2, 'z' = 3, 'radial' = 4, 'axial' = 5),
    sampling_rate UInt32 DEFAULT 10000,
    range_min Float32,
    range_max Float32,
    unit String,
    install_date Date,
    status Enum8('active' = 1, 'inactive' = 2, 'fault' = 3) DEFAULT 'active',
    created_at DateTime DEFAULT now(),
    updated_at DateTime DEFAULT now()
) ENGINE = ReplacingMergeTree(updated_at)
ORDER BY (machine_id, sensor_id)
PRIMARY KEY (machine_id, sensor_id);

-- 原始传感器数据表（高频，100ms间隔）
CREATE TABLE IF NOT EXISTS sensor_raw_data (
    timestamp DateTime64(3, 'Asia/Shanghai'),
    machine_id UInt16,
    sensor_id UInt16,
    sensor_type Enum8('vibration' = 1, 'temperature' = 2, 'displacement' = 3),
    value Float32,
    spindle_speed Float32,
    load Float32,
    temperature Float32
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, sensor_id, timestamp)
TTL timestamp + INTERVAL 30 DAY
SETTINGS index_granularity = 8192;

-- 传感器秒级聚合表（用于实时监控）
CREATE TABLE IF NOT EXISTS sensor_agg_1s (
    timestamp DateTime64(0, 'Asia/Shanghai'),
    machine_id UInt16,
    sensor_id UInt16,
    sensor_type Enum8('vibration' = 1, 'temperature' = 2, 'displacement' = 3),
    value_min Float32,
    value_max Float32,
    value_avg Float32,
    value_rms Float32,
    value_std Float32,
    value_peak Float32,
    spindle_speed_avg Float32,
    load_avg Float32,
    temperature_avg Float32,
    sample_count UInt32
) ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, sensor_id, timestamp)
TTL timestamp + INTERVAL 90 DAY
SETTINGS index_granularity = 8192;

-- 传感器分钟级聚合表（用于趋势分析）
CREATE TABLE IF NOT EXISTS sensor_agg_1m (
    timestamp DateTime,
    machine_id UInt16,
    sensor_id UInt16,
    sensor_type Enum8('vibration' = 1, 'temperature' = 2, 'displacement' = 3),
    value_min Float32,
    value_max Float32,
    value_avg Float32,
    value_rms Float32,
    value_std Float32,
    value_peak Float32,
    value_crest_factor Float32,
    spindle_speed_avg Float32,
    load_avg Float32,
    temperature_avg Float32,
    sample_count UInt32
) ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, sensor_id, timestamp)
TTL timestamp + INTERVAL 2 YEAR
SETTINGS index_granularity = 8192;

-- 振动频谱数据表
CREATE TABLE IF NOT EXISTS vibration_spectrum (
    timestamp DateTime,
    machine_id UInt16,
    sensor_id UInt16,
    frequency Array(Float32),
    amplitude Array(Float32),
    rpm Float32
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, sensor_id, timestamp)
TTL timestamp + INTERVAL 90 DAY
SETTINGS index_granularity = 8192;

-- RUL预测结果表
CREATE TABLE IF NOT EXISTS rul_prediction (
    timestamp DateTime,
    machine_id UInt16,
    bearing_id UInt8,
    rul_hours Float32,
    rul_confidence Float32,
    vibration_rms_trend Float32,
    temperature_rate Float32,
    skf_l10_life Float32,
    lstm_prediction Float32,
    health_score UInt8,
    model_version String DEFAULT 'v1.0',
    created_at DateTime DEFAULT now()
) ENGINE = ReplacingMergeTree(created_at)
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, bearing_id, timestamp)
PRIMARY KEY (machine_id, bearing_id, timestamp)
TTL timestamp + INTERVAL 5 YEAR
SETTINGS index_granularity = 8192;

-- 告警记录表
CREATE TABLE IF NOT EXISTS alarms (
    alarm_id UUID DEFAULT generateUUIDv4(),
    timestamp DateTime,
    machine_id UInt16,
    sensor_id UInt16,
    alarm_level Enum8('info' = 0, 'warning' = 1, 'critical' = 2),
    alarm_type Enum8('vibration_high' = 1, 'temperature_high' = 2, 'displacement_abnormal' = 3, 'rul_low' = 4, 'sensor_fault' = 5),
    alarm_message String,
    value Float32,
    threshold Float32,
    duration_ms UInt32,
    acknowledged Bool DEFAULT false,
    acknowledged_at DateTime,
    acknowledged_by String,
    created_at DateTime DEFAULT now()
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, alarm_level, timestamp)
TTL timestamp + INTERVAL 5 YEAR
SETTINGS index_granularity = 8192;

-- 维护工单表
CREATE TABLE IF NOT EXISTS work_orders (
    work_order_id UUID DEFAULT generateUUIDv4(),
    machine_id UInt16,
    work_order_type Enum8('preventive' = 1, 'corrective' = 2, 'predictive' = 3),
    priority Enum8('low' = 1, 'medium' = 2, 'high' = 3, 'urgent' = 4),
    title String,
    description String,
    recommended_parts Array(String),
    estimated_hours Float32,
    status Enum8('open' = 1, 'in_progress' = 2, 'completed' = 3, 'cancelled' = 4) DEFAULT 'open',
    assigned_to String,
    scheduled_date Date,
    completed_date Date,
    created_at DateTime DEFAULT now(),
    updated_at DateTime DEFAULT now()
) ENGINE = ReplacingMergeTree(updated_at)
PARTITION BY toYYYYMM(created_at)
ORDER BY (machine_id, status, created_at)
PRIMARY KEY work_order_id
TTL created_at + INTERVAL 10 YEAR
SETTINGS index_granularity = 8192;

-- 换刀建议表
CREATE TABLE IF NOT EXISTS tool_change_suggestions (
    suggestion_id UUID DEFAULT generateUUIDv4(),
    machine_id UInt16,
    tool_id UInt16,
    reason String,
    rul_hours Float32,
    recommended_action String,
    created_at DateTime DEFAULT now()
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(created_at)
ORDER BY (machine_id, created_at)
TTL created_at + INTERVAL 1 YEAR
SETTINGS index_granularity = 8192;

-- 健康评分历史表
CREATE TABLE IF NOT EXISTS health_score_history (
    timestamp DateTime,
    machine_id UInt16,
    overall_score UInt8,
    vibration_score UInt8,
    temperature_score UInt8,
    displacement_score UInt8,
    rul_score UInt8,
    created_at DateTime DEFAULT now()
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, timestamp)
TTL timestamp + INTERVAL 5 YEAR
SETTINGS index_granularity = 8192;

-- 初始化机床数据（40台五轴数控机床）
INSERT INTO machine_info (machine_id, machine_name, model, install_date, location, operator, status)
SELECT
    number AS machine_id,
    concat('CNC-', toString(number)) AS machine_name,
    'DMG MORI DMU 50' AS model,
    toDate('2022-01-01') + (number % 365) AS install_date,
    concat('Line-', toString(intDiv(number, 10) + 1)) AS location,
    concat('Operator-', toString(number % 10 + 1)) AS operator,
    if(number % 7 = 0, 'idle', 'running') AS status
FROM numbers(1, 40);

-- 初始化传感器配置
-- 每台机床: 8个振动传感器, 4个温度传感器, 2个位移传感器
INSERT INTO sensor_config (sensor_id, machine_id, sensor_type, position_name, position_x, position_y, position_z, axis, range_min, range_max, unit, install_date)
SELECT
    row_number() OVER (PARTITION BY m.machine_id ORDER BY s.sensor_type, s.idx) AS sensor_id,
    m.machine_id,
    s.sensor_type,
    s.position_name,
    s.position_x,
    s.position_y,
    s.position_z,
    s.axis,
    s.range_min,
    s.range_max,
    s.unit,
    toDate('2022-01-01')
FROM machine_info m
CROSS JOIN (
    -- 8个振动传感器
    SELECT 1 AS idx, 'vibration' AS sensor_type, '前轴承径向X' AS position_name, -20.0 AS position_x, 0.0 AS position_y, 100.0 AS position_z, 'x' AS axis, -50.0 AS range_min, 50.0 AS range_max, 'mm/s' AS unit
    UNION ALL SELECT 2, 'vibration', '前轴承径向Y', 0.0, -20.0, 100.0, 'y', -50.0, 50.0, 'mm/s'
    UNION ALL SELECT 3, 'vibration', '前轴承轴向', 0.0, 0.0, 120.0, 'z', -50.0, 50.0, 'mm/s'
    UNION ALL SELECT 4, 'vibration', '后轴承径向X', -20.0, 0.0, -100.0, 'x', -50.0, 50.0, 'mm/s'
    UNION ALL SELECT 5, 'vibration', '后轴承径向Y', 0.0, -20.0, -100.0, 'y', -50.0, 50.0, 'mm/s'
    UNION ALL SELECT 6, 'vibration', '后轴承轴向', 0.0, 0.0, -120.0, 'z', -50.0, 50.0, 'mm/s'
    UNION ALL SELECT 7, 'vibration', '电机端径向', -15.0, 0.0, 150.0, 'radial', -50.0, 50.0, 'mm/s'
    UNION ALL SELECT 8, 'vibration', '刀具端径向', -15.0, 0.0, -150.0, 'radial', -50.0, 50.0, 'mm/s'
    UNION ALL
    -- 4个温度传感器
    SELECT 9, 'temperature', '前轴承座', 0.0, 0.0, 100.0, 'axial', -40.0, 150.0, '°C'
    UNION ALL SELECT 10, 'temperature', '后轴承座', 0.0, 0.0, -100.0, 'axial', -40.0, 150.0, '°C'
    UNION ALL SELECT 11, 'temperature', '定子绕组', 0.0, 0.0, 50.0, 'axial', -40.0, 180.0, '°C'
    UNION ALL SELECT 12, 'temperature', '环境温度', 50.0, 0.0, 0.0, 'axial', -40.0, 80.0, '°C'
    UNION ALL
    -- 2个位移传感器
    SELECT 13, 'displacement', '轴向位移', 0.0, 0.0, 0.0, 'axial', -2.0, 2.0, 'mm'
    UNION ALL SELECT 14, 'displacement', '径向跳动', 0.0, 0.0, 0.0, 'radial', 0.0, 0.5, 'mm'
) s;

-- 初始化健康评分数据
INSERT INTO health_score_history (timestamp, machine_id, overall_score, vibration_score, temperature_score, displacement_score, rul_score)
SELECT
    now() - INTERVAL 1 HOUR,
    machine_id,
    80 + (machine_id % 20) AS overall_score,
    75 + (machine_id % 25) AS vibration_score,
    85 + (machine_id % 15) AS temperature_score,
    90 + (machine_id % 10) AS displacement_score,
    70 + (machine_id % 30) AS rul_score
FROM machine_info;

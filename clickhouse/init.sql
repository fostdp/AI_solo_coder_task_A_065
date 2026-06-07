CREATE DATABASE IF NOT EXISTS cnc_monitor;

USE cnc_monitor;

CREATE TABLE IF NOT EXISTS machine_metrics (
    timestamp DateTime64(3, 'Asia/Shanghai'),
    machine_id UInt16,
    spindle_id UInt8,
    vibration Array(Float64),
    temperature Array(Float64),
    displacement Array(Float64),
    rpm Float64,
    vibration_rms Array(Float64),
    vibration_peak Array(Float64),
    vibration_freq Array(Array(Float64))
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR;

CREATE TABLE IF NOT EXISTS machine_status (
    machine_id UInt16 PRIMARY KEY,
    last_update DateTime64(3, 'Asia/Shanghai'),
    health_score Float64,
    rul_hours Float64,
    vibration_severity Array(Float64),
    avg_temperature Array(Float64),
    alarm_level UInt8 DEFAULT 0,
    total_runtime_hours Float64 DEFAULT 0
) ENGINE = ReplacingMergeTree(last_update)
ORDER BY machine_id;

CREATE TABLE IF NOT EXISTS alarms (
    timestamp DateTime64(3, 'Asia/Shanghai'),
    machine_id UInt16,
    alarm_type String,
    alarm_level UInt8,
    message String,
    sensor_index UInt8 DEFAULT 0,
    value Float64 DEFAULT 0,
    threshold Float64 DEFAULT 0,
    acknowledged UInt8 DEFAULT 0
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, timestamp)
TTL timestamp + INTERVAL 2 YEAR;

CREATE TABLE IF NOT EXISTS maintenance_orders (
    order_id UUID DEFAULT generateUUIDv4(),
    created_at DateTime64(3, 'Asia/Shanghai'),
    machine_id UInt16,
    order_type String,
    priority String,
    description String,
    estimated_rul Float64,
    status String DEFAULT 'pending',
    scheduled_date Date
) ENGINE = MergeTree()
ORDER BY (created_at, machine_id);

CREATE TABLE IF NOT EXISTS rul_history (
    timestamp DateTime64(3, 'Asia/Shanghai'),
    machine_id UInt16,
    rul_hours Float64,
    health_score Float64,
    vibration_trend Float64,
    temperature_trend Float64,
    model_source String
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, timestamp)
TTL timestamp + INTERVAL 3 YEAR;

CREATE TABLE IF NOT EXISTS monthly_stats (
    month Date,
    machine_id UInt16,
    total_runtime Float64,
    vibration_alerts UInt32,
    temperature_alerts UInt32,
    maintenance_count UInt32,
    avg_health_score Float64
) ENGINE = SummingMergeTree()
ORDER BY (month, machine_id);

CREATE TABLE IF NOT EXISTS sensor_config (
    machine_id UInt16,
    sensor_type String,
    sensor_index UInt8,
    location_x Float64,
    location_y Float64,
    location_z Float64,
    description String,
    install_date Date DEFAULT today()
) ENGINE = ReplacingMergeTree()
ORDER BY (machine_id, sensor_type, sensor_index);

INSERT INTO sensor_config (machine_id, sensor_type, sensor_index, location_x, location_y, location_z, description)
WITH
    ['front_left', 'front_right', 'back_left', 'back_right', 'middle_left', 'middle_right', 'rear_left', 'rear_right'] AS vib_pos,
    ['front', 'middle', 'rear', 'housing'] AS temp_pos
SELECT
    machine_id,
    'vibration' AS sensor_type,
    vib_idx AS sensor_index,
    cos((vib_idx / 8) * 2 * pi()) * 150 AS location_x,
    sin((vib_idx / 8) * 2 * pi()) * 150 AS location_y,
    0 AS location_z,
    vib_pos[vib_idx + 1] AS description
FROM
    (SELECT number AS machine_id FROM numbers(1, 40))
ARRAY JOIN
    range(8) AS vib_idx;

INSERT INTO sensor_config (machine_id, sensor_type, sensor_index, location_x, location_y, location_z, description)
WITH
    ['front', 'middle', 'rear', 'housing'] AS temp_pos
SELECT
    machine_id,
    'temperature' AS sensor_type,
    temp_idx AS sensor_index,
    0 AS location_x,
    0 AS location_y,
    (temp_idx - 1.5) * 50 AS location_z,
    temp_pos[temp_idx + 1] AS description
FROM
    (SELECT number AS machine_id FROM numbers(1, 40))
ARRAY JOIN
    range(4) AS temp_idx;

CREATE VIEW IF NOT EXISTS v_machine_overview
AS
SELECT
    ms.machine_id,
    ms.last_update,
    ms.health_score,
    ms.rul_hours,
    ms.alarm_level,
    ms.total_runtime_hours,
    ms.vibration_severity,
    ms.avg_temperature,
    arrayMax(ms.vibration_severity) AS max_vibration,
    arrayAvg(ms.avg_temperature) AS avg_temperature_all
FROM machine_status ms
ORDER BY ms.machine_id;

CREATE VIEW IF NOT EXISTS v_active_alarms
AS
SELECT
    machine_id,
    alarm_level,
    count() AS alarm_count,
    max(timestamp) AS last_alarm_time,
    groupArray(message) AS recent_messages
FROM alarms
WHERE acknowledged = 0
  AND timestamp > now() - INTERVAL 24 HOUR
GROUP BY machine_id, alarm_level
ORDER BY machine_id, alarm_level DESC;

CREATE VIEW IF NOT EXISTS v_rul_trend
AS
SELECT
    machine_id,
    toStartOfHour(timestamp) AS hour,
    avg(rul_hours) AS avg_rul,
    avg(health_score) AS avg_health,
    max(rul_hours) AS max_rul,
    min(rul_hours) AS min_rul
FROM rul_history
WHERE timestamp > now() - INTERVAL 7 DAY
GROUP BY machine_id, hour
ORDER BY machine_id, hour;

SET allow_experimental_analyzer = 1;

SELECT 'Database initialization completed successfully' AS status;

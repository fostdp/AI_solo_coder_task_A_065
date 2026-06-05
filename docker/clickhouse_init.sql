CREATE DATABASE IF NOT EXISTS spindle_monitor;

USE spindle_monitor;

CREATE TABLE IF NOT EXISTS sensor_data
(
    machine_id      UInt16,
    sensor_id       UInt8,
    sensor_type     LowCardinality(String),
    timestamp       DateTime64(3, 'UTC'),
    value           Float64,
    rpm             Float32 DEFAULT 0,
    vibration_rms   Float64 DEFAULT 0,
    temperature     Float64 DEFAULT 0,
    displacement    Float64 DEFAULT 0,
    date            Date MATERIALIZED toDate(timestamp)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, sensor_id, timestamp)
TTL date + INTERVAL 6 MONTH
SETTINGS index_granularity = 8192;

CREATE TABLE IF NOT EXISTS vibration_spectrum
(
    machine_id      UInt16,
    sensor_id       UInt8,
    timestamp       DateTime64(3, 'UTC'),
    freq_bin        UInt16,
    magnitude       Float64,
    date            Date MATERIALIZED toDate(timestamp)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, sensor_id, timestamp, freq_bin)
TTL date + INTERVAL 3 MONTH
SETTINGS index_granularity = 8192;

CREATE TABLE IF NOT EXISTS spindle_health
(
    machine_id      UInt16,
    timestamp       DateTime64(3, 'UTC'),
    health_score    Float32,
    vibration_rms   Float64,
    temperature     Float64,
    displacement    Float64,
    rpm             Float32,
    rul_hours       Float64,
    bearing_life_l10 Float64,
    date            Date MATERIALIZED toDate(timestamp)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, timestamp)
TTL date + INTERVAL 1 YEAR
SETTINGS index_granularity = 8192;

CREATE TABLE IF NOT EXISTS alerts
(
    id              UUID DEFAULT generateUUIDv4(),
    machine_id      UInt16,
    sensor_id       UInt8 DEFAULT 0,
    alert_level     LowCardinality(String),
    alert_type      LowCardinality(String),
    message         String,
    value           Float64 DEFAULT 0,
    threshold       Float64 DEFAULT 0,
    timestamp       DateTime64(3, 'UTC'),
    acknowledged    UInt8 DEFAULT 0,
    date            Date MATERIALIZED toDate(timestamp)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (machine_id, timestamp)
TTL date + INTERVAL 3 MONTH
SETTINGS index_granularity = 8192;

CREATE TABLE IF NOT EXISTS maintenance_orders
(
    id              UUID DEFAULT generateUUIDv4(),
    machine_id      UInt16,
    order_type      LowCardinality(String),
    priority        LowCardinality(String),
    description     String,
    rul_at_creation Float64,
    created_at      DateTime64(3, 'UTC'),
    status          LowCardinality(String) DEFAULT 'pending',
    completed_at    Nullable(DateTime64(3, 'UTC')),
    date            Date MATERIALIZED toDate(created_at)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(created_at)
ORDER BY (machine_id, created_at)
SETTINGS index_granularity = 8192;

CREATE TABLE IF NOT EXISTS machine_registry
(
    machine_id      UInt16,
    machine_name    String,
    location        String,
    spindle_model   String,
    bearing_model   String DEFAULT 'SKF-7014CD/P4',
    install_date    Date,
    last_maintenance Nullable(Date),
    commission_date Date
)
ENGINE = ReplacingMergeTree()
ORDER BY (machine_id)
SETTINGS index_granularity = 8192;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_vibration_rms
TO spindle_health
AS
SELECT
    machine_id,
    timestamp,
    100.0 - LEAST(100.0, GREATEST(0.0, 100.0 - (avg(vibration_rms) / 7.1) * 50.0 - (max(temperature) - 40.0) * 2.0)) AS health_score,
    avg(vibration_rms) AS vibration_rms,
    avg(temperature) AS temperature,
    avg(displacement) AS displacement,
    avg(rpm) AS rpm,
    0.0 AS rul_hours,
    0.0 AS bearing_life_l10,
    toDate(timestamp) AS date
FROM sensor_data
WHERE sensor_type = 'vibration'
GROUP BY machine_id, toStartOfMinute(timestamp) AS timestamp;

INSERT INTO machine_registry (machine_id, machine_name, location, spindle_model, bearing_model, install_date, commission_date) VALUES
(1, 'CNC-001', 'A-01', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-01-15', '2024-02-01'),
(2, 'CNC-002', 'A-02', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-01-20', '2024-02-01'),
(3, 'CNC-003', 'A-03', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-02-01', '2024-02-15'),
(4, 'CNC-004', 'A-04', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-02-10', '2024-02-20'),
(5, 'CNC-005', 'A-05', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-02-15', '2024-03-01'),
(6, 'CNC-006', 'B-01', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2024-03-01', '2024-03-15'),
(7, 'CNC-007', 'B-02', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2024-03-10', '2024-03-20'),
(8, 'CNC-008', 'B-03', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2024-03-15', '2024-04-01'),
(9, 'CNC-009', 'B-04', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2024-03-20', '2024-04-01'),
(10, 'CNC-010', 'B-05', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2024-04-01', '2024-04-15'),
(11, 'CNC-011', 'C-01', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-04-10', '2024-04-20'),
(12, 'CNC-012', 'C-02', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-04-15', '2024-05-01'),
(13, 'CNC-013', 'C-03', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-05-01', '2024-05-15'),
(14, 'CNC-014', 'C-04', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-05-10', '2024-05-20'),
(15, 'CNC-015', 'C-05', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-05-15', '2024-06-01'),
(16, 'CNC-016', 'D-01', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2024-06-01', '2024-06-15'),
(17, 'CNC-017', 'D-02', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2024-06-10', '2024-06-20'),
(18, 'CNC-018', 'D-03', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2024-06-15', '2024-07-01'),
(19, 'CNC-019', 'D-04', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2024-07-01', '2024-07-15'),
(20, 'CNC-020', 'D-05', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2024-07-10', '2024-07-20'),
(21, 'CNC-021', 'E-01', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-07-15', '2024-08-01'),
(22, 'CNC-022', 'E-02', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-08-01', '2024-08-15'),
(23, 'CNC-023', 'E-03', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-08-10', '2024-08-20'),
(24, 'CNC-024', 'E-04', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-08-15', '2024-09-01'),
(25, 'CNC-025', 'E-05', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-09-01', '2024-09-15'),
(26, 'CNC-026', 'F-01', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2024-09-10', '2024-09-20'),
(27, 'CNC-027', 'F-02', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2024-09-15', '2024-10-01'),
(28, 'CNC-028', 'F-03', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2024-10-01', '2024-10-15'),
(29, 'CNC-029', 'F-04', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2024-10-10', '2024-10-20'),
(30, 'CNC-030', 'F-05', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2024-10-15', '2024-11-01'),
(31, 'CNC-031', 'G-01', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-11-01', '2024-11-15'),
(32, 'CNC-032', 'G-02', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-11-10', '2024-11-20'),
(33, 'CNC-033', 'G-03', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-11-15', '2024-12-01'),
(34, 'CNC-034', 'G-04', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-12-01', '2024-12-15'),
(35, 'CNC-035', 'G-05', 'GMN-HCS170', 'SKF-7014CD/P4', '2024-12-10', '2024-12-20'),
(36, 'CNC-036', 'H-01', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2024-12-15', '2025-01-01'),
(37, 'CNC-037', 'H-02', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2025-01-01', '2025-01-15'),
(38, 'CNC-038', 'H-03', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2025-01-10', '2025-01-20'),
(39, 'CNC-039', 'H-04', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2025-01-15', '2025-02-01'),
(40, 'CNC-040', 'H-05', 'FISCHER-MFM1204', 'SKF-7014CD/P4', '2025-02-01', '2025-02-15');

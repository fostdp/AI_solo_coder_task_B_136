
-- ========================================
-- ClickHouse 高级配置：降采样 + 数据保留策略 (TTL)
-- 古代临冲吕公车结构仿真与稳定性分析系统
-- 配合 init.sql 使用，在首次初始化后执行
-- ========================================

USE siege_tower;

-- ========================================
-- 1. 降采样物化视图（按小时/天/月聚合）
-- ========================================

-- 1.1 传感器数据：每小时降采样视图
DROP TABLE IF EXISTS sensor_stats_hourly;
CREATE TABLE sensor_stats_hourly
(
    timestamp        DateTime('Asia/Shanghai'),
    tower_id         UInt32,
    layer_id         UInt8,
    max_stress       Float64,
    avg_stress       Float64,
    min_stress       Float64,
    p95_stress       Float64,
    max_tilt         Float64,
    avg_tilt         Float64,
    max_wind_speed   Float64,
    avg_wind_speed   Float64,
    max_ground_pressure Float64,
    avg_ground_pressure Float64,
    avg_humidity     Float64,
    avg_temperature  Float64,
    alert_count      UInt64,
    sample_count     UInt64
)
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (tower_id, layer_id, timestamp)
TTL timestamp + INTERVAL 3 YEAR
SETTINGS index_granularity = 4096;

-- 1.2 物化视图：原始数据 -> 每小时聚合
DROP VIEW IF EXISTS sensor_stats_hourly_mv;
CREATE MATERIALIZED VIEW sensor_stats_hourly_mv
TO sensor_stats_hourly
AS
SELECT
    toStartOfHour(timestamp) AS timestamp,
    tower_id,
    layer_id,
    max(stress_von_mises) AS max_stress,
    avg(stress_von_mises) AS avg_stress,
    min(stress_von_mises) AS min_stress,
    quantile(0.95)(stress_von_mises) AS p95_stress,
    max(tilt_total) AS max_tilt,
    avg(tilt_total) AS avg_tilt,
    max(wind_speed) AS max_wind_speed,
    avg(wind_speed) AS avg_wind_speed,
    max(ground_pressure) AS max_ground_pressure,
    avg(ground_pressure) AS avg_ground_pressure,
    avg(humidity) AS avg_humidity,
    avg(temperature) AS avg_temperature,
    sum(is_alert) AS alert_count,
    count() AS sample_count
FROM sensor_data
GROUP BY tower_id, layer_id, timestamp;

-- 1.3 传感器数据：每天降采样
DROP TABLE IF EXISTS sensor_stats_daily;
CREATE TABLE sensor_stats_daily
(
    date             Date,
    tower_id         UInt32,
    layer_id         UInt8,
    max_stress       Float64,
    avg_stress       Float64,
    min_stress       Float64,
    p99_stress       Float64,
    max_tilt         Float64,
    avg_tilt         Float64,
    max_wind_speed   Float64,
    avg_wind_speed   Float64,
    avg_ground_pressure Float64,
    total_alerts     UInt64,
    total_samples    UInt64
)
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(date)
ORDER BY (tower_id, layer_id, date)
TTL date + INTERVAL 10 YEAR
SETTINGS index_granularity = 2048;

-- 1.4 物化视图：每小时 -> 每天聚合
DROP VIEW IF EXISTS sensor_stats_daily_mv;
CREATE MATERIALIZED VIEW sensor_stats_daily_mv
TO sensor_stats_daily
AS
SELECT
    toDate(timestamp) AS date,
    tower_id,
    layer_id,
    max(max_stress) AS max_stress,
    avg(avg_stress) AS avg_stress,
    min(min_stress) AS min_stress,
    max(p95_stress) AS p99_stress,
    max(max_tilt) AS max_tilt,
    avg(avg_tilt) AS avg_tilt,
    max(max_wind_speed) AS max_wind_speed,
    avg(avg_wind_speed) AS avg_wind_speed,
    avg(avg_ground_pressure) AS avg_ground_pressure,
    sum(alert_count) AS total_alerts,
    sum(sample_count) AS total_samples
FROM sensor_stats_hourly
GROUP BY tower_id, layer_id, date;

-- ========================================
-- 2. 告警事件：按月份汇总表
-- ========================================
DROP TABLE IF EXISTS alert_events_monthly;
CREATE TABLE alert_events_monthly
(
    month            Date,
    tower_id         UInt32,
    alert_type       Enum8('tilt_exceed'=1, 'stress_critical'=2, 'wind_overload'=3, 'ground_failure'=4, 'vibration_exceed'=5, 'structure_instability'=6),
    level1_count     UInt32 COMMENT '预警次数',
    level2_count     UInt32 COMMENT '告警次数',
    level3_count     UInt32 COMMENT '危险次数',
    total_count      UInt32 COMMENT '总次数',
    acknowledged_count UInt32 COMMENT '已确认次数'
)
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(month)
ORDER BY (tower_id, alert_type, month)
TTL month + INTERVAL 10 YEAR
SETTINGS index_granularity = 1024;

DROP VIEW IF EXISTS alert_events_monthly_mv;
CREATE MATERIALIZED VIEW alert_events_monthly_mv
TO alert_events_monthly
AS
SELECT
    toStartOfMonth(timestamp) AS month,
    tower_id,
    alert_type,
    countIf(alert_level = 1) AS level1_count,
    countIf(alert_level = 2) AS level2_count,
    countIf(alert_level = 3) AS level3_count,
    count() AS total_count,
    sum(is_acknowledged) AS acknowledged_count
FROM alert_events
GROUP BY tower_id, alert_type, month;

-- ========================================
-- 3. 结构分析：月度汇总
-- ========================================
DROP TABLE IF EXISTS structure_analysis_monthly;
CREATE TABLE structure_analysis_monthly
(
    month            Date,
    tower_id         UInt32,
    min_safety_factor Float64,
    avg_safety_factor Float64,
    min_stability_margin Float64,
    max_stress       Float64,
    max_tilt         Float64,
    unstable_count   UInt32,
    total_analysis   UInt32,
    avg_second_order_effect Float64
)
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(month)
ORDER BY (tower_id, month)
TTL month + INTERVAL 10 YEAR
SETTINGS index_granularity = 1024;

DROP VIEW IF EXISTS structure_analysis_monthly_mv;
CREATE MATERIALIZED VIEW structure_analysis_monthly_mv
TO structure_analysis_monthly
AS
SELECT
    toStartOfMonth(timestamp) AS month,
    tower_id,
    min(safety_factor) AS min_safety_factor,
    avg(safety_factor) AS avg_safety_factor,
    min(stability_margin) AS min_stability_margin,
    max(max_stress) AS max_stress,
    max(max_tilt) AS max_tilt,
    countIf(is_stable = 0) AS unstable_count,
    count() AS total_analysis,
    avg(second_order_effect) AS avg_second_order_effect
FROM structure_analysis
GROUP BY tower_id, month;

-- ========================================
-- 4. 数据保留策略（TTL优化）
-- ========================================

-- 4.1 原始传感器数据：保留 5 年
ALTER TABLE sensor_data MODIFY TTL timestamp + INTERVAL 5 YEAR;

-- 4.2 FEM节点结果：保留 1 年（高基数数据）
ALTER TABLE fem_node_results MODIFY TTL timestamp + INTERVAL 1 YEAR;

-- 4.3 告警事件：永久保留（审计用）
ALTER TABLE alert_events REMOVE TTL;

-- 4.4 结构分析：保留 10 年
ALTER TABLE structure_analysis MODIFY TTL timestamp + INTERVAL 10 YEAR;

-- ========================================
-- 5. 性能优化设置
-- ========================================

-- 启用合并优化
ALTER TABLE sensor_data SETTINGS
    max_parts_in_total = 100000,
    min_bytes_for_wide_part = 104857600;

-- 设置异步删除过期数据
SET merge_tree_adjust_ttl = 1;

-- ========================================
-- 6. 辅助视图：当前状态
-- ========================================

-- 6.1 最新传感器状态（最近1小时数据）
DROP VIEW IF EXISTS current_sensor_status;
CREATE VIEW current_sensor_status AS
SELECT
    tower_id,
    layer_id,
    argMax(timestamp, timestamp) AS last_update,
    argMax(stress_von_mises, timestamp) AS current_stress,
    argMax(tilt_total, timestamp) AS current_tilt,
    argMax(wind_speed, timestamp) AS current_wind,
    argMax(ground_pressure, timestamp) AS current_ground_pressure,
    argMax(is_alert, timestamp) AS current_alert
FROM sensor_data
WHERE timestamp > now() - INTERVAL 1 HOUR
GROUP BY tower_id, layer_id;

-- 6.2 每日告警摘要
DROP VIEW IF EXISTS daily_alert_summary;
CREATE VIEW daily_alert_summary AS
SELECT
    toDate(timestamp) AS date,
    tower_id,
    alert_level,
    count() AS count,
    any(description) AS sample_description
FROM alert_events
GROUP BY date, tower_id, alert_level
ORDER BY date DESC, tower_id, alert_level;

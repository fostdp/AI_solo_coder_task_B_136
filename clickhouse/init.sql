-- ClickHouse 初始化脚本
-- 古代临冲吕公车（攻城塔）结构仿真与稳定性分析系统

CREATE DATABASE IF NOT EXISTS siege_tower ENGINE = Atomic;

USE siege_tower;

-- 传感器原始数据表（按分钟上报）
CREATE TABLE IF NOT EXISTS sensor_data
(
    timestamp        DateTime64(3, 'Asia/Shanghai') DEFAULT now64(),
    tower_id         UInt32,
    tower_name       String,
    layer_id         UInt8,
    layer_name       String,
    stress_x         Float64 COMMENT 'X方向应力 (MPa)',
    stress_y         Float64 COMMENT 'Y方向应力 (MPa)',
    stress_z         Float64 COMMENT 'Z方向应力 (MPa)',
    stress_von_mises Float64 COMMENT 'von Mises等效应力 (MPa)',
    tilt_x           Float64 COMMENT 'X轴倾斜角 (度)',
    tilt_y           Float64 COMMENT 'Y轴倾斜角 (度)',
    tilt_total       Float64 COMMENT '总倾斜角 (度)',
    wind_load_x      Float64 COMMENT 'X方向风荷载 (N/m²)',
    wind_load_y      Float64 COMMENT 'Y方向风荷载 (N/m²)',
    wind_speed       Float64 COMMENT '风速 (m/s)',
    ground_pressure  Float64 COMMENT '地面承载力 (kPa)',
    ground_settlement Float64 COMMENT '地面沉降量 (mm)',
    soil_type        Enum8('sand'=1, 'clay'=2, 'silt'=3, 'rock'=4, 'loam'=5),
    temperature      Float64 COMMENT '环境温度 (°C)',
    humidity         Float64 COMMENT '环境湿度 (%)',
    vibration_freq   Float64 COMMENT '结构振动频率 (Hz)',
    vibration_amp    Float64 COMMENT '结构振动振幅 (mm)',
    is_alert         UInt8 DEFAULT 0 COMMENT '是否告警 0=否 1=是',
    alert_level      UInt8 DEFAULT 0 COMMENT '告警等级 0=正常 1=预警 2=告警 3=危险'
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (tower_id, layer_id, timestamp)
TTL timestamp + INTERVAL 5 YEAR
SETTINGS index_granularity = 8192;

-- 塔体结构分析结果表
CREATE TABLE IF NOT EXISTS structure_analysis
(
    timestamp        DateTime64(3, 'Asia/Shanghai') DEFAULT now64(),
    tower_id         UInt32,
    tower_name       String,
    safety_factor    Float64 COMMENT '整体安全系数',
    critical_stress  Float64 COMMENT '临界应力 (MPa)',
    max_stress       Float64 COMMENT '最大应力 (MPa)',
    max_stress_layer UInt8 COMMENT '最大应力所在层',
    max_tilt         Float64 COMMENT '最大倾斜角 (度)',
    max_tilt_layer   UInt8 COMMENT '最大倾斜所在层',
    wind_resistance_limit Float64 COMMENT '极限抗风能力 (m/s)',
    current_wind_factor Float64 COMMENT '当前风荷载系数',
    ground_capacity_ratio Float64 COMMENT '地面承载力利用率',
    is_stable        UInt8 COMMENT '是否稳定 0=不稳定 1=稳定',
    stability_margin Float64 COMMENT '稳定裕度 (%)',
    second_order_effect Float64 COMMENT '二阶效应放大系数',
    natural_frequency Float64 COMMENT '结构自振频率 (Hz)',
    damping_ratio    Float64 COMMENT '阻尼比'
)
ENGINE = ReplacingMergeTree(timestamp)
PARTITION BY toYYYYMM(timestamp)
ORDER BY (tower_id, timestamp)
SETTINGS index_granularity = 4096;

-- 告警事件表
CREATE TABLE IF NOT EXISTS alert_events
(
    timestamp        DateTime64(3, 'Asia/Shanghai') DEFAULT now64(),
    event_id         UUID DEFAULT generateUUIDv4(),
    tower_id         UInt32,
    tower_name       String,
    alert_type       Enum8('tilt_exceed'=1, 'stress_critical'=2, 'wind_overload'=3, 'ground_failure'=4, 'vibration_exceed'=5, 'structure_instability'=6),
    alert_level      UInt8 COMMENT '告警等级 1=预警 2=告警 3=危险',
    layer_id         UInt8 COMMENT '触发层',
    metric_name      String COMMENT '指标名称',
    metric_value     Float64 COMMENT '当前值',
    threshold        Float64 COMMENT '阈值',
    description      String,
    is_acknowledged  UInt8 DEFAULT 0,
    acknowledged_at  Nullable(DateTime64(3, 'Asia/Shanghai')),
    acknowledged_by  Nullable(String)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (tower_id, alert_level, timestamp)
SETTINGS index_granularity = 1024;

-- 地面适应性分析表
CREATE TABLE IF NOT EXISTS ground_analysis
(
    timestamp        DateTime64(3, 'Asia/Shanghai') DEFAULT now64(),
    tower_id         UInt32,
    soil_type        Enum8('sand'=1, 'clay'=2, 'silt'=3, 'rock'=4, 'loam'=5),
    bearing_capacity Float64 COMMENT '土壤承载力 (kPa)',
    applied_pressure Float64 COMMENT '实际施加压力 (kPa)',
    safety_factor    Float64 COMMENT '抗滑安全系数',
    settlement       Float64 COMMENT '预计沉降 (mm)',
    differential_settlement Float64 COMMENT '差异沉降 (mm)',
    passability_score Float64 COMMENT '通过性评分 (0-100)',
    can_pass         UInt8 COMMENT '是否可通过 0=否 1=是',
    risk_level       UInt8 COMMENT '风险等级 1=低 2=中 3=高'
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (tower_id, soil_type, timestamp)
SETTINGS index_granularity = 4096;

-- 有限元分析节点结果表
CREATE TABLE IF NOT EXISTS fem_node_results
(
    timestamp        DateTime64(3, 'Asia/Shanghai') DEFAULT now64(),
    tower_id         UInt32,
    layer_id         UInt8,
    node_id          UInt32,
    node_x           Float64 COMMENT '节点X坐标 (m)',
    node_y           Float64 COMMENT '节点Y坐标 (m)',
    node_z           Float64 COMMENT '节点Z坐标 (m)',
    displacement_x   Float64 COMMENT 'X向位移 (mm)',
    displacement_y   Float64 COMMENT 'Y向位移 (mm)',
    displacement_z   Float64 COMMENT 'Z向位移 (mm)',
    displacement_total Float64 COMMENT '总位移 (mm)',
    stress_xx        Float64 COMMENT '正应力σxx (MPa)',
    stress_yy        Float64 COMMENT '正应力σyy (MPa)',
    stress_zz        Float64 COMMENT '正应力σzz (MPa)',
    stress_xy        Float64 COMMENT '剪应力τxy (MPa)',
    stress_yz        Float64 COMMENT '剪应力τyz (MPa)',
    stress_zx        Float64 COMMENT '剪应力τzx (MPa)',
    von_mises        Float64 COMMENT 'Von Mises等效应力 (MPa)',
    plastic_strain   Float64 COMMENT '塑性应变'
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (tower_id, layer_id, node_id, timestamp)
SETTINGS index_granularity = 16384;

-- 攻城塔元数据表
CREATE TABLE IF NOT EXISTS towers_metadata
(
    tower_id         UInt32,
    tower_name       String,
    build_date       Date,
    material         String COMMENT '主要材质',
    total_height     Float64 COMMENT '总高度 (m)',
    total_layers     UInt8 COMMENT '总层数',
    base_width       Float64 COMMENT '底宽 (m)',
    base_depth       Float64 COMMENT '底深 (m)',
    total_weight     Float64 COMMENT '总重量 (吨)',
    design_load      Float64 COMMENT '设计荷载 (kN)',
    design_wind_speed Float64 COMMENT '设计风速 (m/s)',
    material_strength Float64 COMMENT '材料强度 (MPa)',
    elastic_modulus  Float64 COMMENT '弹性模量 (MPa)',
    poisson_ratio    Float64 COMMENT '泊松比',
    create_time      DateTime DEFAULT now()
)
ENGINE = ReplacingMergeTree(create_time)
ORDER BY tower_id;

-- 初始化默认攻城塔元数据
INSERT INTO towers_metadata (tower_id, tower_name, build_date, material, total_height, total_layers,
    base_width, base_depth, total_weight, design_load, design_wind_speed, material_strength,
    elastic_modulus, poisson_ratio) VALUES
(1, '临冲吕公车-一号', '1450-03-15', '松木+铁木', 18.5, 5, 6.2, 4.8, 28.5, 850.0, 35.0, 45.0, 12000.0, 0.38),
(2, '临冲吕公车-二号', '1452-07-22', '柏木+楠木', 21.0, 6, 6.8, 5.2, 36.8, 1020.0, 40.0, 52.0, 13500.0, 0.36);

-- 创建物化视图：按分钟统计
CREATE MATERIALIZED VIEW IF NOT EXISTS sensor_stats_minutely
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (tower_id, layer_id, timestamp)
AS
SELECT
    toStartOfMinute(timestamp) AS timestamp,
    tower_id,
    layer_id,
    max(stress_von_mises) AS max_stress,
    avg(stress_von_mises) AS avg_stress,
    max(tilt_total) AS max_tilt,
    avg(tilt_total) AS avg_tilt,
    max(wind_speed) AS max_wind_speed,
    avg(wind_speed) AS avg_wind_speed,
    max(ground_pressure) AS max_ground_pressure,
    count() AS sample_count
FROM sensor_data
GROUP BY tower_id, layer_id, timestamp;

-- 创建用户和权限
CREATE USER IF NOT EXISTS tower_user IDENTIFIED WITH sha256_password BY 'tower_secure_2024';
GRANT ALL ON siege_tower.* TO tower_user;

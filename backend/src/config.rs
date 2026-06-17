use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::path::Path;

static CONFIG: OnceLock<AppConfig> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub clickhouse: ClickHouseConfig,
    pub mqtt: MqttConfig,
    pub tower: TowerConfigRoot,
    pub soil: SoilConfigRoot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickHouseConfig {
    pub url: String,
    pub user: String,
    pub password: String,
    pub database: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttConfig {
    pub broker: String,
    pub port: u16,
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub alert_topic: String,
    pub sensor_topic: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TowerConfigRoot {
    pub towers: Vec<TowerConfigEntry>,
    pub global_simulation: GlobalSimConfig,
    pub alert_thresholds: AlertThresholds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TowerConfigEntry {
    pub tower_id: u32,
    pub tower_name: String,
    pub build_date: String,
    pub material: String,
    pub material_description: String,
    pub total_height: f64,
    pub total_layers: u8,
    pub base_width: f64,
    pub base_depth: f64,
    pub total_weight: f64,
    pub design_load: f64,
    pub design_wind_speed: f64,
    pub material_strength: f64,
    pub elastic_modulus: f64,
    pub poisson_ratio: f64,
    pub shear_modulus: f64,
    pub density_kgm3: f64,
    pub damping_ratio: f64,
    pub design_standard: DesignStandard,
    pub operational_limits: OperationalLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignStandard {
    pub safety_factor_min: f64,
    pub second_order_enabled: bool,
    pub fem_element_size: f64,
    pub arc_length_steps: usize,
    pub max_load_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationalLimits {
    pub max_tilt_deg: f64,
    pub max_wind_speed_mps: f64,
    pub min_ground_bearing_kpa: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSimConfig {
    pub gravity: f64,
    pub air_density: f64,
    pub wind_drag_coefficient: f64,
    pub wind_exposure_coefficient: f64,
    pub terrain_roughness: f64,
    pub safety_factor_min: f64,
    pub second_order_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    pub tilt_warning_deg: f64,
    pub tilt_danger_deg: f64,
    pub stress_warning_ratio: f64,
    pub stress_danger_ratio: f64,
    pub wind_warning_ratio: f64,
    pub wind_danger_ratio: f64,
    pub ground_warning_ratio: f64,
    pub ground_danger_ratio: f64,
    pub vibration_resonance_bandwidth_pct: f64,
    pub vibration_danger_amplitude: f64,
    pub cooldown_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoilConfigRoot {
    pub soil_types: Vec<SoilConfigEntry>,
    pub analysis_defaults: SoilAnalysisDefaults,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoilConfigEntry {
    #[serde(rename = "type")]
    pub soil_type: String,
    pub name_cn: String,
    pub description: String,
    pub bearing_capacity_ref_kpa: f64,
    pub friction_coefficient: f64,
    pub compressibility_index: f64,
    pub terzaghi_params: TerzaghiParams,
    pub settlement_params: SettlementParams,
    pub operational: SoilOperationalLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerzaghiParams {
    pub c_ref_kpa: f64,
    pub phi_ref_deg: f64,
    pub gamma_sat_knm3: f64,
    pub gamma_dry_knm3: f64,
    pub w_opt_pct: f64,
    pub w_sat_pct: f64,
    pub k_c: f64,
    pub k_phi: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementParams {
    pub initial_void_ratio: f64,
    pub compression_index: f64,
    pub recompression_index: f64,
    pub coefficient_of_consolidation_cm2s: f64,
    pub secondary_compression_index: f64,
    pub layer_thickness_m: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoilOperationalLimits {
    pub minimum_bearing_kpa: f64,
    pub max_settlement_mm: f64,
    pub max_differential_settlement_mm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoilAnalysisDefaults {
    pub default_moisture_pct: f64,
    pub foundation_width_m: f64,
    pub foundation_depth_m: f64,
    pub load_incline_deg: f64,
    pub settlement_combined_factor: f64,
}

fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
fn env_or_str(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn read_json_file<T: for<'de> serde::Deserialize<'de>>(path: &str) -> Result<T, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(Path::new(path))
        .map_err(|e| format!("读取配置文件 {} 失败: {}", path, e))?;
    let val: T = serde_json::from_str(&content)
        .map_err(|e| format!("解析配置文件 {} 失败: {}", path, e))?;
    Ok(val)
}

impl AppConfig {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let _ = dotenvy::dotenv().ok();

        let base_dir = std::env::var("SIEGE_CONFIG_DIR")
            .unwrap_or_else(|_| "config".to_string());

        let tower_config: TowerConfigRoot = read_json_file(&format!("{}/tower_config.json", base_dir))?;
        let soil_config: SoilConfigRoot = read_json_file(&format!("{}/soil_config.json", base_dir))?;

        Ok(Self {
            server: ServerConfig {
                host: env_or_str("SIEGE_SERVER_HOST", "0.0.0.0".into()),
                port: env_or("SIEGE_SERVER_PORT", 8080u16),
            },
            clickhouse: ClickHouseConfig {
                url: env_or_str("SIEGE_CLICKHOUSE_URL", "http://localhost:8123".into()),
                user: env_or_str("SIEGE_CLICKHOUSE_USER", "default".into()),
                password: env_or_str("SIEGE_CLICKHOUSE_PASSWORD", "".into()),
                database: env_or_str("SIEGE_CLICKHOUSE_DATABASE", "siege_tower".into()),
            },
            mqtt: MqttConfig {
                broker: env_or_str("SIEGE_MQTT_BROKER", "localhost".into()),
                port: env_or("SIEGE_MQTT_PORT", 1883u16),
                client_id: env_or_str("SIEGE_MQTT_CLIENT_ID", "siege-tower-server".into()),
                username: std::env::var("SIEGE_MQTT_USERNAME").ok(),
                password: std::env::var("SIEGE_MQTT_PASSWORD").ok(),
                alert_topic: env_or_str("SIEGE_MQTT_ALERT_TOPIC", "siege/tower/alert".into()),
                sensor_topic: env_or_str("SIEGE_MQTT_SENSOR_TOPIC", "siege/tower/sensor".into()),
            },
            tower: tower_config,
            soil: soil_config,
        })
    }

    pub fn init() -> &'static Self {
        CONFIG.get_or_init(|| Self::load().expect("配置加载失败"))
    }

    pub fn get() -> &'static Self {
        CONFIG.get().expect("Config not initialized")
    }

    pub fn get_tower(&self, tower_id: u32) -> Option<&TowerConfigEntry> {
        self.tower.towers.iter().find(|t| t.tower_id == tower_id)
    }

    pub fn get_soil(&self, soil_type: &str) -> Option<&SoilConfigEntry> {
        self.soil.soil_types.iter().find(|s| s.soil_type == soil_type)
    }
}

use crate::config::{AppConfig, AlertThresholds};
use crate::models::{SensorData, StructureAnalysis, TowerMetadata};

pub struct StabilityAnalyzer;

impl StabilityAnalyzer {
    pub fn new() -> Self {
        StabilityAnalyzer
    }

    pub fn calculate_safety_factor(
        &self,
        tower: &TowerMetadata,
        max_stress: f64,
        material_strength: f64,
        second_order_coef: f64,
    ) -> f64 {
        let adjusted_stress = max_stress * second_order_coef;
        let sf_stress = material_strength / adjusted_stress.max(0.1);
        sf_stress.min(10.0)
    }

    pub fn calculate_second_order_effect(
        &self,
        tower: &TowerMetadata,
        axial_load: f64,
        deflection: f64,
    ) -> f64 {
        let h = tower.total_height;
        let e = tower.elastic_modulus;
        let i = tower.base_width * tower.base_depth.powi(3) / 12.0;
        let p_cr = std::f64::consts::PI.powi(2) * e * i / (h * h);
        let axial_kn = axial_load * 9.81;

        if axial_kn >= p_cr {
            return 10.0;
        }

        let ratio = axial_kn / p_cr;
        1.0 / (1.0 - ratio).max(0.05)
    }

    pub fn calculate_wind_resistance_limit(
        &self,
        tower: &TowerMetadata,
        safety_factor_min: f64,
        air_density: f64,
        cd: f64,
    ) -> f64 {
        let allowable_stress = tower.material_strength / safety_factor_min;
        let base_area = tower.base_width * tower.base_depth;
        let section_modulus = tower.base_width * tower.base_depth.powi(2) / 6.0;
        let avg_height = tower.total_height / 2.0;
        let projected_area = tower.base_width * tower.total_height * 0.7;

        let allowable_bending = allowable_stress * section_modulus;
        let allowable_base_shear = allowable_stress * base_area * 0.3;

        let wind_from_bending = (2.0 * allowable_bending / (cd * air_density * projected_area * avg_height)).sqrt();
        let wind_from_shear = (2.0 * allowable_base_shear / (cd * air_density * projected_area)).sqrt();

        wind_from_bending.min(wind_from_shear).min(80.0)
    }

    pub fn calculate_natural_frequency(&self, tower: &TowerMetadata) -> f64 {
        let h = tower.total_height;
        let e = tower.elastic_modulus;
        let i = tower.base_width * tower.base_depth.powi(3) / 12.0;
        let m = tower.total_weight * 1000.0;

        let omega_1 = 1.875_f64.powi(2) * (e * i / (m * h.powi(3))).sqrt();
        omega_1 / (2.0 * std::f64::consts::PI)
    }

    pub fn calculate_overturning_ratio(
        &self,
        tower: &TowerMetadata,
        wind_speed: f64,
        tilt_deg: f64,
        air_density: f64,
        cd: f64,
    ) -> f64 {
        let weight_kn = tower.total_weight * 9.81;
        let moment_arm_resist = tower.base_depth / 2.0;
        let resist_moment = weight_kn * moment_arm_resist;

        let q = 0.5 * air_density * cd * wind_speed * wind_speed;
        let projected_area = tower.base_width * tower.total_height * 0.7;
        let total_wind_force = q * projected_area / 1000.0;
        let wind_height = tower.total_height * 0.6;
        let h = tower.total_height;
        let tilt_rad = tilt_deg.to_radians();
        let tilt_moment = weight_kn * h / 2.0 * tilt_rad.sin();
        let overturn_moment = total_wind_force * wind_height + tilt_moment;

        (resist_moment / overturn_moment.max(1.0)).min(10.0)
    }

    pub fn calculate_stability_margin(
        &self,
        safety_factor: f64,
        safety_factor_min: f64,
        wind_ratio: f64,
        ground_ratio: f64,
    ) -> f64 {
        let sf_margin = if safety_factor >= safety_factor_min {
            (safety_factor - safety_factor_min) / safety_factor_min * 100.0
        } else {
            (safety_factor - safety_factor_min) / safety_factor_min * 100.0
        };

        let wind_margin = (1.0 - wind_ratio) * 100.0;
        let ground_margin = (1.0 - ground_ratio) * 100.0;

        let weights = [0.4, 0.35, 0.25];
        let margin = sf_margin * weights[0] + wind_margin * weights[1] + ground_margin * weights[2];
        margin.max(-100.0).min(200.0)
    }

    pub fn check_stability(
        &self,
        tower: &TowerMetadata,
        sensor_data: &[SensorData],
        config: &AppConfig,
    ) -> StructureAnalysis {
        let sim = &config.tower.global_simulation;
        let mut max_stress = 0.0f64;
        let mut max_stress_layer: u8 = 1;
        let mut max_tilt = 0.0f64;
        let mut max_tilt_layer: u8 = 1;
        let mut max_wind_speed = 0.0f64;
        let mut max_ground_pressure = 0.0f64;

        for sd in sensor_data {
            if sd.stress_von_mises > max_stress {
                max_stress = sd.stress_von_mises;
                max_stress_layer = sd.layer_id;
            }
            if sd.tilt_total > max_tilt {
                max_tilt = sd.tilt_total;
                max_tilt_layer = sd.layer_id;
            }
            if sd.wind_speed_mps > max_wind_speed {
                max_wind_speed = sd.wind_speed_mps;
            }
            if sd.ground_pressure > max_ground_pressure {
                max_ground_pressure = sd.ground_pressure;
            }
        }

        let deflection = max_tilt.to_radians() * tower.total_height * 0.5;
        let second_order_coef = if sim.second_order_enabled {
            self.calculate_second_order_effect(tower, tower.total_weight, deflection)
        } else {
            1.0
        };

        let safety_factor = self.calculate_safety_factor(
            tower,
            max_stress,
            tower.material_strength,
            second_order_coef,
        );

        let wind_resistance_limit = self.calculate_wind_resistance_limit(
            tower,
            sim.safety_factor_min,
            sim.air_density,
            sim.wind_drag_coefficient,
        );

        let current_wind_factor = max_wind_speed / wind_resistance_limit;
        let overturn_ratio = self.calculate_overturning_ratio(
            tower,
            max_wind_speed,
            max_tilt,
            sim.air_density,
            sim.wind_drag_coefficient,
        );

        let soil_bearing = if let Some(first) = sensor_data.first() {
            match first.soil_type.as_str() {
                "sand" => 180.0,
                "clay" => 120.0,
                "silt" => 90.0,
                "rock" => 800.0,
                _ => 200.0,
            }
        } else {
            200.0
        };
        let ground_capacity_ratio = max_ground_pressure / soil_bearing;

        let effective_sf = safety_factor / second_order_coef;
        let is_stable = if effective_sf >= sim.safety_factor_min
            && current_wind_factor <= 0.95
            && ground_capacity_ratio <= 0.95
            && overturn_ratio >= 1.2 {
            1
        } else {
            0
        };

        let stability_margin = self.calculate_stability_margin(
            effective_sf,
            sim.safety_factor_min,
            current_wind_factor,
            ground_capacity_ratio,
        );

        let natural_frequency = self.calculate_natural_frequency(tower);

        StructureAnalysis {
            timestamp: chrono::Utc::now(),
            tower_id: tower.tower_id,
            tower_name: tower.tower_name.clone(),
            safety_factor: effective_sf,
            critical_stress: tower.material_strength,
            max_stress,
            max_stress_layer,
            max_tilt,
            max_tilt_layer,
            wind_resistance_limit,
            current_wind_factor,
            ground_capacity_ratio,
            is_stable,
            stability_margin,
            second_order_effect: second_order_coef,
            natural_frequency,
            damping_ratio: 0.02 + tower.poisson_ratio * 0.05,
        }
    }
}

impl Default for StabilityAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{SensorData, TowerMetadata};
    use crate::config::{AppConfig, GlobalSimConfig, TowerConfigRoot, AlertThresholds,
        SoilConfigRoot, SoilAnalysisDefaults, ServerConfig, ClickHouseConfig, MqttConfig};

    fn test_tower_siege() -> TowerMetadata {
        TowerMetadata {
            tower_id: 1,
            tower_name: "临冲吕公车".to_string(),
            build_date: "1450-03-15".to_string(),
            material: "杉木+铁木".to_string(),
            total_height: 18.5,
            total_layers: 5,
            base_width: 6.2,
            base_depth: 4.8,
            total_weight: 28.5,
            design_load: 850.0,
            design_wind_speed: 35.0,
            material_strength: 38.0,
            elastic_modulus: 9500.0,
            poisson_ratio: 0.35,
        }
    }

    fn test_tower_ladder() -> TowerMetadata {
        TowerMetadata {
            tower_id: 3,
            tower_name: "云梯车".to_string(),
            build_date: "1368-05-10".to_string(),
            material: "松木+竹".to_string(),
            total_height: 12.0,
            total_layers: 3,
            base_width: 3.5,
            base_depth: 2.8,
            total_weight: 8.5,
            design_load: 280.0,
            design_wind_speed: 25.0,
            material_strength: 36.0,
            elastic_modulus: 10500.0,
            poisson_ratio: 0.35,
        }
    }

    fn test_tower_ram() -> TowerMetadata {
        TowerMetadata {
            tower_id: 4,
            tower_name: "冲车".to_string(),
            build_date: "0230-01-01".to_string(),
            material: "栎木+铁箍".to_string(),
            total_height: 5.5,
            total_layers: 2,
            base_width: 4.2,
            base_depth: 3.0,
            total_weight: 15.0,
            design_load: 450.0,
            design_wind_speed: 20.0,
            material_strength: 48.0,
            elastic_modulus: 13800.0,
            poisson_ratio: 0.35,
        }
    }

    fn test_tower_modern_crane() -> TowerMetadata {
        TowerMetadata {
            tower_id: 5,
            tower_name: "现代塔吊".to_string(),
            build_date: "2024-01-15".to_string(),
            material: "Q345B钢材(GB/T1591)".to_string(),
            total_height: 60.0,
            total_layers: 12,
            base_width: 8.0,
            base_depth: 8.0,
            total_weight: 95.0,
            design_load: 8000.0,
            design_wind_speed: 55.0,
            material_strength: 295.0,
            elastic_modulus: 206000.0,
            poisson_ratio: 0.30,
        }
    }

    fn test_config() -> AppConfig {
        AppConfig {
            server: ServerConfig { host: "127.0.0.1".to_string(), port: 8080 },
            clickhouse: ClickHouseConfig {
                url: "http://localhost:8123".to_string(),
                user: "default".to_string(),
                password: "".to_string(),
                database: "default".to_string(),
            },
            mqtt: MqttConfig {
                broker: "localhost".to_string(),
                port: 1883,
                client_id: "test".to_string(),
                username: None,
                password: None,
                alert_topic: "alert".to_string(),
                sensor_topic: "sensor".to_string(),
            },
            tower: TowerConfigRoot {
                towers: vec![],
                global_simulation: GlobalSimConfig {
                    gravity: 9.81,
                    air_density: 1.225,
                    wind_drag_coefficient: 1.3,
                    wind_exposure_coefficient: 1.0,
                    terrain_roughness: 0.1,
                    safety_factor_min: 2.0,
                    second_order_enabled: true,
                },
                alert_thresholds: AlertThresholds {
                    tilt_warning_deg: 1.0,
                    tilt_danger_deg: 2.5,
                    stress_warning_ratio: 0.75,
                    stress_danger_ratio: 0.9,
                    wind_warning_ratio: 0.7,
                    wind_danger_ratio: 0.9,
                    ground_warning_ratio: 0.7,
                    ground_danger_ratio: 0.9,
                    vibration_resonance_bandwidth_pct: 10.0,
                    vibration_danger_amplitude: 5.0,
                    cooldown_seconds: 300,
                },
            },
            soil: SoilConfigRoot {
                soil_types: vec![],
                analysis_defaults: SoilAnalysisDefaults {
                    default_moisture_pct: 20.0,
                    foundation_width_m: 5.0,
                    foundation_depth_m: 2.0,
                    load_incline_deg: 0.0,
                    settlement_combined_factor: 1.0,
                },
            },
        }
    }

    fn make_sensor_data(layer: u8, stress: f64, tilt: f64, wind: f64, ground: f64) -> SensorData {
        SensorData {
            tower_id: 1,
            tower_name: "".to_string(),
            layer_id: layer,
            layer_name: format!("L{}", layer),
            timestamp: chrono::Utc::now(),
            stress_x: stress * 0.6,
            stress_y: stress * 0.5,
            stress_z: stress * 0.8,
            stress_von_mises: stress,
            tilt_x: tilt * 0.7,
            tilt_y: tilt * 0.7,
            tilt_total: tilt,
            wind_load_x: 0.0,
            wind_load_y: 0.0,
            wind_speed_mps: wind,
            ground_pressure: ground,
            ground_settlement: 0.0,
            temperature_c: 25.0,
            humidity_pct: 60.0,
            vibration_freq: 2.5,
            vibration_amp: 0.5,
            soil_type: "loam".to_string(),
            is_alert: 0,
            alert_level: 0,
        }
    }

    fn analyzer() -> StabilityAnalyzer {
        StabilityAnalyzer::new()
    }

    #[test]
    fn test_safety_factor_normal() {
        let tower = test_tower_siege();
        let sf = analyzer().calculate_safety_factor(&tower, 20.0, tower.material_strength, 1.0);
        assert!(sf > 1.0);
        assert!(sf <= 10.0);
    }

    #[test]
    fn test_safety_factor_high_stress() {
        let tower = test_tower_siege();
        let sf = analyzer().calculate_safety_factor(&tower, 100.0, tower.material_strength, 1.0);
        assert!(sf < 5.0);
        assert!(sf > 0.0);
    }

    #[test]
    fn test_safety_factor_second_order_magnification() {
        let tower = test_tower_siege();
        let sf1 = analyzer().calculate_safety_factor(&tower, 20.0, tower.material_strength, 1.0);
        let sf2 = analyzer().calculate_safety_factor(&tower, 20.0, tower.material_strength, 1.5);
        assert!(sf2 < sf1);
    }

    #[test]
    fn test_safety_factor_zero_stress() {
        let tower = test_tower_siege();
        let sf = analyzer().calculate_safety_factor(&tower, 0.0, tower.material_strength, 1.0);
        assert!(sf <= 10.0);
        assert!(sf > 0.0);
    }

    #[test]
    fn test_second_order_effect_normal() {
        let tower = test_tower_siege();
        let coef = analyzer().calculate_second_order_effect(&tower, 12.0, 0.05);
        assert!(coef >= 1.0);
        assert!(coef < 10.0);
    }

    #[test]
    fn test_second_order_effect_small_load() {
        let tower = test_tower_siege();
        let coef = analyzer().calculate_second_order_effect(&tower, 0.1, 0.01);
        assert!((coef - 1.0).abs() < 1.0);
    }

    #[test]
    fn test_wind_resistance_limit_tower_types() {
        let config = test_config();
        let sim = &config.tower.global_simulation;

        let siege_wind = analyzer().calculate_wind_resistance_limit(
            &test_tower_siege(), sim.safety_factor_min, sim.air_density, sim.wind_drag_coefficient);
        let ladder_wind = analyzer().calculate_wind_resistance_limit(
            &test_tower_ladder(), sim.safety_factor_min, sim.air_density, sim.wind_drag_coefficient);
        let ram_wind = analyzer().calculate_wind_resistance_limit(
            &test_tower_ram(), sim.safety_factor_min, sim.air_density, sim.wind_drag_coefficient);
        let crane_wind = analyzer().calculate_wind_resistance_limit(
            &test_tower_modern_crane(), sim.safety_factor_min, sim.air_density, sim.wind_drag_coefficient);

        assert!(siege_wind > 0.0);
        assert!(ladder_wind > 0.0);
        assert!(ram_wind > 0.0);
        assert!(crane_wind > 0.0);
        assert!(crane_wind > siege_wind || crane_wind > ladder_wind);
        assert!(siege_wind <= 80.0);
        assert!(crane_wind <= 80.0);
    }

    #[test]
    fn test_natural_frequency_tower_types() {
        let siege_freq = analyzer().calculate_natural_frequency(&test_tower_siege());
        let ladder_freq = analyzer().calculate_natural_frequency(&test_tower_ladder());
        let ram_freq = analyzer().calculate_natural_frequency(&test_tower_ram());
        let crane_freq = analyzer().calculate_natural_frequency(&test_tower_modern_crane());

        assert!(siege_freq > 0.0);
        assert!(ladder_freq > 0.0);
        assert!(ram_freq > 0.0);
        assert!(crane_freq > 0.0);

        assert!(ram_freq > siege_freq);
        assert!(siege_freq > crane_freq || crane_freq < 1.0);
    }

    #[test]
    fn test_natural_frequency_simple_check() {
        let mut tower = test_tower_siege();
        let f1 = analyzer().calculate_natural_frequency(&tower);
        tower.total_weight *= 2.0;
        let f2 = analyzer().calculate_natural_frequency(&tower);
        assert!(f2 < f1);
        assert!((f1 / f2 - 1.4).abs() < 0.2);
    }

    #[test]
    fn test_overturning_ratio_normal() {
        let tower = test_tower_siege();
        let ratio = analyzer().calculate_overturning_ratio(&tower, 15.0, 0.0, 1.225, 1.3);
        assert!(ratio > 1.0);
        assert!(ratio <= 10.0);
    }

    #[test]
    fn test_overturning_ratio_wind_dependency() {
        let tower = test_tower_siege();
        let ratio_calm = analyzer().calculate_overturning_ratio(&tower, 5.0, 0.0, 1.225, 1.3);
        let ratio_storm = analyzer().calculate_overturning_ratio(&tower, 30.0, 0.0, 1.225, 1.3);
        assert!(ratio_storm < ratio_calm);
    }

    #[test]
    fn test_overturning_ratio_tilt_reduces_stability() {
        let tower = test_tower_siege();
        let ratio_up = analyzer().calculate_overturning_ratio(&tower, 10.0, 0.0, 1.225, 1.3);
        let ratio_tilted = analyzer().calculate_overturning_ratio(&tower, 10.0, 3.0, 1.225, 1.3);
        assert!(ratio_tilted < ratio_up);
    }

    #[test]
    fn test_stability_margin_positive() {
        let margin = analyzer().calculate_stability_margin(3.0, 2.0, 0.5, 0.5);
        assert!(margin > 0.0);
    }

    #[test]
    fn test_stability_margin_negative() {
        let margin = analyzer().calculate_stability_margin(1.0, 2.0, 1.0, 1.0);
        assert!(margin < 0.0);
    }

    #[test]
    fn test_stability_margin_clamped() {
        let margin_low = analyzer().calculate_stability_margin(0.0, 2.0, 2.0, 2.0);
        let margin_high = analyzer().calculate_stability_margin(100.0, 2.0, 0.0, 0.0);
        assert!(margin_low >= -100.0);
        assert!(margin_high <= 200.0);
    }

    #[test]
    fn test_check_stability_stable_case() {
        let tower = test_tower_siege();
        let config = test_config();
        let sensors = vec![
            make_sensor_data(1, 10.0, 0.2, 8.0, 100.0),
            make_sensor_data(2, 12.0, 0.3, 8.0, 100.0),
            make_sensor_data(3, 15.0, 0.4, 8.0, 100.0),
        ];

        let result = analyzer().check_stability(&tower, &sensors, &config);
        assert_eq!(result.tower_id, tower.tower_id);
        assert!(result.safety_factor > 1.0);
        assert!(result.wind_resistance_limit > 0.0);
        assert!(result.natural_frequency > 0.0);
    }

    #[test]
    fn test_check_stability_unstable_high_stress() {
        let tower = test_tower_siege();
        let config = test_config();
        let sensors = vec![
            make_sensor_data(1, 100.0, 0.2, 10.0, 100.0),
            make_sensor_data(2, 120.0, 0.3, 10.0, 100.0),
            make_sensor_data(3, 150.0, 0.4, 10.0, 100.0),
        ];

        let result = analyzer().check_stability(&tower, &sensors, &config);
        assert!(result.safety_factor < 2.0);
    }

    #[test]
    fn test_check_stability_max_stress_layer() {
        let tower = test_tower_siege();
        let config = test_config();
        let sensors = vec![
            make_sensor_data(1, 5.0, 0.1, 8.0, 100.0),
            make_sensor_data(3, 25.0, 0.3, 8.0, 100.0),
            make_sensor_data(5, 15.0, 0.2, 8.0, 100.0),
        ];

        let result = analyzer().check_stability(&tower, &sensors, &config);
        assert_eq!(result.max_stress_layer, 3);
        assert_eq!(result.max_stress, 25.0);
    }

    #[test]
    fn test_check_stability_max_tilt_layer() {
        let tower = test_tower_siege();
        let config = test_config();
        let sensors = vec![
            make_sensor_data(1, 10.0, 0.1, 8.0, 100.0),
            make_sensor_data(3, 10.0, 2.0, 8.0, 100.0),
            make_sensor_data(5, 10.0, 0.5, 8.0, 100.0),
        ];

        let result = analyzer().check_stability(&tower, &sensors, &config);
        assert_eq!(result.max_tilt_layer, 3);
        assert_eq!(result.max_tilt, 2.0);
    }

    #[test]
    fn test_dynasty_comparison_safety_order() {
        let config = test_config();
        let sim = &config.tower.global_simulation;
        let sensors = vec![make_sensor_data(1, 10.0, 0.1, 10.0, 100.0)];

        let siege_result = analyzer().check_stability(&test_tower_siege(), &sensors, &config);
        let ladder_result = analyzer().check_stability(&test_tower_ladder(), &sensors, &config);
        let ram_result = analyzer().check_stability(&test_tower_ram(), &sensors, &config);

        assert!(siege_result.safety_factor > 0.0);
        assert!(ladder_result.safety_factor > 0.0);
        assert!(ram_result.safety_factor > 0.0);
    }

    #[test]
    fn test_cross_era_material_efficiency() {
        let ancient = test_tower_siege();
        let modern = test_tower_modern_crane();

        let ancient_strength_modulus_ratio = ancient.material_strength / ancient.elastic_modulus * 1000.0;
        let modern_strength_modulus_ratio = modern.material_strength / modern.elastic_modulus * 1000.0;

        assert!(ancient_strength_modulus_ratio > modern_strength_modulus_ratio);
        assert!(modern.elastic_modulus > ancient.elastic_modulus * 10.0);
        assert!(modern.material_strength > ancient.material_strength * 5.0);

        let ancient_load_efficiency = ancient.design_load / (ancient.total_weight * 9.81);
        let modern_load_efficiency = modern.design_load / (modern.total_weight * 9.81);
        assert!(modern_load_efficiency > ancient_load_efficiency);
    }

    #[test]
    fn test_cross_era_load_to_weight_ratio() {
        let ancient = test_tower_siege();
        let modern = test_tower_modern_crane();

        let ancient_ratio = ancient.design_load / ancient.total_weight;
        let modern_ratio = modern.design_load / modern.total_weight;

        assert!(modern_ratio > ancient_ratio);
    }

    #[test]
    fn test_empty_sensor_data() {
        let tower = test_tower_siege();
        let config = test_config();
        let sensors: Vec<SensorData> = vec![];

        let result = analyzer().check_stability(&tower, &sensors, &config);
        assert_eq!(result.max_stress, 0.0);
        assert_eq!(result.max_tilt, 0.0);
    }

    #[test]
    fn test_second_order_disabled() {
        let mut config = test_config();
        config.tower.global_simulation.second_order_enabled = false;
        let tower = test_tower_siege();
        let sensors = vec![make_sensor_data(1, 20.0, 1.0, 15.0, 100.0)];

        let result = analyzer().check_stability(&tower, &sensors, &config);
        assert_eq!(result.second_order_effect, 1.0);
    }

    #[test]
    fn test_damping_ratio_calculated() {
        let tower = test_tower_siege();
        let config = test_config();
        let sensors = vec![make_sensor_data(1, 10.0, 0.2, 10.0, 100.0)];

        let result = analyzer().check_stability(&tower, &sensors, &config);
        assert!(result.damping_ratio > 0.0);
        assert!(result.damping_ratio < 1.0);
    }

    #[test]
    fn test_all_tower_material_strengths_positive() {
        let towers = vec![
            test_tower_siege(),
            test_tower_ladder(),
            test_tower_ram(),
            test_tower_modern_crane(),
        ];

        for tower in towers {
            assert!(tower.material_strength > 0.0);
            assert!(tower.elastic_modulus > 0.0);
            assert!(tower.total_height > 0.0);
            assert!(tower.total_weight > 0.0);
        }
    }

    #[test]
    fn test_weight_efficiency_dynasty_comparison() {
        let siege = test_tower_siege();
        let ladder = test_tower_ladder();
        let ram = test_tower_ram();

        let siege_eff = siege.design_load / siege.total_weight;
        let ladder_eff = ladder.design_load / ladder.total_weight;
        let ram_eff = ram.design_load / ram.total_weight;

        assert!(siege_eff > 0.0);
        assert!(ladder_eff > 0.0);
        assert!(ram_eff > 0.0);
        assert!(ram_eff > siege_eff);
    }

    #[test]
    fn test_height_to_base_ratio() {
        let siege = test_tower_siege();
        let ladder = test_tower_ladder();
        let ram = test_tower_ram();
        let crane = test_tower_modern_crane();

        let siege_ratio = siege.total_height / siege.base_width;
        let ladder_ratio = ladder.total_height / ladder.base_width;
        let ram_ratio = ram.total_height / ram.base_width;
        let crane_ratio = crane.total_height / crane.base_width;

        assert!(ladder_ratio > siege_ratio);
        assert!(siege_ratio > ram_ratio);
        assert!(crane_ratio > ladder_ratio);
    }
}

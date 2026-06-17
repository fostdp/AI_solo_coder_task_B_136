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
        let total_wind_force = q * projected_area;
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

use crate::models::{MoatAnalysis, SoilType, TowerMetadata};
use chrono::Utc;
use std::f64::consts::PI;

pub struct MoatAnalyzer;

impl MoatAnalyzer {
    pub fn new() -> Self {
        MoatAnalyzer
    }

    pub fn analyze(
        &self,
        tower: &TowerMetadata,
        soil_type: &SoilType,
        moat_distance: f64,
        moat_depth: f64,
        water_table_depth: f64,
        wind_speed: f64,
        tilt_deg: f64,
    ) -> MoatAnalysis {
        let bearing_capacity_reduction =
            1.0 - 0.5 * (water_table_depth / moat_depth).min(1.0);

        let base_bearing = soil_type.bearing_capacity_kpa();
        let effective_bearing_capacity = base_bearing * bearing_capacity_reduction;

        let slope_stability_factor = self.calculate_slope_stability(
            soil_type,
            moat_depth,
            moat_distance,
            water_table_depth,
        );

        let settlement_increase_pct = self.calculate_settlement_increase(soil_type, water_table_depth, moat_depth);

        let lateral_displacement = self.calculate_lateral_displacement(
            tower,
            soil_type,
            moat_distance,
            moat_depth,
            wind_speed,
            tilt_deg,
        );

        let applied_pressure = tower.total_weight * 9.81 / (tower.base_width * tower.base_depth);
        let bearing_sf = effective_bearing_capacity / applied_pressure.max(0.1);
        let overall_sf = bearing_sf.min(slope_stability_factor);

        let risk_level = if overall_sf >= 3.0 {
            1
        } else if overall_sf >= 2.0 {
            2
        } else if overall_sf >= 1.5 {
            3
        } else if overall_sf >= 1.0 {
            4
        } else {
            5
        };

        let recommendations = self.generate_recommendations(
            overall_sf,
            slope_stability_factor,
            bearing_sf,
            lateral_displacement,
            settlement_increase_pct,
            moat_distance,
        );

        MoatAnalysis {
            timestamp: Utc::now(),
            tower_id: tower.tower_id,
            moat_distance_m: moat_distance,
            moat_depth_m: moat_depth,
            water_table_depth_m: water_table_depth,
            soil_type: soil_type.as_str().to_string(),
            bearing_capacity_reduction,
            effective_bearing_capacity,
            slope_stability_factor,
            settlement_increase_pct,
            lateral_displacement_mm: lateral_displacement,
            overall_safety_factor: overall_sf,
            risk_level,
            recommendations,
        }
    }

    fn calculate_slope_stability(
        &self,
        soil_type: &SoilType,
        moat_depth: f64,
        moat_distance: f64,
        water_table_depth: f64,
    ) -> f64 {
        let params = soil_type.params();
        let c = params.c_ref;
        let phi_deg = params.phi_ref_deg;
        let phi = phi_deg.to_radians();
        let gamma = params.gamma_sat;

        let slope_angle = (moat_depth / moat_distance.max(0.5)).atan().min(PI / 3.0);
        let slope_length = (moat_depth.powi(2) + moat_distance.powi(2)).sqrt().max(1.0);

        let w = gamma * slope_length * moat_depth * 0.5;

        let water_force = if water_table_depth < moat_depth {
            let h_w = (moat_depth - water_table_depth).max(0.0);
            0.5 * 9.81 * h_w * h_w.max(1.0)
        } else {
            0.0
        };

        let beta = slope_angle * 0.5;

        let resisting = c * slope_length + w * slope_angle.cos() * phi.tan();
        let driving = w * slope_angle.sin() + water_force * beta.sin();

        (resisting / driving.max(0.1)).min(10.0)
    }

    fn calculate_settlement_increase(&self, soil_type: &SoilType, water_table_depth: f64, moat_depth: f64) -> f64 {
        let base_pct = match soil_type {
            SoilType::Sand => 20.0,
            SoilType::Clay => 60.0,
            SoilType::Silt => 50.0,
            SoilType::Rock => 10.0,
            SoilType::Loam => 35.0,
        };
        let saturation_factor = (1.0 - water_table_depth / moat_depth).max(0.0).min(1.0);
        base_pct + saturation_factor * (80.0 - base_pct)
    }

    fn calculate_lateral_displacement(
        &self,
        tower: &TowerMetadata,
        soil_type: &SoilType,
        moat_distance: f64,
        moat_depth: f64,
        wind_speed: f64,
        tilt_deg: f64,
    ) -> f64 {
        let q = tower.total_weight * 9.81 / (tower.base_width * tower.base_depth);
        let b = tower.base_width;
        let nu = tower.poisson_ratio;
        let e = soil_type.bearing_capacity_kpa() * 50.0;

        let proximity_factor = 1.0 + 2.0 * moat_depth / (moat_distance + moat_depth).max(1.0);

        let delta = q * b * (1.0 - nu * nu) / e.max(1.0) * proximity_factor;

        let wind_component = 0.613 * wind_speed * wind_speed * 1.3 * tower.total_height * 0.7
            / (e.max(1.0) * tower.base_width);
        let tilt_component = tilt_deg * tower.total_height * 0.1;

        (delta + wind_component + tilt_component).max(0.0) * 1000.0
    }

    fn generate_recommendations(
        &self,
        overall_sf: f64,
        slope_sf: f64,
        bearing_sf: f64,
        lateral_mm: f64,
        settlement_pct: f64,
        moat_distance: f64,
    ) -> Vec<String> {
        let mut recs = Vec::new();

        if overall_sf < 1.5 {
            recs.push("整体安全系数不足，建议增加基础面积或减轻结构荷载".to_string());
        }
        if slope_sf < 2.0 {
            recs.push("护坡稳定性不足，建议增设挡土墙或进行边坡加固".to_string());
        }
        if bearing_sf < 2.0 {
            recs.push("地基承载力安全裕度不足，建议采用桩基础或换填法处理地基".to_string());
        }
        if lateral_mm > 30.0 {
            recs.push(format!("侧向位移偏大({:.1}mm)，建议增加基础埋深或设置地锚", lateral_mm));
        }
        if settlement_pct > 50.0 {
            recs.push("沉降增幅较大，建议进行地基预压或采用复合地基方案".to_string());
        }
        if moat_distance < 5.0 {
            recs.push("塔基距护城河过近，建议增加安全距离或设置防护结构".to_string());
        }
        if overall_sf >= 3.0 && slope_sf >= 2.5 {
            recs.push("结构整体安全，可正常使用".to_string());
        }

        if recs.is_empty() {
            recs.push("当前条件下结构基本安全，建议定期监测".to_string());
        }

        recs
    }
}

impl Default for MoatAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

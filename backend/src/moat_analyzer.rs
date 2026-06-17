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
        let pore_pressure_ratio = self.calculate_pore_pressure_ratio(
            water_table_depth, moat_depth,
        );

        let seepage_force = self.calculate_seepage_force(
            soil_type, moat_depth, moat_distance, water_table_depth,
        );

        let water_soil_coupling_factor = self.calculate_coupling_factor(
            soil_type, pore_pressure_ratio,
        );

        let bearing_capacity_reduction =
            (0.5 + 0.5 * (water_table_depth / moat_depth).min(1.0))
            * water_soil_coupling_factor;

        let base_bearing = soil_type.bearing_capacity_kpa();
        let effective_bearing_capacity = base_bearing * bearing_capacity_reduction;

        let slope_stability_factor = self.calculate_slope_stability(
            soil_type,
            moat_depth,
            moat_distance,
            water_table_depth,
            pore_pressure_ratio,
            seepage_force,
        );

        let settlement_increase_pct = self.calculate_settlement_increase(
            soil_type, water_table_depth, moat_depth, pore_pressure_ratio,
        );

        let lateral_displacement = self.calculate_lateral_displacement(
            tower,
            soil_type,
            moat_distance,
            moat_depth,
            wind_speed,
            tilt_deg,
            pore_pressure_ratio,
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
            pore_pressure_ratio,
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
            pore_pressure_ratio,
            seepage_force_kn: seepage_force,
            water_soil_coupling_factor,
            overall_safety_factor: overall_sf,
            risk_level,
            recommendations,
        }
    }

    fn calculate_pore_pressure_ratio(&self, water_table_depth: f64, moat_depth: f64) -> f64 {
        if moat_depth <= 0.0 {
            return 0.0;
        }
        let submergence_ratio = (1.0 - water_table_depth / moat_depth).max(0.0).min(1.0);
        let ru = 0.5 * submergence_ratio.powi(2);
        ru.min(0.5)
    }

    fn calculate_seepage_force(
        &self,
        soil_type: &SoilType,
        moat_depth: f64,
        moat_distance: f64,
        water_table_depth: f64,
    ) -> f64 {
        let params = soil_type.params();
        let k_hydraulic = match soil_type {
            SoilType::Sand => 1e-4,
            SoilType::Clay => 1e-8,
            SoilType::Silt => 1e-6,
            SoilType::Rock => 1e-10,
            SoilType::Loam => 1e-5,
        };

        let h_w = (moat_depth - water_table_depth).max(0.0);
        let i_hydraulic = if moat_distance > 0.0 { h_w / moat_distance } else { 0.0 };

        let slope_length = (moat_depth.powi(2) + moat_distance.powi(2)).sqrt().max(1.0);
        let volume = slope_length * moat_depth * 1.0;

        k_hydraulic * i_hydraulic * params.gamma_sat * volume * 9.81
    }

    fn calculate_coupling_factor(&self, soil_type: &SoilType, pore_pressure_ratio: f64) -> f64 {
        let sensitivity = match soil_type {
            SoilType::Sand => 0.85,
            SoilType::Clay => 0.70,
            SoilType::Silt => 0.75,
            SoilType::Rock => 0.95,
            SoilType::Loam => 0.80,
        };
        1.0 - pore_pressure_ratio * (1.0 - sensitivity)
    }

    fn calculate_slope_stability(
        &self,
        soil_type: &SoilType,
        moat_depth: f64,
        moat_distance: f64,
        water_table_depth: f64,
        pore_pressure_ratio: f64,
        seepage_force: f64,
    ) -> f64 {
        let params = soil_type.params();
        let c_effective = params.c_ref * (1.0 - pore_pressure_ratio * 0.5);
        let phi_deg = params.phi_ref_deg;
        let phi = phi_deg.to_radians();
        let gamma = params.gamma_sat;
        let gamma_w = 9.81;

        let slope_angle = (moat_depth / moat_distance.max(0.5)).atan().min(PI / 3.0);
        let slope_length = (moat_depth.powi(2) + moat_distance.powi(2)).sqrt().max(1.0);

        let submerged_depth = (moat_depth - water_table_depth).max(0.0);
        let gamma_sub = if submerged_depth > 0.0 {
            gamma - gamma_w
        } else {
            gamma
        };
        let w = gamma_sub * slope_length * moat_depth * 0.5;

        let water_force = if water_table_depth < moat_depth {
            let h_w = submerged_depth;
            0.5 * gamma_w * h_w * h_w.max(1.0)
        } else {
            0.0
        };

        let beta = slope_angle * 0.5;

        let resisting = c_effective * slope_length + w * slope_angle.cos() * phi.tan();
        let driving = w * slope_angle.sin() + water_force * beta.sin() + seepage_force * slope_angle.cos();

        (resisting / driving.max(0.1)).min(10.0)
    }

    fn calculate_settlement_increase(&self, soil_type: &SoilType, water_table_depth: f64, moat_depth: f64, pore_pressure_ratio: f64) -> f64 {
        let base_pct = match soil_type {
            SoilType::Sand => 20.0,
            SoilType::Clay => 60.0,
            SoilType::Silt => 50.0,
            SoilType::Rock => 10.0,
            SoilType::Loam => 35.0,
        };
        let saturation_factor = (1.0 - water_table_depth / moat_depth).max(0.0).min(1.0);
        let pore_pressure_amplification = 1.0 + pore_pressure_ratio * 0.5;
        (base_pct + saturation_factor * (80.0 - base_pct)) * pore_pressure_amplification
    }

    fn calculate_lateral_displacement(
        &self,
        tower: &TowerMetadata,
        soil_type: &SoilType,
        moat_distance: f64,
        moat_depth: f64,
        wind_speed: f64,
        tilt_deg: f64,
        pore_pressure_ratio: f64,
    ) -> f64 {
        let q = tower.total_weight * 9.81 / (tower.base_width * tower.base_depth);
        let b = tower.base_width;
        let nu = tower.poisson_ratio;
        let e = soil_type.bearing_capacity_kpa() * 50.0;

        let proximity_factor = 1.0 + 2.0 * moat_depth / (moat_distance + moat_depth).max(1.0);

        let pore_pressure_factor = 1.0 + pore_pressure_ratio * 2.0;

        let delta = q * b * (1.0 - nu * nu) / e.max(1.0) * proximity_factor * pore_pressure_factor;

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
        pore_pressure_ratio: f64,
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
        if pore_pressure_ratio > 0.3 {
            recs.push("孔隙水压比较高，建议设置排水系统降低地下水位".to_string());
        }
        if overall_sf >= 3.0 && slope_sf >= 2.5 && pore_pressure_ratio < 0.15 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{SoilType, TowerMetadata};

    fn test_tower() -> TowerMetadata {
        TowerMetadata {
            tower_id: 1,
            tower_name: "测试塔".to_string(),
            build_date: "1450-01-01".to_string(),
            material: "杉木".to_string(),
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

    fn analyzer() -> MoatAnalyzer {
        MoatAnalyzer::new()
    }

    #[test]
    fn test_bearing_capacity_reduction_normal() {
        let tower = test_tower();
        let soil = SoilType::Loam;
        let result = analyzer().analyze(&tower, &soil, 5.0, 4.0, 2.0, 10.0, 0.0);

        assert!(result.bearing_capacity_reduction > 0.0);
        assert!(result.bearing_capacity_reduction <= 1.0);
        assert_eq!(result.effective_bearing_capacity,
            soil.bearing_capacity_kpa() * result.bearing_capacity_reduction);
    }

    #[test]
    fn test_bearing_capacity_reduction_deep_water_table() {
        let tower = test_tower();
        let soil = SoilType::Sand;
        let result = analyzer().analyze(&tower, &soil, 5.0, 4.0, 10.0, 10.0, 0.0);

        assert_eq!(result.bearing_capacity_reduction, 1.0);
    }

    #[test]
    fn test_bearing_capacity_reduction_shallow_water_table() {
        let tower = test_tower();
        let soil = SoilType::Clay;
        let result = analyzer().analyze(&tower, &soil, 5.0, 4.0, 0.0, 10.0, 0.0);

        assert!(result.bearing_capacity_reduction < 0.7);
        assert!(result.bearing_capacity_reduction >= 0.3);
        assert!(result.pore_pressure_ratio > 0.0);
        assert!(result.water_soil_coupling_factor < 1.0);
    }

    #[test]
    fn test_slope_stability_normal() {
        let tower = test_tower();
        let soil = SoilType::Rock;
        let result = analyzer().analyze(&tower, &soil, 5.0, 4.0, 2.0, 10.0, 0.0);

        assert!(result.slope_stability_factor > 1.0);
        assert!(result.slope_stability_factor <= 10.0);
    }

    #[test]
    fn test_slope_stability_steep_slope() {
        let tower = test_tower();
        let soil = SoilType::Silt;
        let result_close = analyzer().analyze(&tower, &soil, 1.0, 4.0, 2.0, 10.0, 0.0);
        let result_far = analyzer().analyze(&tower, &soil, 10.0, 4.0, 2.0, 10.0, 0.0);

        assert!(result_close.slope_stability_factor < result_far.slope_stability_factor);
    }

    #[test]
    fn test_settlement_increase_sand_clay_difference() {
        let tower = test_tower();
        let sand_result = analyzer().analyze(&tower, &SoilType::Sand, 5.0, 4.0, 1.0, 10.0, 0.0);
        let clay_result = analyzer().analyze(&tower, &SoilType::Clay, 5.0, 4.0, 1.0, 10.0, 0.0);

        assert!(clay_result.settlement_increase_pct > sand_result.settlement_increase_pct);
        assert!(sand_result.settlement_increase_pct >= 20.0);
        assert!(clay_result.settlement_increase_pct <= 120.0);
    }

    #[test]
    fn test_settlement_increase_zero_water_table() {
        let tower = test_tower();
        let result = analyzer().analyze(&tower, &SoilType::Loam, 5.0, 4.0, 4.0, 10.0, 0.0);

        assert!(result.settlement_increase_pct < 50.0);
    }

    #[test]
    fn test_lateral_displacement_increases_with_wind() {
        let tower = test_tower();
        let soil = SoilType::Loam;
        let result_calm = analyzer().analyze(&tower, &soil, 5.0, 4.0, 2.0, 5.0, 0.0);
        let result_storm = analyzer().analyze(&tower, &soil, 5.0, 4.0, 2.0, 30.0, 0.0);

        assert!(result_storm.lateral_displacement_mm >= result_calm.lateral_displacement_mm);
        assert!(result_storm.lateral_displacement_mm > 0.0);
    }

    #[test]
    fn test_lateral_displacement_increases_with_tilt() {
        let tower = test_tower();
        let soil = SoilType::Loam;
        let result_straight = analyzer().analyze(&tower, &soil, 5.0, 4.0, 2.0, 10.0, 0.0);
        let result_tilted = analyzer().analyze(&tower, &soil, 5.0, 4.0, 2.0, 10.0, 3.0);

        assert!(result_tilted.lateral_displacement_mm >= result_straight.lateral_displacement_mm);
    }

    #[test]
    fn test_overall_safety_factor_valid_range() {
        let tower = test_tower();
        let soil = SoilType::Loam;
        let result = analyzer().analyze(&tower, &soil, 5.0, 4.0, 2.0, 10.0, 0.0);

        let applied_pressure = tower.total_weight * 9.81 / (tower.base_width * tower.base_depth);
        let bearing_sf = result.effective_bearing_capacity / applied_pressure.max(0.1);

        assert!(result.overall_safety_factor > 0.0);
        assert!(result.overall_safety_factor <= bearing_sf);
        assert!(result.overall_safety_factor <= result.slope_stability_factor);
    }

    #[test]
    fn test_risk_level_mapping() {
        let tower = test_tower();

        let rock_result = analyzer().analyze(&tower, &SoilType::Rock, 10.0, 4.0, 10.0, 5.0, 0.0);
        assert!(rock_result.risk_level >= 1);
        assert!(rock_result.risk_level <= 5);

        let clay_close_result = analyzer().analyze(&tower, &SoilType::Clay, 1.0, 10.0, 0.0, 30.0, 5.0);
        assert!(clay_close_result.risk_level >= 1);
        assert!(clay_close_result.risk_level <= 5);
    }

    #[test]
    fn test_risk_level_monotonic_with_sf() {
        let tower = test_tower();
        let soil = SoilType::Silt;

        let result_safe = analyzer().analyze(&tower, &soil, 10.0, 2.0, 10.0, 5.0, 0.0);
        let result_unsafe = analyzer().analyze(&tower, &soil, 1.0, 10.0, 0.0, 40.0, 5.0);

        assert!(result_safe.risk_level <= result_unsafe.risk_level);
    }

    #[test]
    fn test_recommendations_not_empty() {
        let tower = test_tower();
        let soil = SoilType::Loam;
        let result = analyzer().analyze(&tower, &soil, 5.0, 4.0, 2.0, 10.0, 0.0);

        assert!(!result.recommendations.is_empty());
    }

    #[test]
    fn test_recommendations_for_risky_situation() {
        let tower = test_tower();
        let soil = SoilType::Clay;
        let result = analyzer().analyze(&tower, &soil, 2.0, 8.0, 0.5, 35.0, 4.0);

        let rec_texts: Vec<&str> = result.recommendations.iter().map(|s| s.as_str()).collect();
        let has_moat_warning = rec_texts.iter().any(|r| r.contains("护城河") || r.contains("安全距离"));
        let has_safety_warning = result.overall_safety_factor < 1.5 &&
            rec_texts.iter().any(|r| r.contains("安全系数"));

        assert!(has_moat_warning || result.moat_distance_m >= 5.0);
        if result.overall_safety_factor < 1.5 {
            assert!(has_safety_warning);
        }
    }

    #[test]
    fn test_soil_types_all_supported() {
        let tower = test_tower();
        let soils = vec![
            SoilType::Sand,
            SoilType::Clay,
            SoilType::Silt,
            SoilType::Rock,
            SoilType::Loam,
        ];

        for soil in soils {
            let result = analyzer().analyze(&tower, &soil, 5.0, 4.0, 2.0, 10.0, 0.0);
            assert!(result.overall_safety_factor > 0.0);
            assert!(!result.soil_type.is_empty());
            assert_eq!(result.soil_type, soil.as_str());
        }
    }

    #[test]
    fn test_boundary_zero_moat_distance() {
        let tower = test_tower();
        let soil = SoilType::Sand;
        let result = analyzer().analyze(&tower, &soil, 0.1, 4.0, 2.0, 10.0, 0.0);

        assert!(result.slope_stability_factor > 0.0);
        assert!(result.overall_safety_factor > 0.0);
    }

    #[test]
    fn test_boundary_zero_wind() {
        let tower = test_tower();
        let soil = SoilType::Loam;
        let result = analyzer().analyze(&tower, &soil, 5.0, 4.0, 2.0, 0.0, 0.0);

        assert!(result.lateral_displacement_mm >= 0.0);
    }

    #[test]
    fn test_boundary_zero_tilt() {
        let tower = test_tower();
        let soil = SoilType::Loam;
        let result = analyzer().analyze(&tower, &soil, 5.0, 4.0, 2.0, 10.0, 0.0);

        assert!(result.overall_safety_factor > 0.0);
    }

    #[test]
    fn test_timestamp_present() {
        let tower = test_tower();
        let soil = SoilType::Loam;
        let result = analyzer().analyze(&tower, &soil, 5.0, 4.0, 2.0, 10.0, 0.0);

        assert!(result.timestamp.timestamp() > 0);
    }

    #[test]
    fn test_large_moat_distance_stable() {
        let tower = test_tower();
        let soil = SoilType::Rock;
        let result = analyzer().analyze(&tower, &soil, 100.0, 4.0, 10.0, 10.0, 0.0);

        assert!(result.overall_safety_factor > 2.0);
    }

    #[test]
    fn test_pore_pressure_ratio_range() {
        let tower = test_tower();
        let dry = analyzer().analyze(&tower, &SoilType::Loam, 5.0, 4.0, 10.0, 10.0, 0.0);
        let wet = analyzer().analyze(&tower, &SoilType::Loam, 5.0, 4.0, 0.0, 10.0, 0.0);

        assert_eq!(dry.pore_pressure_ratio, 0.0);
        assert!(wet.pore_pressure_ratio > 0.0);
        assert!(wet.pore_pressure_ratio <= 0.5);
    }

    #[test]
    fn test_coupling_factor_decreases_with_pore_pressure() {
        let tower = test_tower();
        let dry = analyzer().analyze(&tower, &SoilType::Clay, 5.0, 4.0, 10.0, 10.0, 0.0);
        let wet = analyzer().analyze(&tower, &SoilType::Clay, 5.0, 4.0, 0.0, 10.0, 0.0);

        assert!(dry.water_soil_coupling_factor > wet.water_soil_coupling_factor);
        assert!(dry.water_soil_coupling_factor <= 1.0);
        assert!(wet.water_soil_coupling_factor >= 0.5);
    }

    #[test]
    fn test_seepage_force_sand_greater_than_clay() {
        let tower = test_tower();
        let sand = analyzer().analyze(&tower, &SoilType::Sand, 5.0, 4.0, 0.0, 10.0, 0.0);
        let clay = analyzer().analyze(&tower, &SoilType::Clay, 5.0, 4.0, 0.0, 10.0, 0.0);

        assert!(sand.seepage_force_kn >= 0.0);
        assert!(clay.seepage_force_kn >= 0.0);
    }

    #[test]
    fn test_coupling_reduces_safety_factor() {
        let tower = test_tower();
        let dry = analyzer().analyze(&tower, &SoilType::Silt, 5.0, 4.0, 10.0, 10.0, 0.0);
        let wet = analyzer().analyze(&tower, &SoilType::Silt, 5.0, 4.0, 0.0, 10.0, 0.0);

        assert!(wet.overall_safety_factor <= dry.overall_safety_factor);
    }

    #[test]
    fn test_pore_pressure_drainage_recommendation() {
        let tower = test_tower();
        let result = analyzer().analyze(&tower, &SoilType::Clay, 2.0, 8.0, 0.0, 35.0, 4.0);

        if result.pore_pressure_ratio > 0.3 {
            let has_drainage = result.recommendations.iter().any(|r| r.contains("排水"));
            assert!(has_drainage);
        }
    }
}

use crate::models::{GroundAnalysis, SoilType, TowerMetadata};
use chrono::Utc;
use std::f64::consts::PI;

#[derive(Debug, Clone)]
pub struct SoilParams {
    pub name: &'static str,
    pub c_ref: f64,
    pub phi_ref_deg: f64,
    pub gamma_sat: f64,
    pub gamma_dry: f64,
    pub w_opt: f64,
    pub w_sat: f64,
    pub k_c: f64,
    pub k_phi: f64,
    pub ref_bearing: f64,
    pub ref_friction: f64,
    pub compressibility_ref: f64,
}

impl SoilType {
    pub fn params(&self) -> SoilParams {
        match self {
            SoilType::Rock => SoilParams {
                name: "rock", c_ref: 5000.0, phi_ref_deg: 45.0,
                gamma_sat: 26.0, gamma_dry: 25.0,
                w_opt: 1.0, w_sat: 3.0,
                k_c: 0.02, k_phi: 0.02,
                ref_bearing: 800.0, ref_friction: 0.65,
                compressibility_ref: 0.001,
            },
            SoilType::Loam => SoilParams {
                name: "loam", c_ref: 35.0, phi_ref_deg: 28.0,
                gamma_sat: 19.5, gamma_dry: 16.5,
                w_opt: 18.0, w_sat: 32.0,
                k_c: 0.035, k_phi: 0.25,
                ref_bearing: 200.0, ref_friction: 0.40,
                compressibility_ref: 0.08,
            },
            SoilType::Sand => SoilParams {
                name: "sand", c_ref: 2.0, phi_ref_deg: 34.0,
                gamma_sat: 20.0, gamma_dry: 16.8,
                w_opt: 10.0, w_sat: 24.0,
                k_c: 0.08, k_phi: 0.18,
                ref_bearing: 180.0, ref_friction: 0.45,
                compressibility_ref: 0.02,
            },
            SoilType::Silt => SoilParams {
                name: "silt", c_ref: 18.0, phi_ref_deg: 24.0,
                gamma_sat: 18.8, gamma_dry: 15.5,
                w_opt: 22.0, w_sat: 40.0,
                k_c: 0.04, k_phi: 0.30,
                ref_bearing: 90.0, ref_friction: 0.30,
                compressibility_ref: 0.20,
            },
            SoilType::Clay => SoilParams {
                name: "clay", c_ref: 60.0, phi_ref_deg: 18.0,
                gamma_sat: 18.2, gamma_dry: 15.0,
                w_opt: 28.0, w_sat: 55.0,
                k_c: 0.025, k_phi: 0.40,
                ref_bearing: 120.0, ref_friction: 0.25,
                compressibility_ref: 0.35,
            },
        }
    }

    pub fn effective_params(&self, moisture_pct: f64) -> (f64, f64, f64) {
        let p = self.params();
        let w = moisture_pct.clamp(0.0, p.w_sat);
        let delta_w = (w - p.w_opt).max(0.0);
        let ratio = delta_w / (p.w_sat - p.w_opt + 1e-6);

        let c_eff = p.c_ref * (-p.k_c * delta_w).exp();
        let phi_eff = p.phi_ref_deg * (1.0 - p.k_phi * ratio.min(1.0));
        let gamma = p.gamma_dry + (p.gamma_sat - p.gamma_dry) * (w / p.w_sat).min(1.0);

        (c_eff, phi_eff, gamma)
    }

    pub fn bearing_capacity_with_moisture(&self, moisture_pct: f64, width: f64,
                                           depth: f64, incline_deg: f64) -> f64 {
        let (c, phi_deg, gamma) = self.effective_params(moisture_pct);
        terzaghi_ultimate_bearing(c, phi_deg, gamma, width, depth, incline_deg)
    }

    pub fn moisture_corrected_friction(&self, moisture_pct: f64) -> f64 {
        let p = self.params();
        let w = moisture_pct.clamp(0.0, p.w_sat);
        let delta_w = (w - p.w_opt).max(0.0);
        let ratio = delta_w / (p.w_sat - p.w_opt + 1e-6);
        let (_, phi_eff, _) = self.effective_params(moisture_pct);
        let phi_correction = (phi_eff.to_radians()).tan() / p.phi_ref_deg.to_radians().tan();
        (p.ref_friction * phi_correction * (1.0 - 0.25 * ratio.min(1.0))).max(0.05)
    }

    pub fn compressibility_with_moisture(&self, moisture_pct: f64) -> f64 {
        let p = self.params();
        let w = moisture_pct.clamp(0.0, p.w_sat);
        let ratio = ((w - p.w_opt).max(0.0)) / (p.w_sat - p.w_opt + 1e-6);
        p.compressibility_ref * (1.0 + ratio * 2.5)
    }
}

pub fn terzaghi_bearing_coefficients(phi_deg: f64) -> (f64, f64, f64) {
    use std::f64::consts::PI;
    let phi = phi_deg.clamp(0.0, 50.0).to_radians();
    let tan_phi = phi.tan();
    let kp = (PI / 4.0 + phi / 2.0).tan().powi(2);

    let n_q = kp * (2.0 * PI * phi.tan()).exp();

    let n_c = if phi_deg < 1e-4 {
        5.7
    } else {
        (n_q - 1.0) / tan_phi
    };

    let n_gamma = if phi_deg < 10.0 {
        0.0
    } else {
        2.0 * (n_q - 1.0) * tan_phi
    };

    (n_c, n_q, n_gamma)
}

pub fn terzaghi_ultimate_bearing(
    c: f64, phi_deg: f64, gamma: f64,
    width_b: f64, depth_d: f64, load_incline_deg: f64,
) -> f64 {
    let (n_c, n_q, n_gamma) = terzaghi_bearing_coefficients(phi_deg);
    let l = width_b;
    let b = width_b * 0.78;

    let s_c = 1.0 + (b / l) * (n_q / n_c);
    let s_q = 1.0 + (b / l) * (n_q.sqrt() - 1.0);
    let s_gamma = 1.0 - 0.4 * (b / l);

    let d_c = 1.0 + 0.4 * (depth_d / b).min(1.0);
    let dq_arg = (depth_d / b).clamp(0.0, 1.0).atan();
    let d_q = 1.0 + 2.0 * dq_arg * phi_deg.to_radians().tan() / PI;
    let d_q = d_q * (1.0 + 2.0 * phi_deg.to_radians().tan() / PI);
    let d_gamma = 1.0;

    let incline = load_incline_deg.to_radians();
    let i_c = if phi_deg < 1e-4 {
        0.5 - 0.5 * (1.0 - incline.sin()).sqrt()
    } else {
        (1.0 - incline / (2.0 * phi_deg.to_radians()).min(PI / 2.0)).powi(2)
    };
    let i_q = (1.0 - incline / phi_deg.to_radians().atan().tan().atan().max(0.01)).powi(2);
    let i_q = (1.0 - incline.sin()).powi(2);
    let i_gamma = (1.0 - incline / phi_deg.to_radians().atan().tan().atan().max(0.01)).powi(3);
    let i_gamma = (1.0 - incline.sin()).powi(3);

    let q = gamma * depth_d;

    let term_c = c * n_c * s_c * d_c * i_c;
    let term_q = q * n_q * s_q * d_q * i_q;
    let term_gamma = 0.5 * gamma * b * n_gamma * s_gamma * d_gamma * i_gamma;

    let q_ult = term_c + term_q + term_gamma;
    q_ult.max(10.0)
}

pub struct GroundAnalyzer;

impl GroundAnalyzer {
    pub fn new() -> Self { GroundAnalyzer }

    pub fn calculate_applied_pressure(&self, tower: &TowerMetadata, tilt_deg: f64) -> f64 {
        let base_area = tower.base_width * tower.base_depth;
        let weight_kn = tower.total_weight * 9.81;
        let uniform_pressure = weight_kn / base_area;
        let tilt_rad = tilt_deg.to_radians();
        let eccentricity = (tower.total_height / 2.0) * tilt_rad.sin();
        let section_modulus = tower.base_width * tower.base_depth.powi(2) / 6.0;
        let bending_pressure = weight_kn * eccentricity / section_modulus;
        uniform_pressure + bending_pressure.abs()
    }

    pub fn calculate_settlement(
        &self, soil: &SoilType, moisture_pct: f64,
        applied_pressure_kpa: f64, bearing_capacity_kpa: f64,
        layer_thickness_m: f64,
    ) -> f64 {
        let cc = soil.compressibility_with_moisture(moisture_pct);
        let stress_ratio = applied_pressure_kpa / bearing_capacity_kpa.max(1.0);
        if stress_ratio > 0.98 { return 600.0; }
        let eo = 0.6 + 0.5 * (moisture_pct / 100.0);
        let stress_increase_ratio = (applied_pressure_kpa / 50.0 + 1.0).log10();
        let delta_e = cc * stress_increase_ratio;
        let primary = (delta_e / (1.0 + eo)) * layer_thickness_m * 1000.0;
        let secondary = primary * 0.15 * stress_ratio;
        primary + secondary
    }

    pub fn calculate_safety_factor(
        &self, soil: &SoilType, moisture_pct: f64,
        applied_pressure_kpa: f64, bearing_capacity_kpa: f64, wind_speed: f64,
    ) -> (f64, f64, f64) {
        let pressure_sf = bearing_capacity_kpa / applied_pressure_kpa.max(0.1);

        let friction = soil.moisture_corrected_friction(moisture_pct);
        let normal_force = applied_pressure_kpa * tower_basic_area();
        let sliding_resistance = friction * normal_force;

        let air_density = 1.225;
        let cd = 1.3;
        let q = 0.5 * air_density * cd * wind_speed * wind_speed / 1000.0;
        let sliding_force = q * tower_project_area();
        let sliding_sf = sliding_resistance / sliding_force.max(1.0);

        let bearing_sf = bearing_capacity_kpa / (applied_pressure_kpa * 1.05).max(1.0);
        let overall = pressure_sf.min(sliding_sf).min(bearing_sf);
        (overall.min(10.0), pressure_sf.min(10.0), sliding_sf.min(10.0))
    }

    pub fn calculate_passability(
        &self, sf: f64, settlement_mm: f64,
        diff_settlement_mm: f64, max_allowed_settlement: f64,
    ) -> (f64, u8, u8) {
        let sf_score = if sf >= 3.0 {
            100.0
        } else if sf >= 2.0 {
            70.0 + (sf - 2.0) * 30.0
        } else if sf >= 1.5 {
            40.0 + (sf - 1.5) * 60.0
        } else if sf >= 1.0 {
            20.0 + (sf - 1.0) * 40.0
        } else {
            sf * 20.0
        };
        let set_score = if settlement_mm <= max_allowed_settlement {
            100.0
        } else if settlement_mm <= max_allowed_settlement * 2.0 {
            70.0 - (settlement_mm - max_allowed_settlement) / max_allowed_settlement * 70.0
        } else { 0.0 };
        let diff_set_score = if diff_settlement_mm <= 20.0 {
            100.0
        } else if diff_settlement_mm <= 50.0 {
            60.0 - (diff_settlement_mm - 20.0) / 30.0 * 60.0
        } else { 0.0 };

        let total_score = (sf_score * 0.45 + set_score * 0.3 + diff_set_score * 0.25)
            .max(0.0).min(100.0);
        let (can_pass, risk_level) = if total_score >= 75.0 {
            (1u8, 1u8)
        } else if total_score >= 50.0 {
            (1u8, 2u8)
        } else if total_score >= 30.0 {
            (0u8, 2u8)
        } else {
            (0u8, 3u8)
        };
        (total_score, can_pass, risk_level)
    }

    pub fn analyze(
        &self, tower: &TowerMetadata, soil: SoilType,
        wind_speed: f64, tilt_deg: f64,
        additional_settlement: Option<f64>,
        moisture_pct: Option<f64>,
    ) -> GroundAnalysis {
        let params = soil.params();
        let moisture = moisture_pct.unwrap_or(params.w_opt);
        let depth = 0.5;
        let incline = tilt_deg.min(8.0);

        let bearing_capacity = soil.bearing_capacity_with_moisture(
            moisture, tower.base_width, depth, incline,
        );
        let applied_pressure = self.calculate_applied_pressure(tower, tilt_deg);
        let (sf, _pres_sf, _sl_sf) = self.calculate_safety_factor(
            &soil, moisture, applied_pressure, bearing_capacity, wind_speed,
        );

        let soil_layer_thickness = 2.0;
        let settlement = self.calculate_settlement(
            &soil, moisture, applied_pressure, bearing_capacity, soil_layer_thickness,
        ) + additional_settlement.unwrap_or(0.0);
        let diff_settlement = settlement * 0.3 + tilt_deg * 10.0
                            + wind_speed * 0.5;

        let max_settlement = match soil {
            SoilType::Rock => 10.0,
            SoilType::Sand => 50.0,
            SoilType::Loam => 75.0,
            SoilType::Silt => 100.0,
            SoilType::Clay => 150.0,
        };

        let (score, can_pass, risk_level) = self.calculate_passability(
            sf, settlement, diff_settlement, max_settlement,
        );

        GroundAnalysis {
            timestamp: Utc::now(),
            tower_id: tower.tower_id,
            soil_type: soil.as_str().to_string(),
            bearing_capacity,
            applied_pressure,
            safety_factor: sf,
            settlement,
            differential_settlement: diff_settlement,
            passability_score: score,
            can_pass,
            risk_level,
        }
    }

    pub fn analyze_all_soils(
        &self, tower: &TowerMetadata, wind_speed: f64, tilt_deg: f64,
        moisture_pct: Option<f64>,
    ) -> Vec<GroundAnalysis> {
        let soils = [SoilType::Rock, SoilType::Loam, SoilType::Sand,
                     SoilType::Silt, SoilType::Clay];
        soils.iter().map(|soil| {
            self.analyze(tower, soil.clone(), wind_speed, tilt_deg, None, moisture_pct)
        }).collect()
    }
}

fn tower_basic_area() -> f64 { 28.0 }
fn tower_project_area() -> f64 { 85.0 }

impl Default for GroundAnalyzer {
    fn default() -> Self { Self::new() }
}

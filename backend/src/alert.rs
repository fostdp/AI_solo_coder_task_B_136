use crate::config::AlertThresholds;
use crate::models::{AlertEvent, AlertType, SensorData, StructureAnalysis, TowerMetadata};
use std::collections::HashMap;

pub struct AlertManager {
    last_alerts: HashMap<String, chrono::DateTime<chrono::Utc>>,
    cooldown_seconds: i64,
}

impl AlertManager {
    pub fn new() -> Self {
        AlertManager {
            last_alerts: HashMap::new(),
            cooldown_seconds: 300,
        }
    }

    fn should_alert(&mut self, key: &str) -> bool {
        let now = chrono::Utc::now();
        if let Some(last) = self.last_alerts.get(key) {
            if (now - *last).num_seconds() < self.cooldown_seconds {
                return false;
            }
        }
        self.last_alerts.insert(key.to_string(), now);
        true
    }

    pub fn check_sensor_alerts(
        &mut self,
        tower: &TowerMetadata,
        data: &[SensorData],
        config: &AlertThresholds,
    ) -> Vec<AlertEvent> {
        let mut alerts = Vec::new();
        let tower_name = tower.tower_name.clone();
        let tower_id = tower.tower_id;

        let vib_bandwidth = config.vibration_resonance_bandwidth_pct / 100.0;
        let vib_danger_amp = config.vibration_danger_amplitude;

        for sd in data {
            if sd.tilt_total >= config.tilt_danger_deg {
                let key = format!("tilt_danger_{}_{}", tower_id, sd.layer_id);
                if self.should_alert(&key) {
                    alerts.push(AlertEvent::new(
                        tower_id, tower_name.clone(),
                        AlertType::TiltExceed, 3,
                        sd.layer_id, "tilt_total".to_string(),
                        sd.tilt_total, config.tilt_danger_deg,
                    ));
                }
            } else if sd.tilt_total >= config.tilt_warning_deg {
                let key = format!("tilt_warn_{}_{}", tower_id, sd.layer_id);
                if self.should_alert(&key) {
                    alerts.push(AlertEvent::new(
                        tower_id, tower_name.clone(),
                        AlertType::TiltExceed, 1,
                        sd.layer_id, "tilt_total".to_string(),
                        sd.tilt_total, config.tilt_warning_deg,
                    ));
                }
            }

            let stress_ratio = sd.stress_von_mises / tower.material_strength;
            if stress_ratio >= config.stress_danger_ratio {
                let key = format!("stress_danger_{}_{}", tower_id, sd.layer_id);
                if self.should_alert(&key) {
                    alerts.push(AlertEvent::new(
                        tower_id, tower_name.clone(),
                        AlertType::StressCritical, 3,
                        sd.layer_id, "stress_von_mises".to_string(),
                        sd.stress_von_mises,
                        tower.material_strength * config.stress_danger_ratio,
                    ));
                }
            } else if stress_ratio >= config.stress_warning_ratio {
                let key = format!("stress_warn_{}_{}", tower_id, sd.layer_id);
                if self.should_alert(&key) {
                    alerts.push(AlertEvent::new(
                        tower_id, tower_name.clone(),
                        AlertType::StressCritical, 1,
                        sd.layer_id, "stress_von_mises".to_string(),
                        sd.stress_von_mises,
                        tower.material_strength * config.stress_warning_ratio,
                    ));
                }
            }

            let wind_ratio = sd.wind_speed_mps / tower.design_wind_speed;
            if wind_ratio >= config.wind_danger_ratio {
                let key = format!("wind_danger_{}", tower_id);
                if self.should_alert(&key) {
                    alerts.push(AlertEvent::new(
                        tower_id, tower_name.clone(),
                        AlertType::WindOverload, 2,
                        sd.layer_id, "wind_speed_mps".to_string(),
                        sd.wind_speed_mps,
                        tower.design_wind_speed * config.wind_danger_ratio,
                    ));
                }
            } else if wind_ratio >= config.wind_warning_ratio {
                let key = format!("wind_warn_{}", tower_id);
                if self.should_alert(&key) {
                    alerts.push(AlertEvent::new(
                        tower_id, tower_name.clone(),
                        AlertType::WindOverload, 1,
                        sd.layer_id, "wind_speed_mps".to_string(),
                        sd.wind_speed_mps,
                        tower.design_wind_speed * config.wind_warning_ratio,
                    ));
                }
            }

            let soil_cap = match sd.soil_type.as_str() {
                "sand" => 180.0,
                "clay" => 120.0,
                "silt" => 90.0,
                "rock" => 800.0,
                _ => 200.0,
            };
            let ground_ratio = sd.ground_pressure / soil_cap;
            if ground_ratio >= config.ground_danger_ratio {
                let key = format!("ground_danger_{}", tower_id);
                if self.should_alert(&key) {
                    alerts.push(AlertEvent::new(
                        tower_id, tower_name.clone(),
                        AlertType::GroundFailure, 3,
                        1, "ground_pressure".to_string(),
                        sd.ground_pressure,
                        soil_cap * config.ground_danger_ratio,
                    ));
                }
            } else if ground_ratio >= config.ground_warning_ratio {
                let key = format!("ground_warn_{}", tower_id);
                if self.should_alert(&key) {
                    alerts.push(AlertEvent::new(
                        tower_id, tower_name.clone(),
                        AlertType::GroundFailure, 1,
                        1, "ground_pressure".to_string(),
                        sd.ground_pressure,
                        soil_cap * config.ground_warning_ratio,
                    ));
                }
            }

            let natural_freq = (tower.elastic_modulus * tower.base_width * tower.base_depth.powi(3)
                / 12.0 / (tower.total_weight * 1000.0) / tower.total_height.powi(3)).sqrt()
                * 1.875_f64.powi(2) / (2.0 * std::f64::consts::PI);
            let freq_ratio = (sd.vibration_freq - natural_freq).abs() / natural_freq;
            if freq_ratio < vib_bandwidth && sd.vibration_amp > vib_danger_amp {
                let key = format!("vib_exceed_{}", tower_id);
                if self.should_alert(&key) {
                    alerts.push(AlertEvent::new(
                        tower_id, tower_name.clone(),
                        AlertType::VibrationExceed, 2,
                        sd.layer_id, "vibration_freq".to_string(),
                        sd.vibration_freq, natural_freq * (1.0 + vib_bandwidth),
                    ));
                }
            }
        }

        alerts
    }

    pub fn check_structure_alerts(
        &mut self,
        analysis: &StructureAnalysis,
        config: &AlertThresholds,
    ) -> Vec<AlertEvent> {
        let mut alerts = Vec::new();
        let tower_id = analysis.tower_id;
        let tower_name = analysis.tower_name.clone();
        let min_sf = 1.5;

        if analysis.is_stable == 0 {
            let key = format!("instability_{}", tower_id);
            if self.should_alert(&key) {
                alerts.push(AlertEvent::new(
                    tower_id, tower_name.clone(),
                    AlertType::StructureInstability, 3,
                    analysis.max_stress_layer, "safety_factor".to_string(),
                    analysis.safety_factor, min_sf,
                ));
            }
        }

        if analysis.current_wind_factor >= config.wind_danger_ratio {
            let key = format!("wind_limit_{}", tower_id);
            if self.should_alert(&key) {
                alerts.push(AlertEvent::new(
                    tower_id, tower_name.clone(),
                    AlertType::WindOverload, 3,
                    analysis.max_tilt_layer, "current_wind_factor".to_string(),
                    analysis.current_wind_factor, config.wind_danger_ratio,
                ));
            }
        }

        if analysis.second_order_effect >= 1.5 {
            let key = format!("second_order_{}", tower_id);
            if self.should_alert(&key) {
                alerts.push(AlertEvent::new(
                    tower_id, tower_name.clone(),
                    AlertType::StructureInstability, 2,
                    analysis.max_stress_layer, "second_order_effect".to_string(),
                    analysis.second_order_effect, 1.5,
                ));
            }
        }

        alerts
    }
}

impl Default for AlertManager {
    fn default() -> Self {
        Self::new()
    }
}

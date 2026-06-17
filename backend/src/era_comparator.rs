use crate::database::get_default_tower;
use crate::models::{CrossEraComparison, EraComparisonData, CrossEraRatios, TowerMetadata};
use crate::stability::StabilityAnalyzer;

pub struct EraComparator {
    analyzer: StabilityAnalyzer,
    air_density: f64,
    drag_coefficient: f64,
    safety_factor_min: f64,
}

impl EraComparator {
    pub fn new() -> Self {
        Self {
            analyzer: StabilityAnalyzer::new(),
            air_density: 1.225,
            drag_coefficient: 1.3,
            safety_factor_min: 1.5,
        }
    }

    fn evaluate_era_metrics(
        &self,
        tower: &TowerMetadata,
        era_label: &str,
        tilt_deg: f64,
    ) -> EraComparisonData {
        let max_stress = tower.material_strength * 0.6;
        let second_order = self.analyzer.calculate_second_order_effect(
            tower, tower.total_weight, tilt_deg.to_radians() * tower.total_height * 0.5,
        );
        let safety_factor = self.analyzer.calculate_safety_factor(
            tower, max_stress, tower.material_strength, second_order,
        );
        let wind_resistance = self.analyzer.calculate_wind_resistance_limit(
            tower, self.safety_factor_min, self.air_density, self.drag_coefficient,
        );
        let natural_frequency = self.analyzer.calculate_natural_frequency(tower);

        EraComparisonData {
            tower_id: tower.tower_id,
            tower_name: tower.tower_name.clone(),
            era: era_label.to_string(),
            material: tower.material.clone(),
            elastic_modulus: tower.elastic_modulus,
            material_strength: tower.material_strength,
            safety_factor,
            wind_resistance,
            natural_frequency,
            weight_per_height: tower.total_weight / tower.total_height,
            load_efficiency: tower.design_load / (tower.total_weight * 9.81),
        }
    }

    pub fn compare_cross_era(&self, tilt_deg: f64) -> CrossEraComparison {
        let ancient_tower = get_default_tower(1);
        let modern_tower = get_default_tower(5);

        let ancient_data = self.evaluate_era_metrics(&ancient_tower, "明朝", tilt_deg);
        let modern_data = self.evaluate_era_metrics(&modern_tower, "现代", tilt_deg);

        let ancient_le = ancient_data.load_efficiency;
        let modern_le = modern_data.load_efficiency;

        let ratios = CrossEraRatios {
            elastic_modulus_ratio: modern_data.elastic_modulus / ancient_data.elastic_modulus,
            strength_ratio: modern_data.material_strength / ancient_data.material_strength,
            safety_factor_ratio: modern_data.safety_factor / ancient_data.safety_factor.max(0.01),
            wind_resistance_ratio: modern_data.wind_resistance / ancient_data.wind_resistance.max(0.01),
            frequency_ratio: modern_data.natural_frequency / ancient_data.natural_frequency.max(0.01),
            weight_efficiency_ratio: modern_le / ancient_le.max(0.01),
        };

        let analysis = format!(
            "现代Q345B钢材弹性模量为古代松木的{:.1}倍，材料强度为{:.1}倍，风阻能力为{:.1}倍。\
             钢结构在力学性能上全面超越木结构，但古代木构设计蕴含丰富的工程智慧。",
            ratios.elastic_modulus_ratio, ratios.strength_ratio, ratios.wind_resistance_ratio,
        );

        CrossEraComparison {
            ancient: ancient_data,
            modern: modern_data,
            ratios,
            analysis,
        }
    }
}

impl Default for EraComparator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_era_ancient_uses_tower_1() {
        let cmp = EraComparator::new();
        let result = cmp.compare_cross_era(0.5);
        assert_eq!(result.ancient.tower_id, 1);
        assert_eq!(result.ancient.era, "明朝");
    }

    #[test]
    fn test_cross_era_modern_uses_tower_5() {
        let cmp = EraComparator::new();
        let result = cmp.compare_cross_era(0.5);
        assert_eq!(result.modern.tower_id, 5);
        assert_eq!(result.modern.era, "现代");
    }

    #[test]
    fn test_cross_era_ratios_steel_beats_wood() {
        let cmp = EraComparator::new();
        let result = cmp.compare_cross_era(0.5);
        assert!(result.ratios.elastic_modulus_ratio > 15.0);
        assert!(result.ratios.strength_ratio > 5.0);
        assert!(result.ratios.wind_resistance_ratio > 1.0);
    }

    #[test]
    fn test_cross_era_positive_metrics() {
        let cmp = EraComparator::new();
        let result = cmp.compare_cross_era(0.5);
        assert!(result.ancient.safety_factor > 0.0);
        assert!(result.modern.safety_factor > 0.0);
        assert!(result.ancient.wind_resistance > 0.0);
        assert!(result.modern.wind_resistance > 0.0);
        assert!(result.ancient.load_efficiency > 0.0);
        assert!(result.modern.load_efficiency > 0.0);
    }

    #[test]
    fn test_cross_era_analysis_not_empty() {
        let cmp = EraComparator::new();
        let result = cmp.compare_cross_era(0.5);
        assert!(!result.analysis.is_empty());
        assert!(result.analysis.len() > 10);
    }
}

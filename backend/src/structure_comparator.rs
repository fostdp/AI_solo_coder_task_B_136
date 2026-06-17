use crate::database::get_default_tower;
use crate::models::{
    TowerMetadata, TowerCategory, DynastyComparison, TowerComparisonItem, ComparisonMetrics,
};
use crate::stability::StabilityAnalyzer;

pub struct StructureComparator {
    analyzer: StabilityAnalyzer,
    air_density: f64,
    drag_coefficient: f64,
    safety_factor_min: f64,
}

impl StructureComparator {
    pub fn new() -> Self {
        Self {
            analyzer: StabilityAnalyzer::new(),
            air_density: 1.225,
            drag_coefficient: 1.3,
            safety_factor_min: 1.5,
        }
    }

    pub fn get_dynasty_for_tower(tower_id: u32) -> String {
        match tower_id {
            1 | 2 => "明朝(景泰)".to_string(),
            3 => "明朝(洪武)".to_string(),
            4 => "三国(魏)".to_string(),
            5 => "现代".to_string(),
            _ => "未知".to_string(),
        }
    }

    pub fn get_category_for_tower(tower_id: u32) -> TowerCategory {
        match tower_id {
            5 => TowerCategory::ModernSteel,
            _ => TowerCategory::AncientWooden,
        }
    }

    pub fn analyze_single(&self, tower: &TowerMetadata, wind_speed: f64, tilt_deg: f64) -> TowerComparisonItem {
        let max_stress = tower.material_strength * 0.6;
        let second_order_coef = self.analyzer.calculate_second_order_effect(
            tower, tower.total_weight, tilt_deg.to_radians() * tower.total_height * 0.5,
        );
        let safety_factor = self.analyzer.calculate_safety_factor(
            tower, max_stress, tower.material_strength, second_order_coef,
        );
        let wind_resistance_limit = self.analyzer.calculate_wind_resistance_limit(
            tower, self.safety_factor_min, self.air_density, self.drag_coefficient,
        );
        let natural_frequency = self.analyzer.calculate_natural_frequency(tower);
        let overturning_ratio = self.analyzer.calculate_overturning_ratio(
            tower, wind_speed, tilt_deg, self.air_density, self.drag_coefficient,
        );
        let weight_efficiency = tower.design_load / (tower.total_weight * 9.81);
        let height_to_base_ratio = tower.total_height / tower.base_width;

        TowerComparisonItem {
            tower_id: tower.tower_id,
            tower_name: tower.tower_name.clone(),
            dynasty: Self::get_dynasty_for_tower(tower.tower_id),
            category: Self::get_category_for_tower(tower.tower_id),
            safety_factor,
            wind_resistance_limit,
            natural_frequency,
            overturning_ratio,
            weight_efficiency,
            height_to_base_ratio,
        }
    }

    pub fn compare_ancient_dynasties(&self, wind_speed: f64, tilt_deg: f64) -> DynastyComparison {
        let ids: [u32; 4] = [1, 2, 3, 4];
        let mut items: Vec<TowerComparisonItem> = ids
            .iter()
            .map(|&tid| {
                let tower = get_default_tower(tid);
                self.analyze_single(&tower, wind_speed, tilt_deg)
            })
            .collect();

        let best_sf = items.iter()
            .max_by(|a, b| a.safety_factor.partial_cmp(&b.safety_factor).unwrap())
            .map(|i| (i.tower_id, i.safety_factor))
            .unwrap_or((1, 0.0));
        let best_wind = items.iter()
            .max_by(|a, b| a.wind_resistance_limit.partial_cmp(&b.wind_resistance_limit).unwrap())
            .map(|i| (i.tower_id, i.wind_resistance_limit))
            .unwrap_or((1, 0.0));
        let best_freq = items.iter()
            .max_by(|a, b| a.natural_frequency.partial_cmp(&b.natural_frequency).unwrap())
            .map(|i| (i.tower_id, i.natural_frequency))
            .unwrap_or((1, 0.0));
        let best_ot = items.iter()
            .max_by(|a, b| a.overturning_ratio.partial_cmp(&b.overturning_ratio).unwrap())
            .map(|i| (i.tower_id, i.overturning_ratio))
            .unwrap_or((1, 0.0));
        let best_we = items.iter()
            .max_by(|a, b| a.weight_efficiency.partial_cmp(&b.weight_efficiency).unwrap())
            .map(|i| (i.tower_id, i.weight_efficiency))
            .unwrap_or((1, 0.0));

        DynastyComparison {
            towers: items,
            metrics: ComparisonMetrics {
                best_safety_factor: best_sf,
                best_wind_resistance: best_wind,
                best_frequency: best_freq,
                best_overturning: best_ot,
                best_weight_efficiency: best_we,
            },
        }
    }
}

impl Default for StructureComparator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_comparator() -> StructureComparator {
        StructureComparator::new()
    }

    #[test]
    fn test_dynasty_mapping_correct() {
        assert_eq!(StructureComparator::get_dynasty_for_tower(1), "明朝(景泰)");
        assert_eq!(StructureComparator::get_dynasty_for_tower(3), "明朝(洪武)");
        assert_eq!(StructureComparator::get_dynasty_for_tower(4), "三国(魏)");
        assert_eq!(StructureComparator::get_dynasty_for_tower(5), "现代");
    }

    #[test]
    fn test_category_mapping_correct() {
        assert_eq!(StructureComparator::get_category_for_tower(1), TowerCategory::AncientWooden);
        assert_eq!(StructureComparator::get_category_for_tower(5), TowerCategory::ModernSteel);
    }

    #[test]
    fn test_analyze_single_returns_valid_metrics() {
        let cmp = make_comparator();
        let tower = get_default_tower(1);
        let item = cmp.analyze_single(&tower, 15.0, 0.5);

        assert_eq!(item.tower_id, 1);
        assert!(item.safety_factor > 0.0);
        assert!(item.wind_resistance_limit > 0.0);
        assert!(item.natural_frequency > 0.0);
        assert!(item.overturning_ratio > 0.0);
        assert!(item.weight_efficiency > 0.0);
        assert!(item.height_to_base_ratio > 1.0);
    }

    #[test]
    fn test_compare_dynasties_returns_4_towers() {
        let cmp = make_comparator();
        let result = cmp.compare_ancient_dynasties(15.0, 0.5);

        assert_eq!(result.towers.len(), 4);
        assert!(result.metrics.best_safety_factor.0 >= 1);
        assert!(result.metrics.best_safety_factor.1 > 0.0);
        assert!(result.metrics.best_wind_resistance.1 > 0.0);
        assert!(result.metrics.best_frequency.1 > 0.0);
        assert!(result.metrics.best_overturning.1 > 0.0);
        assert!(result.metrics.best_weight_efficiency.1 > 0.0);
    }

    #[test]
    fn test_best_sf_matches_actual_max() {
        let cmp = make_comparator();
        let result = cmp.compare_ancient_dynasties(15.0, 0.5);

        let actual_max_sf = result.towers.iter()
            .map(|t| t.safety_factor)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((actual_max_sf - result.metrics.best_safety_factor.1).abs() < 1e-9);

        let max_wind = result.towers.iter()
            .map(|t| t.wind_resistance_limit)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((max_wind - result.metrics.best_wind_resistance.1).abs() < 1e-9);
    }
}

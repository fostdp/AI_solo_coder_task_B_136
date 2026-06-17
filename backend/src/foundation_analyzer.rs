use crate::database::get_default_tower;
use crate::models::{MoatAnalysis, SoilType, TowerMetadata};
use crate::moat_analyzer::MoatAnalyzer;
use serde::Deserialize;

#[derive(Debug, Default, Clone, Deserialize)]
pub struct FoundationQuery {
    pub moat_distance: Option<f64>,
    pub moat_depth: Option<f64>,
    pub water_table_depth: Option<f64>,
    pub wind_speed: Option<f64>,
    pub tilt_deg: Option<f64>,
    pub soil_type: Option<String>,
}

pub struct FoundationAnalyzer {
    moat: MoatAnalyzer,
}

impl FoundationAnalyzer {
    pub fn new() -> Self {
        Self {
            moat: MoatAnalyzer::new(),
        }
    }

    pub fn parse_soil_type(input: &str) -> SoilType {
        input
            .parse::<SoilType>()
            .unwrap_or(SoilType::Loam)
    }

    pub fn analyze_moat(
        &self,
        tower: &TowerMetadata,
        soil_type: &SoilType,
        moat_distance: f64,
        moat_depth: f64,
        water_table_depth: f64,
        wind_speed: f64,
        tilt_deg: f64,
    ) -> MoatAnalysis {
        self.moat.analyze(
            tower,
            soil_type,
            moat_distance,
            moat_depth,
            water_table_depth,
            wind_speed,
            tilt_deg,
        )
    }

    pub fn analyze_moat_by_tower_id(
        &self,
        tower_id: u32,
        query: FoundationQuery,
    ) -> MoatAnalysis {
        let tower = get_default_tower(tower_id);
        let moat_distance = query.moat_distance.unwrap_or(3.0);
        let moat_depth = query.moat_depth.unwrap_or(4.0);
        let water_table_depth = query.water_table_depth.unwrap_or(1.5);
        let wind_speed = query.wind_speed.unwrap_or(15.0);
        let tilt_deg = query.tilt_deg.unwrap_or(0.5);
        let soil_type = query.soil_type
            .as_deref()
            .unwrap_or("loam");
        let soil = Self::parse_soil_type(soil_type);

        self.analyze_moat(
            &tower,
            &soil,
            moat_distance,
            moat_depth,
            water_table_depth,
            wind_speed,
            tilt_deg,
        )
    }

    pub fn multi_soil_analysis(
        &self,
        tower: &TowerMetadata,
        soils: &[SoilType],
        moat_distance: f64,
        moat_depth: f64,
        water_table_depth: f64,
        wind_speed: f64,
        tilt_deg: f64,
    ) -> Vec<(SoilType, MoatAnalysis)> {
        soils
            .iter()
            .map(|soil| {
                let result = self.analyze_moat(
                    tower,
                    soil,
                    moat_distance,
                    moat_depth,
                    water_table_depth,
                    wind_speed,
                    tilt_deg,
                );
                (soil.clone(), result)
            })
            .collect()
    }

    pub fn recommend_soil(
        &self,
        results: &[(SoilType, MoatAnalysis)],
    ) -> (SoilType, f64) {
        results
            .iter()
            .max_by(|a, b| a.1.overall_safety_factor.partial_cmp(&b.1.overall_safety_factor).unwrap())
            .map(|(s, r)| (s.clone(), r.overall_safety_factor))
            .unwrap_or((SoilType::Rock, 0.0))
    }
}

impl Default for FoundationAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_analyzer() -> FoundationAnalyzer {
        FoundationAnalyzer::new()
    }

    #[test]
    fn test_parse_soil_type() {
        assert_eq!(FoundationAnalyzer::parse_soil_type("sand"), SoilType::Sand);
        assert_eq!(FoundationAnalyzer::parse_soil_type("clay"), SoilType::Clay);
        assert_eq!(FoundationAnalyzer::parse_soil_type("loam"), SoilType::Loam);
        assert_eq!(FoundationAnalyzer::parse_soil_type("silt"), SoilType::Silt);
        assert_eq!(FoundationAnalyzer::parse_soil_type("rock"), SoilType::Rock);
        assert_eq!(FoundationAnalyzer::parse_soil_type("unknown"), SoilType::Loam);
    }

    #[test]
    fn test_analyze_moat_returns_valid_result() {
        let fa = make_analyzer();
        let tower = get_default_tower(1);
        let result = fa.analyze_moat(&tower, &SoilType::Loam, 3.0, 4.0, 1.5, 15.0, 0.5);

        assert!(result.overall_safety_factor > 0.0);
        assert!(result.bearing_capacity_reduction > 0.0);
        assert!(result.bearing_capacity_reduction <= 1.0);
        assert!(result.pore_pressure_ratio >= 0.0);
        assert!(result.water_soil_coupling_factor > 0.0);
    }

    #[test]
    fn test_analyze_by_tower_id_defaults() {
        let fa = make_analyzer();
        let query = FoundationQuery::default();
        let result = fa.analyze_moat_by_tower_id(1, query);
        assert!(result.overall_safety_factor > 0.0);
        assert_eq!(result.soil_type, "loam");
    }

    #[test]
    fn test_rock_safer_than_clay() {
        let fa = make_analyzer();
        let tower = get_default_tower(1);
        let soils = vec![SoilType::Rock, SoilType::Clay];
        let results = fa.multi_soil_analysis(&tower, &soils, 3.0, 4.0, 1.5, 15.0, 0.5);
        let rec = fa.recommend_soil(&results);

        assert_eq!(rec.0, SoilType::Rock);
        assert!(rec.1 > 0.0);
    }

    #[test]
    fn test_multi_soil_returns_all() {
        let fa = make_analyzer();
        let tower = get_default_tower(1);
        let soils = vec![SoilType::Sand, SoilType::Silt, SoilType::Loam];
        let results = fa.multi_soil_analysis(&tower, &soils, 3.0, 4.0, 1.5, 15.0, 0.5);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_water_table_affects_coupling() {
        let fa = make_analyzer();
        let tower = get_default_tower(1);
        let wet = fa.analyze_moat(&tower, &SoilType::Silt, 5.0, 4.0, 0.0, 15.0, 0.5);
        let dry = fa.analyze_moat(&tower, &SoilType::Silt, 5.0, 4.0, 10.0, 15.0, 0.5);

        assert!(wet.pore_pressure_ratio > dry.pore_pressure_ratio);
        assert!(wet.water_soil_coupling_factor < dry.water_soil_coupling_factor);
        assert!(wet.overall_safety_factor <= dry.overall_safety_factor);
    }
}

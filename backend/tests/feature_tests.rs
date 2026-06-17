use siege_tower_backend::models::{TowerMetadata, SoilType, SensorData, ClimbingViewpoint, ClimbingExperience};
use siege_tower_backend::stability::StabilityAnalyzer;
use siege_tower_backend::moat_analyzer::MoatAnalyzer;
use siege_tower_backend::config::{AppConfig, GlobalSimConfig, TowerConfigRoot, AlertThresholds,
    SoilConfigRoot, SoilAnalysisDefaults, ServerConfig, ClickHouseConfig, MqttConfig};

fn make_tower(id: u32, name: &str, height: f64, layers: u8, bw: f64, bd: f64,
    weight: f64, strength: f64, e_mod: f64, poisson: f64, design_load: f64, design_wind: f64) -> TowerMetadata {
    TowerMetadata {
        tower_id: id,
        tower_name: name.to_string(),
        build_date: "2024-01-01".to_string(),
        material: "test_material".to_string(),
        total_height: height,
        total_layers: layers,
        base_width: bw,
        base_depth: bd,
        total_weight: weight,
        design_load,
        design_wind_speed: design_wind,
        material_strength: strength,
        elastic_modulus: e_mod,
        poisson_ratio: poisson,
    }
}

fn default_config() -> AppConfig {
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

fn make_sensors(n: u8, base_stress: f64, base_tilt: f64, wind: f64, ground: f64) -> Vec<SensorData> {
    (1..=n).map(|i| {
        let layer_factor = 1.0 + (i as f64 - 1.0) / n as f64 * 0.5;
        SensorData {
            tower_id: 1,
            tower_name: "".to_string(),
            layer_id: i,
            layer_name: format!("L{}", i),
            timestamp: chrono::Utc::now(),
            stress_x: base_stress * layer_factor * 0.6,
            stress_y: base_stress * layer_factor * 0.5,
            stress_z: base_stress * layer_factor * 0.8,
            stress_von_mises: base_stress * layer_factor,
            tilt_x: base_tilt * layer_factor * 0.7,
            tilt_y: base_tilt * layer_factor * 0.7,
            tilt_total: base_tilt * layer_factor,
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
    }).collect()
}

fn dynasty_towers() -> Vec<TowerMetadata> {
    vec![
        make_tower(1, "临冲吕公车", 18.5, 5, 6.2, 4.8, 28.5, 38.0, 9500.0, 0.35, 850.0, 35.0),
        make_tower(2, "临冲吕公车-二号", 21.0, 6, 6.8, 5.2, 36.8, 44.0, 12000.0, 0.35, 1020.0, 40.0),
        make_tower(3, "云梯车", 12.0, 3, 3.5, 2.8, 8.5, 36.0, 10500.0, 0.35, 280.0, 25.0),
        make_tower(4, "冲车", 5.5, 2, 4.2, 3.0, 15.0, 48.0, 13800.0, 0.35, 450.0, 20.0),
    ]
}

fn modern_tower() -> TowerMetadata {
    make_tower(5, "现代塔吊", 60.0, 12, 8.0, 8.0, 95.0, 295.0, 206000.0, 0.30, 8000.0, 55.0)
}

#[test]
fn test_feature_dynasty_comparison_stability_factors() {
    let analyzer = StabilityAnalyzer::new();
    let config = default_config();
    let towers = dynasty_towers();
    let sensors = make_sensors(3, 10.0, 0.3, 10.0, 100.0);

    let mut results = Vec::new();
    for tower in &towers {
        let result = analyzer.check_stability(tower, &sensors, &config);
        results.push((tower.tower_id, tower.tower_name.clone(), result));
    }

    assert_eq!(results.len(), 4);

    for (_, _, r) in &results {
        assert!(r.safety_factor > 0.0);
        assert!(r.wind_resistance_limit > 0.0);
        assert!(r.natural_frequency > 0.0);
        assert!(r.stability_margin >= -100.0);
    }

    let mut sfs: Vec<f64> = results.iter().map(|(_, _, r)| r.safety_factor).collect();
    sfs.sort_by(|a, b| b.partial_cmp(a).unwrap());
    assert!(sfs[0] >= sfs[sfs.len() - 1]);
}

#[test]
fn test_feature_dynasty_comparison_best_safety_is_positive() {
    let analyzer = StabilityAnalyzer::new();
    let config = default_config();
    let towers = dynasty_towers();
    let sensors = make_sensors(3, 8.0, 0.2, 8.0, 100.0);

    let mut best_sf = 0.0;
    let mut best_id = 0;
    for tower in &towers {
        let r = analyzer.check_stability(tower, &sensors, &config);
        if r.safety_factor > best_sf {
            best_sf = r.safety_factor;
            best_id = tower.tower_id;
        }
    }

    assert!(best_sf > 0.0);
    assert!(best_id >= 1 && best_id <= 4);
}

#[test]
fn test_feature_cross_era_material_efficiency() {
    let ancient = dynasty_towers().remove(0);
    let modern = modern_tower();

    assert!(modern.material_strength > ancient.material_strength * 5.0);
    assert!(modern.elastic_modulus > ancient.elastic_modulus * 15.0);

    let ancient_load_eff = ancient.design_load / ancient.total_weight;
    let modern_load_eff = modern.design_load / modern.total_weight;
    assert!(modern_load_eff > ancient_load_eff);

    let ancient_modulus_ratio = ancient.elastic_modulus / ancient.material_strength;
    let modern_modulus_ratio = modern.elastic_modulus / modern.material_strength;
    assert!(modern_modulus_ratio > ancient_modulus_ratio);
}

#[test]
fn test_feature_cross_era_structural_performance() {
    let analyzer = StabilityAnalyzer::new();
    let config = default_config();

    let ancient = dynasty_towers().remove(0);
    let modern = modern_tower();

    let ancient_sensors = make_sensors(ancient.total_layers as u8, 5.0, 0.2, 10.0, 50.0);
    let modern_sensors = make_sensors(modern.total_layers as u8, 100.0, 0.1, 20.0, 200.0);

    let ancient_result = analyzer.check_stability(&ancient, &ancient_sensors, &config);
    let modern_result = analyzer.check_stability(&modern, &modern_sensors, &config);

    assert!(ancient_result.safety_factor > 0.0);
    assert!(modern_result.safety_factor > 0.0);

    assert!(modern_result.wind_resistance_limit > ancient_result.wind_resistance_limit);
}

#[test]
fn test_feature_moat_analysis_safety_factor_normal() {
    let analyzer = MoatAnalyzer::new();
    let tower = dynasty_towers().remove(0);

    let result = analyzer.analyze(&tower, &SoilType::Loam, 5.0, 4.0, 2.0, 10.0, 0.0);

    assert!(result.overall_safety_factor > 0.0);
    assert!(result.effective_bearing_capacity > 0.0);
    assert!(result.slope_stability_factor > 0.0);
    assert!(result.risk_level >= 1 && result.risk_level <= 5);
}

#[test]
fn test_feature_moat_analysis_safety_factor_boundaries() {
    let analyzer = MoatAnalyzer::new();
    let tower = dynasty_towers().remove(0);

    let safe = analyzer.analyze(&tower, &SoilType::Rock, 20.0, 2.0, 10.0, 5.0, 0.0);
    assert!(safe.overall_safety_factor > 2.0);
    assert!(safe.risk_level <= 3);

    let risky = analyzer.analyze(&tower, &SoilType::Clay, 1.0, 10.0, 0.5, 40.0, 5.0);
    assert!(risky.overall_safety_factor < safe.overall_safety_factor);
    assert!(risky.risk_level >= safe.risk_level);
}

#[test]
fn test_feature_moat_analysis_all_soil_types() {
    let analyzer = MoatAnalyzer::new();
    let tower = dynasty_towers().remove(0);
    let soils = vec![SoilType::Sand, SoilType::Clay, SoilType::Silt, SoilType::Rock, SoilType::Loam];

    let mut results = Vec::new();
    for soil in &soils {
        let r = analyzer.analyze(&tower, soil, 5.0, 4.0, 2.0, 10.0, 0.0);
        results.push(r);
    }

    assert_eq!(results.len(), 5);

    let rock_sf = results.iter().find(|r| r.soil_type == "rock").unwrap().overall_safety_factor;
    let clay_sf = results.iter().find(|r| r.soil_type == "clay").unwrap().overall_safety_factor;
    assert!(rock_sf > clay_sf);
}

#[test]
fn test_feature_moat_analysis_recommendations_cover_scenarios() {
    let analyzer = MoatAnalyzer::new();
    let tower = dynasty_towers().remove(0);

    let safe_result = analyzer.analyze(&tower, &SoilType::Rock, 20.0, 2.0, 10.0, 5.0, 0.0);
    assert!(!safe_result.recommendations.is_empty());
    let safe_recs: Vec<&str> = safe_result.recommendations.iter().map(|s| s.as_str()).collect();
    assert!(safe_recs.iter().any(|r| r.contains("安全") || r.contains("监测")));

    let risky_result = analyzer.analyze(&tower, &SoilType::Clay, 1.0, 8.0, 0.5, 35.0, 3.0);
    assert!(!risky_result.recommendations.is_empty());
    assert!(risky_result.recommendations.len() >= 1);
}

#[test]
fn test_feature_climbing_viewpoints_count() {
    let towers = dynasty_towers();
    for tower in &towers {
        let viewpoints = generate_climbing_viewpoints(tower);
        assert_eq!(viewpoints.len(), tower.total_layers as usize);
    }
}

#[test]
fn test_feature_climbing_viewpoints_positions() {
    let tower = dynasty_towers().remove(0);
    let viewpoints = generate_climbing_viewpoints(&tower);

    for (i, vp) in viewpoints.iter().enumerate() {
        assert_eq!(vp.layer_id, (i + 1) as u8);
        assert!(!vp.description.is_empty());
        assert!(!vp.strategic_value.is_empty());
        assert!(vp.camera_position[1] > 0.0);
        assert!(vp.look_at[2] > vp.camera_position[2]);
    }
}

#[test]
fn test_feature_climbing_viewpoints_height_increases() {
    let tower = modern_tower();
    let viewpoints = generate_climbing_viewpoints(&tower);

    for i in 1..viewpoints.len() {
        assert!(viewpoints[i].camera_position[1] > viewpoints[i-1].camera_position[1]);
    }
}

#[test]
fn test_feature_climbing_viewpoints_visibility_range() {
    let tower = dynasty_towers().remove(0);
    let viewpoints = generate_climbing_viewpoints(&tower);

    for i in 1..viewpoints.len() {
        assert!(viewpoints[i].visibility_range_m >= viewpoints[i-1].visibility_range_m);
    }

    assert!(viewpoints.last().unwrap().visibility_range_m > viewpoints.first().unwrap().visibility_range_m);
}

#[test]
fn test_feature_climbing_viewpoints_strategic_value_progression() {
    let tower = dynasty_towers().remove(0);
    let viewpoints = generate_climbing_viewpoints(&tower);

    let values: Vec<&str> = viewpoints.iter().map(|v| v.strategic_value.as_str()).collect();
    assert!(!values.is_empty());
    assert!(values.iter().any(|v| v.contains("瞭望") || v.contains("观察") || v.contains("指挥")));
}

#[test]
fn test_feature_dynasty_comparison_weight_efficiency_ranking() {
    let towers = dynasty_towers();
    let mut efficiencies: Vec<(u32, f64)> = towers.iter()
        .map(|t| (t.tower_id, t.design_load / t.total_weight))
        .collect();
    efficiencies.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    assert!(efficiencies[0].1 > efficiencies[efficiencies.len()-1].1);
    assert_eq!(efficiencies.len(), 4);
}

#[test]
fn test_feature_dynasty_comparison_height_to_base_ratio() {
    let towers = dynasty_towers();
    let mut ratios: Vec<(u32, f64)> = towers.iter()
        .map(|t| (t.tower_id, t.total_height / t.base_width))
        .collect();
    ratios.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    assert!(ratios[0].1 > 1.0);
    assert!(ratios[ratios.len()-1].1 > 0.0);
}

#[test]
fn test_feature_cross_era_technology_gap() {
    let ancient = dynasty_towers().remove(0);
    let modern = modern_tower();

    let strength_ratio = modern.material_strength / ancient.material_strength;
    let modulus_ratio = modern.elastic_modulus / ancient.elastic_modulus;
    let height_ratio = modern.total_height / ancient.total_height;

    assert!(strength_ratio > 5.0);
    assert!(modulus_ratio > 15.0);
    assert!(height_ratio > 3.0);
}

#[test]
fn test_feature_moat_analysis_extreme_wind() {
    let analyzer = MoatAnalyzer::new();
    let tower = dynasty_towers().remove(0);

    let calm = analyzer.analyze(&tower, &SoilType::Loam, 5.0, 4.0, 2.0, 0.0, 0.0);
    let storm = analyzer.analyze(&tower, &SoilType::Loam, 5.0, 4.0, 2.0, 50.0, 0.0);

    assert!(storm.lateral_displacement_mm > calm.lateral_displacement_mm);
    assert!(storm.risk_level >= calm.risk_level);
}

#[test]
fn test_feature_moat_analysis_tilt_effect() {
    let analyzer = MoatAnalyzer::new();
    let tower = dynasty_towers().remove(0);

    let straight = analyzer.analyze(&tower, &SoilType::Loam, 5.0, 4.0, 2.0, 10.0, 0.0);
    let tilted = analyzer.analyze(&tower, &SoilType::Loam, 5.0, 4.0, 2.0, 10.0, 5.0);

    assert!(tilted.lateral_displacement_mm >= straight.lateral_displacement_mm);
}

#[test]
fn test_feature_moat_zero_settlement_increase_impossible() {
    let analyzer = MoatAnalyzer::new();
    let tower = dynasty_towers().remove(0);

    let result = analyzer.analyze(&tower, &SoilType::Rock, 10.0, 4.0, 10.0, 10.0, 0.0);
    assert!(result.settlement_increase_pct >= 0.0);
    assert!(result.settlement_increase_pct <= 100.0);
}

#[test]
fn test_feature_climbing_tower_name_in_experience() {
    let tower = dynasty_towers().remove(0);
    let exp = generate_climbing_experience(&tower);

    assert_eq!(exp.tower_id, tower.tower_id);
    assert_eq!(exp.tower_name, tower.tower_name);
    assert_eq!(exp.total_height, tower.total_height);
    assert!(!exp.battlefield_description.is_empty());
    assert_eq!(exp.viewpoints.len(), tower.total_layers as usize);
}

#[test]
fn test_feature_integration_full_dynasty_workflow() {
    let analyzer = StabilityAnalyzer::new();
    let config = default_config();
    let towers = dynasty_towers();

    let mut comparison = Vec::new();
    for tower in &towers {
        let sensors = make_sensors(tower.total_layers as u8, 10.0, 0.2, 12.0, 80.0);
        let result = analyzer.check_stability(tower, &sensors, &config);
        let weight_eff = tower.design_load / tower.total_weight;
        let hb_ratio = tower.total_height / tower.base_width;

        comparison.push((
            tower.tower_id,
            tower.tower_name.clone(),
            result.safety_factor,
            result.wind_resistance_limit,
            result.natural_frequency,
            result.stability_margin,
            weight_eff,
            hb_ratio,
        ));
    }

    assert_eq!(comparison.len(), 4);

    let mut best_sf = comparison[0].clone();
    let mut best_wind = comparison[0].clone();
    for item in &comparison {
        if item.2 > best_sf.2 { best_sf = item.clone(); }
        if item.3 > best_wind.3 { best_wind = item.clone(); }
    }

    assert!(best_sf.2 > 0.0);
    assert!(best_wind.3 > 0.0);
}

fn generate_climbing_viewpoints(tower: &TowerMetadata) -> Vec<ClimbingViewpoint> {
    let layer_height = tower.total_height / tower.total_layers as f64;
    let mut viewpoints = Vec::new();

    for i in 0..tower.total_layers {
        let layer_id = (i + 1) as u8;
        let y = layer_height * (i as f64 + 0.5);
        let height_ratio = (i as f64 + 1.0) / tower.total_layers as f64;
        let visibility = 500.0 + height_ratio * 1500.0;

        let acrophobia_risk_level = if y < 5.0 { 1 } else if y < 10.0 { 2 } else if y < 20.0 { 3 } else if y < 35.0 { 4 } else { 5 };
        let recommended_fov_deg = if acrophobia_risk_level <= 2 { 75.0 } else if acrophobia_risk_level == 3 { 65.0 } else if acrophobia_risk_level == 4 { 55.0 } else { 45.0 };
        let transition_duration_ms = if acrophobia_risk_level <= 2 { 500 } else if acrophobia_risk_level == 3 { 1000 } else if acrophobia_risk_level == 4 { 1500 } else { 2000 };

        let description = if height_ratio < 0.33 {
            format!("第{}层：基层作战区，士兵集结与装备存放", layer_id)
        } else if height_ratio < 0.66 {
            format!("第{}层：中层射击区，弓弩手抛石机位", layer_id)
        } else {
            format!("第{}层：顶层指挥区，统帅瞭望与指挥", layer_id)
        };

        let strategic_value = if height_ratio < 0.33 {
            "兵力投送"
        } else if height_ratio < 0.66 {
            "火力压制"
        } else {
            "指挥瞭望"
        };

        viewpoints.push(ClimbingViewpoint {
            layer_id,
            layer_name: format!("第{}层", layer_id),
            camera_position: [0.0, y, tower.base_depth / 2.0 + 0.5],
            look_at: [0.0, y, tower.base_depth + 20.0],
            description,
            visibility_range_m: visibility,
            strategic_value: strategic_value.to_string(),
            height_above_ground_m: y,
            acrophobia_risk_level,
            recommended_fov_deg,
            transition_duration_ms,
        });
    }

    viewpoints
}

fn generate_climbing_experience(tower: &TowerMetadata) -> ClimbingExperience {
    ClimbingExperience {
        tower_id: tower.tower_id,
        tower_name: tower.tower_name.clone(),
        viewpoints: generate_climbing_viewpoints(tower),
        total_height: tower.total_height,
        battlefield_description: "古代战场场景：城墙、护城河、敌楼、兵营、攻城器械阵列".to_string(),
    }
}

#[test]
fn test_feature_climbing_acrophobia_risk_increases_with_height() {
    let tower = modern_tower();
    let viewpoints = generate_climbing_viewpoints(&tower);

    for i in 1..viewpoints.len() {
        assert!(viewpoints[i].acrophobia_risk_level >= viewpoints[i-1].acrophobia_risk_level);
    }
    assert!(viewpoints.last().unwrap().acrophobia_risk_level >= 3);
}

#[test]
fn test_feature_climbing_fov_decreases_with_height() {
    let tower = modern_tower();
    let viewpoints = generate_climbing_viewpoints(&tower);

    for vp in &viewpoints {
        assert!(vp.recommended_fov_deg >= 45.0);
        assert!(vp.recommended_fov_deg <= 75.0);
    }

    let high_viewpoints: Vec<_> = viewpoints.iter().filter(|v| v.acrophobia_risk_level >= 4).collect();
    let low_viewpoints: Vec<_> = viewpoints.iter().filter(|v| v.acrophobia_risk_level <= 2).collect();
    if !high_viewpoints.is_empty() && !low_viewpoints.is_empty() {
        assert!(high_viewpoints[0].recommended_fov_deg < low_viewpoints[0].recommended_fov_deg);
    }
}

#[test]
fn test_feature_climbing_transition_duration_increases_with_height() {
    let tower = modern_tower();
    let viewpoints = generate_climbing_viewpoints(&tower);

    for vp in &viewpoints {
        assert!(vp.transition_duration_ms >= 500);
        assert!(vp.transition_duration_ms <= 2000);
    }

    for i in 1..viewpoints.len() {
        if viewpoints[i].acrophobia_risk_level > viewpoints[i-1].acrophobia_risk_level {
            assert!(viewpoints[i].transition_duration_ms >= viewpoints[i-1].transition_duration_ms);
        }
    }
}

#[test]
fn test_feature_moat_water_soil_coupling() {
    let analyzer = MoatAnalyzer::new();
    let tower = dynasty_towers().remove(0);

    let dry = analyzer.analyze(&tower, &SoilType::Loam, 5.0, 4.0, 10.0, 10.0, 0.0);
    let wet = analyzer.analyze(&tower, &SoilType::Loam, 5.0, 4.0, 0.0, 10.0, 0.0);

    assert!(dry.pore_pressure_ratio < wet.pore_pressure_ratio);
    assert!(dry.water_soil_coupling_factor > wet.water_soil_coupling_factor);
    assert!(wet.pore_pressure_ratio >= 0.0);
    assert!(wet.pore_pressure_ratio <= 0.5);
    assert!(dry.water_soil_coupling_factor >= 0.5);
    assert!(dry.water_soil_coupling_factor <= 1.0);
}

#[test]
fn test_feature_moat_seepage_force_present() {
    let analyzer = MoatAnalyzer::new();
    let tower = dynasty_towers().remove(0);

    let with_water = analyzer.analyze(&tower, &SoilType::Sand, 5.0, 4.0, 0.0, 10.0, 0.0);
    let no_water = analyzer.analyze(&tower, &SoilType::Sand, 5.0, 4.0, 10.0, 10.0, 0.0);

    assert!(with_water.seepage_force_kn >= 0.0);
    assert!(no_water.seepage_force_kn <= with_water.seepage_force_kn);
}

use crate::database::get_default_tower;
use crate::models::{ClimbingViewpoint, ClimbingExperience, TowerMetadata};

pub struct VrSiegeTower {}

impl VrSiegeTower {
    pub fn new() -> Self {
        Self {}
    }

    pub fn compute_acrophobia_risk(height_m: f64) -> u8 {
        if height_m < 5.0 { 1 }
        else if height_m < 10.0 { 2 }
        else if height_m < 20.0 { 3 }
        else if height_m < 35.0 { 4 }
        else { 5 }
    }

    pub fn compute_recommended_fov(acrophobia_level: u8) -> f64 {
        if acrophobia_level <= 2 { 75.0 }
        else if acrophobia_level == 3 { 65.0 }
        else if acrophobia_level == 4 { 55.0 }
        else { 45.0 }
    }

    pub fn compute_transition_duration(acrophobia_level: u8) -> u32 {
        if acrophobia_level <= 2 { 500 }
        else if acrophobia_level == 3 { 1000 }
        else if acrophobia_level == 4 { 1500 }
        else { 2000 }
    }

    fn build_viewpoint(tower: &TowerMetadata, layer: u8, layer_y: f64, h_ratio: f64) -> ClimbingViewpoint {
        let description = if h_ratio < 0.3 {
            "底层视角：观察地面部署与城墙根基".to_string()
        } else if h_ratio < 0.6 {
            "中层视角：可观察城墙中部防御与敌军动向".to_string()
        } else if h_ratio < 0.85 {
            "高层视角：俯瞰战场全局，观察远距离敌情".to_string()
        } else {
            "顶层视角：全面掌控战场态势，通信指挥位置".to_string()
        };

        let strategic_value = if h_ratio < 0.3 {
            "近距突击准备".to_string()
        } else if h_ratio < 0.6 {
            "中距火力压制".to_string()
        } else if h_ratio < 0.85 {
            "远距侦察指挥".to_string()
        } else {
            "全局指挥调度".to_string()
        };

        let visibility = 100.0 + layer_y * 50.0;
        let acrophobia_risk_level = Self::compute_acrophobia_risk(layer_y);
        let recommended_fov_deg = Self::compute_recommended_fov(acrophobia_risk_level);
        let transition_duration_ms = Self::compute_transition_duration(acrophobia_risk_level);

        ClimbingViewpoint {
            layer_id: layer,
            layer_name: format!("L{}", layer),
            camera_position: [0.0, layer_y, tower.base_depth / 2.0 + 0.5],
            look_at: [0.0, layer_y, tower.base_depth + 20.0],
            description,
            visibility_range_m: visibility,
            strategic_value,
            height_above_ground_m: layer_y,
            acrophobia_risk_level,
            recommended_fov_deg,
            transition_duration_ms,
        }
    }

    pub fn generate_viewpoints(&self, tower: &TowerMetadata) -> Vec<ClimbingViewpoint> {
        let layer_height = tower.total_height / tower.total_layers as f64;
        (1..=tower.total_layers)
            .map(|layer| {
                let layer_y = layer as f64 * layer_height;
                let h_ratio = layer as f64 / tower.total_layers as f64;
                Self::build_viewpoint(tower, layer as u8, layer_y, h_ratio)
            })
            .collect()
    }

    pub fn build_experience(&self, tower_id: u32) -> ClimbingExperience {
        let tower = get_default_tower(tower_id);
        let viewpoints = self.generate_viewpoints(&tower);
        let battlefield_description = format!(
            "{}高{}m，共{}层，可提供从近距突击到全局指挥的多层次战场视角",
            tower.tower_name, tower.total_height, tower.total_layers,
        );

        ClimbingExperience {
            tower_id,
            tower_name: tower.tower_name.clone(),
            viewpoints,
            total_height: tower.total_height,
            battlefield_description,
        }
    }
}

impl Default for VrSiegeTower {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acrophobia_risk_band_1_low() {
        assert_eq!(VrSiegeTower::compute_acrophobia_risk(0.0), 1);
        assert_eq!(VrSiegeTower::compute_acrophobia_risk(4.9), 1);
    }

    #[test]
    fn test_acrophobia_risk_band_2_medium_low() {
        assert_eq!(VrSiegeTower::compute_acrophobia_risk(5.0), 2);
        assert_eq!(VrSiegeTower::compute_acrophobia_risk(9.9), 2);
    }

    #[test]
    fn test_acrophobia_risk_band_3_medium_high() {
        assert_eq!(VrSiegeTower::compute_acrophobia_risk(10.0), 3);
        assert_eq!(VrSiegeTower::compute_acrophobia_risk(19.9), 3);
    }

    #[test]
    fn test_acrophobia_risk_band_4_high() {
        assert_eq!(VrSiegeTower::compute_acrophobia_risk(20.0), 4);
        assert_eq!(VrSiegeTower::compute_acrophobia_risk(34.9), 4);
    }

    #[test]
    fn test_acrophobia_risk_band_5_very_high() {
        assert_eq!(VrSiegeTower::compute_acrophobia_risk(35.0), 5);
        assert_eq!(VrSiegeTower::compute_acrophobia_risk(100.0), 5);
    }

    #[test]
    fn test_fov_matches_risk() {
        assert_eq!(VrSiegeTower::compute_recommended_fov(1), 75.0);
        assert_eq!(VrSiegeTower::compute_recommended_fov(2), 75.0);
        assert_eq!(VrSiegeTower::compute_recommended_fov(3), 65.0);
        assert_eq!(VrSiegeTower::compute_recommended_fov(4), 55.0);
        assert_eq!(VrSiegeTower::compute_recommended_fov(5), 45.0);
    }

    #[test]
    fn test_fov_decreases_monotonically() {
        let levels = [1u8, 2, 3, 4, 5];
        let fovs: Vec<_> = levels.iter().map(|&l| VrSiegeTower::compute_recommended_fov(l)).collect();
        for i in 1..fovs.len() {
            assert!(fovs[i] <= fovs[i - 1]);
        }
    }

    #[test]
    fn test_transition_increases_with_risk() {
        let levels = [1u8, 2, 3, 4, 5];
        let durs: Vec<_> = levels.iter().map(|&l| VrSiegeTower::compute_transition_duration(l)).collect();
        for i in 1..durs.len() {
            assert!(durs[i] >= durs[i - 1]);
        }
        assert_eq!(durs[0], 500);
        assert_eq!(durs[4], 2000);
    }

    #[test]
    fn test_generate_viewpoints_count_matches_layers() {
        let vr = VrSiegeTower::new();
        let tower = get_default_tower(1);
        let vps = vr.generate_viewpoints(&tower);
        assert_eq!(vps.len() as u8, tower.total_layers);
    }

    #[test]
    fn test_viewpoints_height_increases() {
        let vr = VrSiegeTower::new();
        let tower = get_default_tower(5);
        let vps = vr.generate_viewpoints(&tower);
        for i in 1..vps.len() {
            assert!(vps[i].height_above_ground_m > vps[i - 1].height_above_ground_m);
        }
    }

    #[test]
    fn test_viewpoints_risk_increases_with_height() {
        let vr = VrSiegeTower::new();
        let tower = get_default_tower(5);
        let vps = vr.generate_viewpoints(&tower);
        for i in 1..vps.len() {
            assert!(vps[i].acrophobia_risk_level >= vps[i - 1].acrophobia_risk_level);
        }
    }

    #[test]
    fn test_build_experience_contains_all_fields() {
        let vr = VrSiegeTower::new();
        let exp = vr.build_experience(3);
        assert_eq!(exp.tower_id, 3);
        assert!(!exp.tower_name.is_empty());
        assert_eq!(exp.viewpoints.len() as u8, get_default_tower(3).total_layers);
        assert!(!exp.battlefield_description.is_empty());
    }
}

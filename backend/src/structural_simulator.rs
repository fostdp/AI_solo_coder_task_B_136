use crate::config::AppConfig;
use crate::database::ClickHouseClient;
use crate::fem::FEMAnalysis;
use crate::models::{TowerMetadata};
use crate::stability::StabilityAnalyzer;
use crate::{SimCommand, SimResponse};
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{info, error};

pub struct StructuralSimulator {
    config: Arc<AppConfig>,
    db: Arc<ClickHouseClient>,
    stability: StabilityAnalyzer,
    fem: Mutex<FEMAnalysis>,
}

impl StructuralSimulator {
    pub fn new(config: Arc<AppConfig>, db: Arc<ClickHouseClient>) -> Self {
        let stability = StabilityAnalyzer::new();
        let fem = FEMAnalysis::new();
        Self {
            config,
            db,
            stability,
            fem: Mutex::new(fem),
        }
    }

    pub async fn run_full_analysis(
        &self,
        tower: &TowerMetadata,
        sensor_data: &[crate::models::SensorData],
    ) -> SimResponse {
        info!("[结构仿真] 开始完整分析: tower={}", tower.tower_id);
        let sim = &self.config.tower.global_simulation;
        let design_wind = tower.design_wind_speed;
        let wind_speed = sensor_data.first().map(|s| s.wind_speed_mps).unwrap_or(15.0);
        info!("[结构仿真] 风速={}m/s, 设计风速={}m/s", wind_speed, design_wind);

        info!("[结构仿真] 步骤1: 稳定性检查");
        let analysis = match std::panic::catch_unwind(AssertUnwindSafe(|| {
            self.stability.check_stability(tower, sensor_data, &self.config)
        })) {
            Ok(a) => a,
            Err(e) => {
                error!("[结构仿真] 稳定性检查 panic: {:?}", e);
                return Self::error_response("稳定性检查失败");
            }
        };
        info!("[结构仿真] 稳定性检查完成, 安全系数={:.3}", analysis.safety_factor);

        info!("[结构仿真] 步骤2: FEM分析");
        let (fem_results, layer_stresses) = {
            let mut fem = self.fem.lock().await;
            info!("[FEM] 步骤2.1: 构建塔体网格");
            if let Err(e) = std::panic::catch_unwind(AssertUnwindSafe(|| fem.build_tower_mesh(tower))) {
                error!("[FEM] 构建网格 panic: {:?}", e);
                return Self::error_response("构建网格失败");
            }
            info!("[FEM] 步骤2.2: 组装刚度矩阵");
            if let Err(e) = std::panic::catch_unwind(AssertUnwindSafe(|| fem.assemble_matrices())) {
                error!("[FEM] 组装矩阵 panic: {:?}", e);
                return Self::error_response("组装刚度矩阵失败");
            }
            info!("[FEM] 步骤2.3: 施加荷载");
            if let Err(e) = std::panic::catch_unwind(AssertUnwindSafe(|| {
                fem.apply_loads(tower, wind_speed, 0.0, sim.gravity, sim.air_density, sim.wind_drag_coefficient)
            })) {
                error!("[FEM] 施加荷载 panic: {:?}", e);
                return Self::error_response("施加荷载失败");
            }
            info!("[FEM] 步骤2.4: 边界条件");
            if let Err(e) = std::panic::catch_unwind(AssertUnwindSafe(|| fem.apply_boundary_conditions(tower))) {
                error!("[FEM] 边界条件 panic: {:?}", e);
                return Self::error_response("边界条件失败");
            }
            info!("[FEM] 步骤2.5: 求解");
            if let Err(e) = std::panic::catch_unwind(AssertUnwindSafe(|| fem.solve())) {
                error!("[FEM] 求解 panic: {:?}", e);
                return Self::error_response("求解失败");
            }
            info!("[FEM] 步骤2.6: 二阶效应");
            if let Err(e) = std::panic::catch_unwind(AssertUnwindSafe(|| {
                fem.apply_second_order_effects(
                    tower,
                    design_wind,
                    sim.gravity,
                    sim.air_density,
                    sim.wind_drag_coefficient,
                    self.config.tower.global_simulation.second_order_enabled,
                    &self.config.tower.alert_thresholds,
                )
            })) {
                error!("[FEM] 二阶效应 panic: {:?}", e);
                return Self::error_response("二阶效应失败");
            }

            info!("[FEM] 步骤2.7: 提取结果");
            let fr = match std::panic::catch_unwind(AssertUnwindSafe(|| {
                fem.get_node_results(tower.tower_id, chrono::Utc::now(), tower.material_strength)
            })) {
                Ok(v) => v,
                Err(e) => {
                    error!("[FEM] 提取节点结果 panic: {:?}", e);
                    return Self::error_response("提取节点结果失败");
                }
            };
            let ls = match std::panic::catch_unwind(AssertUnwindSafe(|| {
                fem.get_layer_stresses(tower, wind_speed)
            })) {
                Ok(v) => v,
                Err(e) => {
                    error!("[FEM] 提取层应力 panic: {:?}", e);
                    return Self::error_response("提取层应力失败");
                }
            };
            (fr, ls)
        };
        info!("[结构仿真] FEM完成, 节点数={}, 层数={}", fem_results.len(), layer_stresses.len());

        let _ = self.db.insert_structure_analysis(&analysis).await;
        let _ = self.db.insert_fem_results(&fem_results).await;
        info!("[结构仿真] 数据库写入完成");

        SimResponse {
            analysis,
            fem_sample: Some(fem_results.iter().take(20).cloned().collect()),
            fem_total_nodes: fem_results.len(),
            layer_stresses,
        }
    }

    fn error_response(msg: &str) -> SimResponse {
        use crate::models::StructureAnalysis;
        error!("[结构仿真] 返回错误响应: {}", msg);
        SimResponse {
            analysis: StructureAnalysis::default_error(msg),
            fem_sample: None,
            fem_total_nodes: 0,
            layer_stresses: Vec::new(),
        }
    }

    pub async fn run_custom_analysis(
        &self,
        tower: &TowerMetadata,
        wind_speed: f64,
        tilt_deg: f64,
    ) -> SimResponse {
        info!("[结构仿真] 开始自定义分析: tower={}, wind={}m/s, tilt={}°", tower.tower_id, wind_speed, tilt_deg);
        let sim = &self.config.tower.global_simulation;
        let dummy = crate::handlers::generate_dummy_sensor_data_with_params(tower, wind_speed, tilt_deg);

        info!("[结构仿真] 步骤1: 稳定性检查");
        let analysis = match std::panic::catch_unwind(AssertUnwindSafe(|| {
            self.stability.check_stability(tower, &dummy, &self.config)
        })) {
            Ok(a) => a,
            Err(e) => {
                error!("[结构仿真] 稳定性检查 panic: {:?}", e);
                return Self::error_response("稳定性检查失败");
            }
        };
        info!("[结构仿真] 稳定性检查完成, 安全系数={:.3}", analysis.safety_factor);

        info!("[结构仿真] 步骤2: FEM分析");
        let (fem_results, layer_stresses) = {
            let mut fem = self.fem.lock().await;
            info!("[FEM] 步骤2.1: 构建塔体网格");
            if let Err(e) = std::panic::catch_unwind(AssertUnwindSafe(|| fem.build_tower_mesh(tower))) {
                error!("[FEM] 构建网格 panic: {:?}", e);
                return Self::error_response("构建网格失败");
            }
            info!("[FEM] 步骤2.2: 组装刚度矩阵");
            if let Err(e) = std::panic::catch_unwind(AssertUnwindSafe(|| fem.assemble_matrices())) {
                error!("[FEM] 组装矩阵 panic: {:?}", e);
                return Self::error_response("组装刚度矩阵失败");
            }
            info!("[FEM] 步骤2.3: 施加荷载");
            if let Err(e) = std::panic::catch_unwind(AssertUnwindSafe(|| {
                fem.apply_loads(tower, wind_speed, 0.0, sim.gravity, sim.air_density, sim.wind_drag_coefficient)
            })) {
                error!("[FEM] 施加荷载 panic: {:?}", e);
                return Self::error_response("施加荷载失败");
            }
            info!("[FEM] 步骤2.4: 边界条件");
            if let Err(e) = std::panic::catch_unwind(AssertUnwindSafe(|| fem.apply_boundary_conditions(tower))) {
                error!("[FEM] 边界条件 panic: {:?}", e);
                return Self::error_response("边界条件失败");
            }
            info!("[FEM] 步骤2.5: 求解");
            if let Err(e) = std::panic::catch_unwind(AssertUnwindSafe(|| fem.solve())) {
                error!("[FEM] 求解 panic: {:?}", e);
                return Self::error_response("求解失败");
            }
            info!("[FEM] 步骤2.6: 二阶效应");
            if let Err(e) = std::panic::catch_unwind(AssertUnwindSafe(|| {
                fem.apply_second_order_effects(
                    tower,
                    tower.design_wind_speed,
                    sim.gravity,
                    sim.air_density,
                    sim.wind_drag_coefficient,
                    self.config.tower.global_simulation.second_order_enabled,
                    &self.config.tower.alert_thresholds,
                )
            })) {
                error!("[FEM] 二阶效应 panic: {:?}", e);
                return Self::error_response("二阶效应失败");
            }

            info!("[FEM] 步骤2.7: 提取结果");
            let fr = match std::panic::catch_unwind(AssertUnwindSafe(|| {
                fem.get_node_results(tower.tower_id, chrono::Utc::now(), tower.material_strength)
            })) {
                Ok(v) => v,
                Err(e) => {
                    error!("[FEM] 提取节点结果 panic: {:?}", e);
                    return Self::error_response("提取节点结果失败");
                }
            };
            let ls = match std::panic::catch_unwind(AssertUnwindSafe(|| {
                fem.get_layer_stresses(tower, wind_speed)
            })) {
                Ok(v) => v,
                Err(e) => {
                    error!("[FEM] 提取层应力 panic: {:?}", e);
                    return Self::error_response("提取层应力失败");
                }
            };
            (fr, ls)
        };
        info!("[结构仿真] FEM完成, 节点数={}, 层数={}", fem_results.len(), layer_stresses.len());

        SimResponse {
            analysis,
            fem_sample: Some(fem_results.iter().take(20).cloned().collect()),
            fem_total_nodes: fem_results.len(),
            layer_stresses,
        }
    }
}

pub async fn run_structural_simulator(
    mut cmd_rx: mpsc::Receiver<SimCommand>,
    simulator: Arc<StructuralSimulator>,
) {
    info!("[结构仿真模块] 主循环启动，等待命令...");
    while let Some(cmd) = cmd_rx.recv().await {
        info!("[结构仿真模块] 收到命令");
        match cmd {
            SimCommand::RunFullAnalysis { tower, sensor_data, resp_tx, .. } => {
                info!("[结构仿真模块] 执行 RunFullAnalysis, tower={}", tower.tower_id);
                let resp = simulator.run_full_analysis(&tower, &sensor_data).await;
                info!("[结构仿真模块] RunFullAnalysis 完成，发送响应");
                let _ = resp_tx.send(resp);
                info!("[结构仿真模块] 响应已发送");
            }
            SimCommand::RunCustomAnalysis { tower, wind_speed, tilt_deg, resp_tx } => {
                info!("[结构仿真模块] 执行 RunCustomAnalysis, tower={}, wind={}m/s", tower.tower_id, wind_speed);
                let resp = simulator.run_custom_analysis(&tower, wind_speed, tilt_deg).await;
                info!("[结构仿真模块] RunCustomAnalysis 完成，发送响应");
                let _ = resp_tx.send(resp);
                info!("[结构仿真模块] 响应已发送");
            }
        }
    }
    error!("[结构仿真模块] 主循环退出 - channel 已关闭");
}

use crate::config::AppConfig;
use crate::database::{ClickHouseClient, get_default_tower};
use crate::dtu_receiver::DtuReceiver;
use crate::models::{
    SensorData, BatchSensorData, TowerMetadata, StructureAnalysis,
    AlertEvent, GroundAnalysis, SoilType, ApiResponse,
    TowerCategory, DynastyComparison, TowerComparisonItem, ComparisonMetrics,
    CrossEraComparison, EraComparisonData, CrossEraRatios,
    ClimbingViewpoint, ClimbingExperience,
};
use crate::moat_analyzer::MoatAnalyzer;
use crate::mqtt_client::MqttService;
use crate::stability::StabilityAnalyzer;
use crate::{SimCommand, SoilCommand, AlarmCommand};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tokio_stream::StreamExt;
use tracing::{info, warn};

pub struct AppState {
    pub config: AppConfig,
    pub db: Arc<ClickHouseClient>,
    pub dtu: DtuReceiver,
    pub mqtt: Arc<Mutex<MqttService>>,
    pub sim_cmd_tx: mpsc::Sender<SimCommand>,
    pub soil_cmd_tx: mpsc::Sender<SoilCommand>,
    pub alarm_cmd_tx: mpsc::Sender<AlarmCommand>,
    pub alert_tx: broadcast::Sender<AlertEvent>,
    pub analysis_tx: broadcast::Sender<StructureAnalysis>,
    pub sensor_tx: broadcast::Sender<Vec<SensorData>>,
}

pub type SharedState = Arc<AppState>;

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub layer_id: Option<u8>,
}

#[derive(Debug, Deserialize)]
pub struct AnalysisQuery {
    pub wind_speed: Option<f64>,
    pub tilt_deg: Option<f64>,
    pub soil_type: Option<String>,
    pub moisture_pct: Option<f64>,
}

pub async fn prometheus_metrics() -> impl IntoResponse {
    use axum::http::header;
    let body = crate::metrics::gather_metrics();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
}

pub async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({
        "status": "ok",
        "service": "siege-tower-backend",
        "version": "2.0.0"
    })))
}

pub async fn get_all_towers(
    State(state): State<SharedState>,
) -> impl IntoResponse {
    match state.db.query_all_towers().await {
        Ok(towers) => (StatusCode::OK, Json(ApiResponse::success(towers))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<Vec<TowerMetadata>>::error(500, e.to_string())))
    }
}

pub async fn get_tower(
    State(_state): State<SharedState>,
    Path(tower_id): Path<u32>,
) -> impl IntoResponse {
    let tower = get_default_tower(tower_id);
    (StatusCode::OK, Json(ApiResponse::success(tower)))
}

pub async fn get_tower_config(
    State(state): State<SharedState>,
    Path(tower_id): Path<u32>,
) -> impl IntoResponse {
    match state.config.get_tower(tower_id) {
        Some(tc) => (StatusCode::OK, Json(ApiResponse::success(tc.clone()))),
        None => (StatusCode::NOT_FOUND, Json(ApiResponse::<crate::config::TowerConfigEntry>::error(404, "塔不存在".to_string())))
    }
}

pub async fn get_all_soil_configs(
    State(state): State<SharedState>,
) -> impl IntoResponse {
    (StatusCode::OK, Json(ApiResponse::success(state.config.soil.soil_types.clone())))
}

pub async fn get_sensor_data(
    State(state): State<SharedState>,
    Path(tower_id): Path<u32>,
    Query(params): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100);
    match state.db.query_recent_sensor_data(tower_id, limit).await {
        Ok(data) => (StatusCode::OK, Json(ApiResponse::success(data))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<Vec<SensorData>>::error(500, e.to_string())))
    }
}

pub async fn receive_sensor_data(
    State(state): State<SharedState>,
    Path(tower_id): Path<u32>,
    Json(payload): Json<BatchSensorData>,
) -> impl IntoResponse {
    let tower = get_default_tower(tower_id);

    match state.dtu.process_batch(payload, tower.clone()).await {
        Ok(expanded) => {
            let _ = state.sensor_tx.send(expanded.clone());
            info!("塔 {} 接收传感器数据: {} 层", tower_id, expanded.len());

            let response = ApiResponse::success(serde_json::json!({
                "received": expanded.len(),
                "tower_id": tower_id,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }));
            (StatusCode::OK, Json(response))
        }
        Err(e) => {
            warn!("传感器数据校验失败: {}", e);
            (StatusCode::BAD_REQUEST, Json(ApiResponse::<serde_json::Value>::error(400, e)))
        }
    }
}

pub async fn run_analysis(
    State(state): State<SharedState>,
    Path(tower_id): Path<u32>,
    Query(params): Query<AnalysisQuery>,
) -> impl IntoResponse {
    info!("[Handler] 完整分析请求: tower={}, wind={:?}, tilt={:?}", tower_id, params.wind_speed, params.tilt_deg);
    let tower = get_default_tower(tower_id);
    let wind_speed = params.wind_speed.unwrap_or(15.0);
    let tilt_deg = params.tilt_deg.unwrap_or(0.5);
    let moisture = params.moisture_pct;
    info!("[Handler] 解析参数完成: wind_speed={}, tilt_deg={}", wind_speed, tilt_deg);

    let (sim_tx, sim_rx) = oneshot::channel();
    let sim_cmd = SimCommand::RunCustomAnalysis {
        tower: tower.clone(),
        wind_speed,
        tilt_deg,
        resp_tx: sim_tx,
    };
    info!("[Handler] 准备发送 SimCommand");

    let (soil_tx, soil_rx) = oneshot::channel();
    let soil_cmd = SoilCommand::AnalyzeAll {
        tower: tower.clone(),
        wind_speed,
        tilt_deg,
        moisture_pct: moisture,
        resp_tx: soil_tx,
    };

    let soil_type = params.soil_type.as_deref().unwrap_or("loam").parse::<SoilType>().unwrap_or(SoilType::Loam);
    let (ground_one_tx, ground_one_rx) = oneshot::channel();
    let ground_one_cmd = SoilCommand::AnalyzeOne {
        tower: tower.clone(),
        soil_type,
        wind_speed,
        tilt_deg,
        moisture_pct: moisture,
        additional_settlement: None,
        resp_tx: ground_one_tx,
    };

    info!("[Handler] 发送结构仿真命令");
    if let Err(e) = state.sim_cmd_tx.send(sim_cmd).await {
        warn!("[Handler] 结构仿真命令发送失败: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<serde_json::Value>::error(500, format!("仿真服务不可用: {}", e))));
    }
    info!("[Handler] 结构仿真命令发送成功");
    info!("[Handler] 发送土壤分析命令(All)");
    if let Err(e) = state.soil_cmd_tx.send(soil_cmd).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<serde_json::Value>::error(500, format!("土壤服务不可用: {}", e))));
    }
    info!("[Handler] 发送土壤分析命令(One)");
    if let Err(e) = state.soil_cmd_tx.send(ground_one_cmd).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<serde_json::Value>::error(500, format!("土壤服务不可用: {}", e))));
    }
    info!("[Handler] 所有命令发送成功，等待响应...");

    let (sim_result, soil_result, ground_result) = tokio::join!(sim_rx, soil_rx, ground_one_rx);

    let sim_resp = match sim_result {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<serde_json::Value>::error(500, format!("仿真失败: {}", e)))),
    };

    let all_grounds = match soil_result {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<serde_json::Value>::error(500, format!("土壤分析失败: {}", e)))),
    };

    let ground = match ground_result {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<serde_json::Value>::error(500, format!("单土壤分析失败: {}", e)))),
    };

    let _ = state.db.insert_structure_analysis(&sim_resp.analysis).await;
    let _ = state.analysis_tx.send(sim_resp.analysis.clone());

    let (alarm_tx, alarm_rx) = oneshot::channel();
    let dummy_sensors = generate_dummy_sensor_data_with_params(&tower, wind_speed, tilt_deg);
    let alarm_cmd = AlarmCommand::Evaluate {
        tower: tower.clone(),
        sensor_data: dummy_sensors,
        analysis: sim_resp.analysis.clone(),
        resp_tx: alarm_tx,
    };
    let _ = state.alarm_cmd_tx.send(alarm_cmd).await;

    (StatusCode::OK, Json(ApiResponse::<serde_json::Value>::success(serde_json::json!({
        "structure": sim_resp.analysis,
        "ground_current": ground,
        "ground_all_soils": all_grounds,
        "fem_sample": sim_resp.fem_sample,
        "fem_total_nodes": sim_resp.fem_total_nodes,
        "layer_stresses": sim_resp.layer_stresses
    }))))
}

pub async fn run_structure_analysis(
    State(state): State<SharedState>,
    Path(tower_id): Path<u32>,
    Query(params): Query<AnalysisQuery>,
) -> impl IntoResponse {
    let tower = get_default_tower(tower_id);
    let wind_speed = params.wind_speed.unwrap_or(15.0);
    let tilt_deg = params.tilt_deg.unwrap_or(0.5);

    let (sim_tx, sim_rx) = oneshot::channel();
    let sim_cmd = SimCommand::RunCustomAnalysis {
        tower: tower.clone(),
        wind_speed,
        tilt_deg,
        resp_tx: sim_tx,
    };

    if let Err(e) = state.sim_cmd_tx.send(sim_cmd).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<serde_json::Value>::error(500, format!("仿真服务不可用: {}", e))));
    }

    match sim_rx.await {
        Ok(sim_resp) => {
            let _ = state.db.insert_structure_analysis(&sim_resp.analysis).await;
            let _ = state.analysis_tx.send(sim_resp.analysis.clone());
            (StatusCode::OK, Json(ApiResponse::<serde_json::Value>::success(serde_json::json!({
                "analysis": sim_resp.analysis,
                "fem_sample": sim_resp.fem_sample,
                "fem_total_nodes": sim_resp.fem_total_nodes,
                "layer_stresses": sim_resp.layer_stresses
            }))))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<serde_json::Value>::error(500, format!("仿真失败: {}", e)))),
    }
}

pub async fn run_ground_analysis(
    State(state): State<SharedState>,
    Path(tower_id): Path<u32>,
    Query(params): Query<AnalysisQuery>,
) -> impl IntoResponse {
    let tower = get_default_tower(tower_id);
    let wind_speed = params.wind_speed.unwrap_or(15.0);
    let tilt_deg = params.tilt_deg.unwrap_or(0.5);
    let moisture = params.moisture_pct;

    let (soil_tx, soil_rx) = oneshot::channel();
    let soil_cmd = SoilCommand::AnalyzeAll {
        tower: tower.clone(),
        wind_speed,
        tilt_deg,
        moisture_pct: moisture,
        resp_tx: soil_tx,
    };

    if let Err(e) = state.soil_cmd_tx.send(soil_cmd).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<Vec<GroundAnalysis>>::error(500, format!("土壤服务不可用: {}", e))));
    }

    match soil_rx.await {
        Ok(all_grounds) => {
            let _ = state.db.insert_ground_analysis(&all_grounds).await;
            (StatusCode::OK, Json(ApiResponse::<Vec<GroundAnalysis>>::success(all_grounds)))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<Vec<GroundAnalysis>>::error(500, format!("土壤分析失败: {}", e)))),
    }
}

pub async fn get_alert_events(
    State(state): State<SharedState>,
    Path(tower_id): Path<u32>,
    Query(params): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50);
    match state.db.query_alert_events(tower_id, limit).await {
        Ok(data) => (StatusCode::OK, Json(ApiResponse::success(data))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<Vec<AlertEvent>>::error(500, e.to_string())))
    }
}

pub async fn get_latest_analysis(
    State(state): State<SharedState>,
    Path(tower_id): Path<u32>,
) -> impl IntoResponse {
    match state.db.query_latest_analysis(tower_id).await {
        Ok(Some(analysis)) => (StatusCode::OK, Json(ApiResponse::success(analysis))),
        Ok(None) => (StatusCode::NOT_FOUND, Json(ApiResponse::<StructureAnalysis>::error(404, "暂无分析数据".to_string()))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<StructureAnalysis>::error(500, e.to_string())))
    }
}

pub async fn run_simulation(
    State(state): State<SharedState>,
    Path(tower_id): Path<u32>,
    Query(params): Query<AnalysisQuery>,
) -> impl IntoResponse {
    let tower = get_default_tower(tower_id);
    let wind_speed = params.wind_speed.unwrap_or(25.0);
    let tilt_deg = params.tilt_deg.unwrap_or(2.0);

    let sensor_data = generate_dummy_sensor_data_with_params(&tower, wind_speed, tilt_deg);
    let batch = BatchSensorData::from_sensor_data(&sensor_data, tower_id);

    match state.dtu.process_batch(batch, tower.clone()).await {
        Ok(expanded) => {
            let _ = state.sensor_tx.send(expanded.clone());
            info!("模拟数据生成完成: 塔 {}, 风速 {}", tower_id, wind_speed);

            (StatusCode::OK, Json(ApiResponse::<serde_json::Value>::success(serde_json::json!({
                "status": "simulated",
                "tower_id": tower_id,
                "records": expanded.len(),
                "wind_speed": wind_speed,
                "tilt_deg": tilt_deg,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }))))
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(ApiResponse::<serde_json::Value>::error(400, e))),
    }
}

pub async fn sse_sensor_stream(
    State(state): State<SharedState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.sensor_tx.subscribe();
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
        .filter_map(|result| {
            match result {
                Ok(data) => {
                    let json = serde_json::to_string(&data).unwrap_or_default();
                    Some(Ok(Event::default().event("sensor").data(json)))
                }
                Err(_) => None,
            }
        });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

pub async fn sse_analysis_stream(
    State(state): State<SharedState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.analysis_tx.subscribe();
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
        .filter_map(|result| {
            match result {
                Ok(data) => {
                    let json = serde_json::to_string(&data).unwrap_or_default();
                    Some(Ok(Event::default().event("analysis").data(json)))
                }
                Err(_) => None,
            }
        });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

pub async fn sse_alert_stream(
    State(state): State<SharedState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.alert_tx.subscribe();
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
        .filter_map(|result| {
            match result {
                Ok(data) => {
                    let json = serde_json::to_string(&data).unwrap_or_default();
                    Some(Ok(Event::default().event("alert").data(json)))
                }
                Err(_) => None,
            }
        });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

pub async fn forward_analysis_sse(analysis: &StructureAnalysis) {
    let _ = analysis_tx_global_send(analysis).await;
}

static GLOBAL_ANALYSIS_TX: OnceLock<broadcast::Sender<StructureAnalysis>> = OnceLock::new();

pub fn set_global_analysis_tx(tx: broadcast::Sender<StructureAnalysis>) {
    let _ = GLOBAL_ANALYSIS_TX.set(tx);
}

async fn analysis_tx_global_send(analysis: &StructureAnalysis) {
    if let Some(tx) = GLOBAL_ANALYSIS_TX.get() {
        let _ = tx.send(analysis.clone());
    }
}

pub fn generate_dummy_sensor_data_with_params(
    tower: &TowerMetadata,
    wind_speed: f64,
    tilt_deg: f64,
) -> Vec<SensorData> {
    use chrono::Utc;
    let now = Utc::now();
    let mut result = Vec::with_capacity(tower.total_layers as usize);

    for layer in 1..=tower.total_layers {
        let h_ratio = layer as f64 / tower.total_layers as f64;
        let wind_factor = 1.0 + h_ratio * 0.5;
        let tilt_factor = 1.0 + h_ratio * 0.8;
        let stress_base = 5.0 + h_ratio * 30.0;
        let stress_noise = (rand::random::<f64>() - 0.5) * 2.0;

        result.push(SensorData {
            timestamp: now,
            tower_id: tower.tower_id,
            tower_name: tower.tower_name.clone(),
            layer_id: layer,
            layer_name: format!("L{}", layer),
            stress_x: stress_base * 0.6 + stress_noise,
            stress_y: stress_base * 0.4 + stress_noise * 0.5,
            stress_z: stress_base + stress_noise,
            stress_von_mises: stress_base * 1.1 + stress_noise.abs(),
            tilt_x: tilt_deg * tilt_factor * 0.6,
            tilt_y: tilt_deg * tilt_factor * 0.8,
            tilt_total: tilt_deg * tilt_factor,
            wind_load_x: 0.613 * wind_speed * wind_speed * 1.3 * wind_factor,
            wind_load_y: 0.0,
            wind_speed_mps: wind_speed * wind_factor,
            ground_pressure: 120.0 + h_ratio * 40.0,
            ground_settlement: h_ratio * 15.0,
            soil_type: "loam".to_string(),
            temperature_c: 20.0 + (rand::random::<f64>() - 0.5) * 4.0,
            humidity_pct: 50.0 + (rand::random::<f64>() - 0.5) * 10.0,
            vibration_freq: 2.5 + h_ratio * 0.5,
            vibration_amp: 0.5 + h_ratio * 0.3,
            is_alert: 0,
            alert_level: 0,
        });
    }

    result
}

#[derive(Debug, Deserialize)]
pub struct MoatQuery {
    pub moat_distance: Option<f64>,
    pub moat_depth: Option<f64>,
    pub water_table_depth: Option<f64>,
    pub soil_type: Option<String>,
    pub wind_speed: Option<f64>,
    pub tilt_deg: Option<f64>,
}

fn get_dynasty_for_tower(tower_id: u32) -> String {
    match tower_id {
        1 | 2 => "明朝".to_string(),
        3 => "明朝".to_string(),
        4 => "战国".to_string(),
        5 => "现代".to_string(),
        _ => "未知".to_string(),
    }
}

fn get_category_for_tower(tower_id: u32) -> TowerCategory {
    match tower_id {
        5 => TowerCategory::ModernSteel,
        _ => TowerCategory::AncientWooden,
    }
}

pub async fn compare_dynasty(
    State(_state): State<SharedState>,
    Query(params): Query<AnalysisQuery>,
) -> impl IntoResponse {
    let wind_speed = params.wind_speed.unwrap_or(15.0);
    let tilt_deg = params.tilt_deg.unwrap_or(0.5);
    let analyzer = StabilityAnalyzer::new();
    let air_density = 1.225;
    let cd = 1.3;
    let sf_min = 1.5;

    let mut items: Vec<TowerComparisonItem> = Vec::new();

    for tid in [1u32, 2, 3, 4] {
        let tower = get_default_tower(tid);

        let max_stress = tower.material_strength * 0.6;
        let second_order_coef = analyzer.calculate_second_order_effect(
            &tower, tower.total_weight, tilt_deg.to_radians() * tower.total_height * 0.5,
        );
        let safety_factor = analyzer.calculate_safety_factor(
            &tower, max_stress, tower.material_strength, second_order_coef,
        );
        let wind_resistance_limit = analyzer.calculate_wind_resistance_limit(
            &tower, sf_min, air_density, cd,
        );
        let natural_frequency = analyzer.calculate_natural_frequency(&tower);
        let overturning_ratio = analyzer.calculate_overturning_ratio(
            &tower, wind_speed, tilt_deg, air_density, cd,
        );
        let weight_efficiency = tower.design_load / (tower.total_weight * 9.81);
        let height_to_base_ratio = tower.total_height / tower.base_width;

        items.push(TowerComparisonItem {
            tower_id: tid,
            tower_name: tower.tower_name.clone(),
            dynasty: get_dynasty_for_tower(tid),
            category: get_category_for_tower(tid),
            safety_factor,
            wind_resistance_limit,
            natural_frequency,
            overturning_ratio,
            weight_efficiency,
            height_to_base_ratio,
        });
    }

    let best_sf = items.iter().max_by(|a, b| a.safety_factor.partial_cmp(&b.safety_factor).unwrap()).map(|i| (i.tower_id, i.safety_factor)).unwrap_or((1, 0.0));
    let best_wind = items.iter().max_by(|a, b| a.wind_resistance_limit.partial_cmp(&b.wind_resistance_limit).unwrap()).map(|i| (i.tower_id, i.wind_resistance_limit)).unwrap_or((1, 0.0));
    let best_freq = items.iter().max_by(|a, b| a.natural_frequency.partial_cmp(&b.natural_frequency).unwrap()).map(|i| (i.tower_id, i.natural_frequency)).unwrap_or((1, 0.0));
    let best_ot = items.iter().max_by(|a, b| a.overturning_ratio.partial_cmp(&b.overturning_ratio).unwrap()).map(|i| (i.tower_id, i.overturning_ratio)).unwrap_or((1, 0.0));
    let best_we = items.iter().max_by(|a, b| a.weight_efficiency.partial_cmp(&b.weight_efficiency).unwrap()).map(|i| (i.tower_id, i.weight_efficiency)).unwrap_or((1, 0.0));

    let comparison = DynastyComparison {
        towers: items,
        metrics: ComparisonMetrics {
            best_safety_factor: best_sf,
            best_wind_resistance: best_wind,
            best_frequency: best_freq,
            best_overturning: best_ot,
            best_weight_efficiency: best_we,
        },
    };

    (StatusCode::OK, Json(ApiResponse::success(comparison)))
}

pub async fn compare_cross_era(
    State(_state): State<SharedState>,
    Query(params): Query<AnalysisQuery>,
) -> impl IntoResponse {
    let _wind_speed = params.wind_speed.unwrap_or(15.0);
    let tilt_deg = params.tilt_deg.unwrap_or(0.5);
    let analyzer = StabilityAnalyzer::new();
    let air_density = 1.225;
    let cd = 1.3;
    let sf_min = 1.5;

    let ancient_tower = get_default_tower(1);
    let modern_tower = get_default_tower(5);

    let ancient_max_stress = ancient_tower.material_strength * 0.6;
    let ancient_so = analyzer.calculate_second_order_effect(
        &ancient_tower, ancient_tower.total_weight, tilt_deg.to_radians() * ancient_tower.total_height * 0.5,
    );
    let ancient_sf = analyzer.calculate_safety_factor(&ancient_tower, ancient_max_stress, ancient_tower.material_strength, ancient_so);
    let ancient_wind = analyzer.calculate_wind_resistance_limit(&ancient_tower, sf_min, air_density, cd);
    let ancient_freq = analyzer.calculate_natural_frequency(&ancient_tower);

    let modern_max_stress = modern_tower.material_strength * 0.6;
    let modern_so = analyzer.calculate_second_order_effect(
        &modern_tower, modern_tower.total_weight, tilt_deg.to_radians() * modern_tower.total_height * 0.5,
    );
    let modern_sf = analyzer.calculate_safety_factor(&modern_tower, modern_max_stress, modern_tower.material_strength, modern_so);
    let modern_wind = analyzer.calculate_wind_resistance_limit(&modern_tower, sf_min, air_density, cd);
    let modern_freq = analyzer.calculate_natural_frequency(&modern_tower);

    let ancient_data = EraComparisonData {
        tower_id: 1,
        tower_name: ancient_tower.tower_name.clone(),
        era: "明朝".to_string(),
        material: ancient_tower.material.clone(),
        elastic_modulus: ancient_tower.elastic_modulus,
        material_strength: ancient_tower.material_strength,
        safety_factor: ancient_sf,
        wind_resistance: ancient_wind,
        natural_frequency: ancient_freq,
        weight_per_height: ancient_tower.total_weight / ancient_tower.total_height,
        load_efficiency: ancient_tower.design_load / (ancient_tower.total_weight * 9.81),
    };

    let modern_data = EraComparisonData {
        tower_id: 5,
        tower_name: modern_tower.tower_name.clone(),
        era: "现代".to_string(),
        material: modern_tower.material.clone(),
        elastic_modulus: modern_tower.elastic_modulus,
        material_strength: modern_tower.material_strength,
        safety_factor: modern_sf,
        wind_resistance: modern_wind,
        natural_frequency: modern_freq,
        weight_per_height: modern_tower.total_weight / modern_tower.total_height,
        load_efficiency: modern_tower.design_load / (modern_tower.total_weight * 9.81),
    };

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

    let comparison = CrossEraComparison {
        ancient: ancient_data,
        modern: modern_data,
        ratios,
        analysis,
    };

    (StatusCode::OK, Json(ApiResponse::success(comparison)))
}

pub async fn moat_analysis(
    State(_state): State<SharedState>,
    Path(tower_id): Path<u32>,
    Query(params): Query<MoatQuery>,
) -> impl IntoResponse {
    let tower = get_default_tower(tower_id);
    let moat_distance = params.moat_distance.unwrap_or(3.0);
    let moat_depth = params.moat_depth.unwrap_or(4.0);
    let water_table_depth = params.water_table_depth.unwrap_or(1.5);
    let wind_speed = params.wind_speed.unwrap_or(15.0);
    let tilt_deg = params.tilt_deg.unwrap_or(0.5);

    let soil_type = params.soil_type.as_deref()
        .unwrap_or("loam")
        .parse::<SoilType>()
        .unwrap_or(SoilType::Loam);

    let analyzer = MoatAnalyzer::new();
    let result = analyzer.analyze(
        &tower,
        &soil_type,
        moat_distance,
        moat_depth,
        water_table_depth,
        wind_speed,
        tilt_deg,
    );

    (StatusCode::OK, Json(ApiResponse::success(result)))
}

pub async fn climbing_viewpoints(
    Path(tower_id): Path<u32>,
) -> impl IntoResponse {
    let tower = get_default_tower(tower_id);
    let layer_height = tower.total_height / tower.total_layers as f64;

    let viewpoints: Vec<ClimbingViewpoint> = (1..=tower.total_layers)
        .map(|layer| {
            let layer_y = layer as f64 * layer_height;
            let h_ratio = layer as f64 / tower.total_layers as f64;

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

            ClimbingViewpoint {
                layer_id: layer,
                layer_name: format!("L{}", layer),
                camera_position: [0.0, layer_y, tower.base_depth / 2.0 + 0.5],
                look_at: [0.0, layer_y, tower.base_depth + 20.0],
                description,
                visibility_range_m: visibility,
                strategic_value,
            }
        })
        .collect();

    let battlefield_description = format!(
        "{}高{}m，共{}层，可提供从近距突击到全局指挥的多层次战场视角",
        tower.tower_name, tower.total_height, tower.total_layers,
    );

    let experience = ClimbingExperience {
        tower_id,
        tower_name: tower.tower_name.clone(),
        viewpoints,
        total_height: tower.total_height,
        battlefield_description,
    };

    (StatusCode::OK, Json(ApiResponse::success(experience)))
}

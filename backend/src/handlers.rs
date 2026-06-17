use crate::config::AppConfig;
use crate::database::{ClickHouseClient, get_default_tower};
use crate::dtu_receiver::DtuReceiver;
use crate::models::{
    SensorData, BatchSensorData, TowerMetadata, StructureAnalysis,
    AlertEvent, GroundAnalysis, SoilType, ApiResponse,
};
use crate::mqtt_client::MqttService;
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

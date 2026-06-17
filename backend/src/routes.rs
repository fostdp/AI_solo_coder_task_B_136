use crate::handlers::*;
use crate::handlers::SharedState;
use axum::{
    routing::{get, post},
    Router,
};

pub fn create_routes(state: SharedState) -> Router {
    Router::new()
        .route("/metrics", get(prometheus_metrics))
        .route("/api/health", get(health))
        .route("/api/towers", get(get_all_towers))
        .route("/api/towers/:tower_id", get(get_tower))
        .route("/api/config/towers/:tower_id", get(get_tower_config))
        .route("/api/config/soils", get(get_all_soil_configs))
        .route("/api/towers/:tower_id/sensor", post(receive_sensor_data))
        .route("/api/towers/:tower_id/sensor", get(get_sensor_data))
        .route("/api/towers/:tower_id/analysis", get(get_latest_analysis))
        .route("/api/towers/:tower_id/analysis", post(run_simulation))
        .route("/api/towers/:tower_id/analysis/full", get(run_analysis))
        .route("/api/towers/:tower_id/analysis/structure", get(run_structure_analysis))
        .route("/api/towers/:tower_id/ground", get(run_ground_analysis))
        .route("/api/towers/:tower_id/alerts", get(get_alert_events))
        .route("/api/stream/sensor", get(sse_sensor_stream))
        .route("/api/stream/analysis", get(sse_analysis_stream))
        .route("/api/stream/alerts", get(sse_alert_stream))
        .route("/api/comparison/dynasty", get(compare_dynasty))
        .route("/api/comparison/cross-era", get(compare_cross_era))
        .route("/api/towers/:tower_id/moat", get(moat_analysis))
        .route("/api/towers/:tower_id/climbing", get(climbing_viewpoints))
        .with_state(state)
}

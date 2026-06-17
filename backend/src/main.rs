use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tower_http::compression::CompressionLayer;

use siege_tower_backend::config::AppConfig;
use siege_tower_backend::database::ClickHouseClient;
use siege_tower_backend::dtu_receiver::{DtuReceiver, run_sensor_ingestion_pipeline};
use siege_tower_backend::handlers::{AppState, set_global_analysis_tx};
use siege_tower_backend::mqtt_client::MqttService;
use siege_tower_backend::routes::create_routes;
use siege_tower_backend::soil_analyzer::{SoilAnalyzerService, run_soil_analyzer};
use siege_tower_backend::structural_simulator::{StructuralSimulator, run_structural_simulator};
use siege_tower_backend::alarm_mqtt::{AlarmMqttService, run_alarm_mqtt};
use siege_tower_backend::{SimCommand, SoilCommand, AlarmCommand};

use tracing::{info, error, warn};
use tokio::sync::{broadcast, mpsc, Mutex};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "siege_tower_backend=info,tower_http=info".into()),
        )
        .with_target(true)
        .with_thread_ids(true)
        .init();

    info!("=== 古代临冲吕公车结构仿真与稳定性分析系统 ===");
    info!("正在加载配置...");

    let config = match AppConfig::load() {
        Ok(cfg) => {
            info!("配置加载成功");
            Arc::new(cfg)
        }
        Err(e) => {
            error!("配置加载失败: {:?}", e);
            return Err(anyhow::anyhow!("配置加载失败: {}", e));
        }
    };

    info!("正在初始化 ClickHouse 客户端...");
    let mut db = ClickHouseClient::new(config.clickhouse.clone());
    match db.connect().await {
        Ok(_) => info!("ClickHouse 连接初始化完成"),
        Err(e) => warn!("ClickHouse 连接失败（将以本地模式运行）: {:?}", e),
    }
    let db = Arc::new(db);

    info!("正在初始化 MQTT 客户端...");
    let mut mqtt = MqttService::new(config.mqtt.clone());
    match mqtt.connect().await {
        Ok(_) => info!("MQTT 连接初始化完成"),
        Err(e) => warn!("MQTT 连接失败（将以本地模式运行）: {:?}", e),
    }
    let mqtt = Arc::new(Mutex::new(mqtt));

    info!("正在创建模块间通信通道...");

    let (sim_cmd_tx, sim_cmd_rx) = mpsc::channel::<SimCommand>(64);
    let (soil_cmd_tx, soil_cmd_rx) = mpsc::channel::<SoilCommand>(64);
    let (alarm_cmd_tx, alarm_cmd_rx) = mpsc::channel::<AlarmCommand>(128);

    let (alert_tx, _) = broadcast::channel::<siege_tower_backend::models::AlertEvent>(128);
    let (analysis_tx, _) = broadcast::channel::<siege_tower_backend::models::StructureAnalysis>(64);
    let (sensor_tx, _) = broadcast::channel::<Vec<siege_tower_backend::models::SensorData>>(256);

    set_global_analysis_tx(analysis_tx.clone());

    info!("正在初始化 DTU 接收模块...");
    let (dtu, dtu_broadcast_rx) = DtuReceiver::new(config.clone(), db.clone());

    info!("正在初始化结构仿真模块...");
    let structural_simulator = Arc::new(StructuralSimulator::new(config.clone(), db.clone()));

    info!("正在初始化土壤分析模块...");
    let soil_analyzer = Arc::new(SoilAnalyzerService::new(config.clone(), db.clone()));

    info!("正在初始化告警MQTT模块...");
    let (alarm_mqtt_service, _alarm_broadcast_rx) = AlarmMqttService::new(
        config.clone(),
        db.clone(),
        mqtt.clone(),
    );
    let alarm_mqtt_service = Arc::new(alarm_mqtt_service);

    info!("正在启动各模块异步任务...");

    let sim_handle = tokio::spawn(run_structural_simulator(
        sim_cmd_rx,
        structural_simulator.clone(),
    ));

    let soil_handle = tokio::spawn(run_soil_analyzer(
        soil_cmd_rx,
        soil_analyzer.clone(),
    ));

    let alarm_handle = tokio::spawn(run_alarm_mqtt(
        alarm_cmd_rx,
        alarm_mqtt_service.clone(),
    ));

    let pipeline_handle = tokio::spawn(run_sensor_ingestion_pipeline(
        dtu_broadcast_rx,
        sim_cmd_tx.clone(),
        soil_cmd_tx.clone(),
        alarm_cmd_tx.clone(),
        config.clone(),
    ));

    let state = Arc::new(AppState {
        config: (*config).clone(),
        db: db.clone(),
        dtu,
        mqtt: mqtt.clone(),
        sim_cmd_tx: sim_cmd_tx.clone(),
        soil_cmd_tx: soil_cmd_tx.clone(),
        alarm_cmd_tx: alarm_cmd_tx.clone(),
        alert_tx: alert_tx.clone(),
        analysis_tx: analysis_tx.clone(),
        sensor_tx: sensor_tx.clone(),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .max_age(Duration::from_secs(86400));

    let app = create_routes(state)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    info!("HTTP 服务正在启动: {}", addr);
    info!("API 端点:");
    info!("  GET  /api/health                        - 健康检查");
    info!("  GET  /api/towers                        - 获取所有攻城塔");
    info!("  GET  /api/towers/:id                    - 获取塔详情");
    info!("  GET  /api/config/towers/:id             - 获取塔配置");
    info!("  GET  /api/config/soils                  - 获取土壤配置");
    info!("  POST /api/towers/:id/sensor             - 接收传感器数据");
    info!("  GET  /api/towers/:id/sensor             - 查询传感器数据");
    info!("  GET  /api/towers/:id/analysis           - 获取最新分析");
    info!("  GET  /api/towers/:id/analysis/custom    - 自定义参数分析");
    info!("  GET  /api/towers/:id/ground             - 地面适应性分析");
    info!("  GET  /api/towers/:id/alerts             - 查询告警事件");
    info!("  GET  /api/stream/sensor                 - SSE 传感器数据流");
    info!("  GET  /api/stream/analysis               - SSE 分析结果流");
    info!("  GET  /api/stream/alerts                 - SSE 告警流");
    info!("");
    info!("MQTT Broker: {}:{}", config.mqtt.broker, config.mqtt.port);
    info!("  告警主题: {}", config.mqtt.alert_topic);
    info!("  传感器主题: {}", config.mqtt.sensor_topic);
    info!("");
    info!("模块架构:");
    info!("  [DTU Receiver] → broadcast → [Ingestion Pipeline]");
    info!("                                        ↓");
    info!("                          ┌─────────────┼─────────────┐");
    info!("                          ↓             ↓             ↓");
    info!("                  [Structural]    [Soil Analyzer]  [Alarm MQTT]");
    info!("                  [Simulator]                       ↓");
    info!("                        ↓                          MQTT");
    info!("                      SSE                            +");
    info!("                     Push                           SSE");
    info!("");
    info!("服务已就绪，开始监听连接...");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    info!("正在关闭各模块...");
    sim_handle.abort();
    soil_handle.abort();
    alarm_handle.abort();
    pipeline_handle.abort();

    Ok(())
}

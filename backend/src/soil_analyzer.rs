use crate::config::AppConfig;
use crate::database::ClickHouseClient;
use crate::ground::GroundAnalyzer;
use crate::models::{GroundAnalysis, TowerMetadata};
use crate::SoilCommand;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct SoilAnalyzerService {
    config: Arc<AppConfig>,
    db: Arc<ClickHouseClient>,
    analyzer: GroundAnalyzer,
}

impl SoilAnalyzerService {
    pub fn new(config: Arc<AppConfig>, db: Arc<ClickHouseClient>) -> Self {
        let analyzer = GroundAnalyzer::new();
        Self {
            config,
            db,
            analyzer,
        }
    }

    pub fn analyze_one(
        &self,
        tower: &TowerMetadata,
        soil_type: crate::models::SoilType,
        wind_speed: f64,
        tilt_deg: f64,
        moisture_pct: Option<f64>,
        additional_settlement: Option<f64>,
    ) -> GroundAnalysis {
        self.analyzer.analyze(
            tower,
            soil_type,
            wind_speed,
            tilt_deg,
            additional_settlement,
            moisture_pct,
        )
    }

    pub fn analyze_all(
        &self,
        tower: &TowerMetadata,
        wind_speed: f64,
        tilt_deg: f64,
        moisture_pct: Option<f64>,
    ) -> Vec<GroundAnalysis> {
        self.analyzer.analyze_all_soils(tower, wind_speed, tilt_deg, moisture_pct)
    }
}

pub async fn run_soil_analyzer(
    mut cmd_rx: mpsc::Receiver<SoilCommand>,
    service: Arc<SoilAnalyzerService>,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            SoilCommand::AnalyzeOne {
                tower, soil_type, wind_speed, tilt_deg,
                moisture_pct, additional_settlement, resp_tx,
            } => {
                let result = service.analyze_one(
                    &tower, soil_type, wind_speed, tilt_deg,
                    moisture_pct, additional_settlement,
                );
                let _ = service.db.insert_ground_analysis(&[result.clone()]).await;
                let _ = resp_tx.send(result);
            }
            SoilCommand::AnalyzeAll {
                tower, wind_speed, tilt_deg, moisture_pct, resp_tx,
            } => {
                let results = service.analyze_all(&tower, wind_speed, tilt_deg, moisture_pct);
                let _ = service.db.insert_ground_analysis(&results).await;
                let _ = resp_tx.send(results);
            }
        }
    }
    tracing::info!("土壤分析模块退出");
}

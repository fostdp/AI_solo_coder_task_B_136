pub mod config;
pub mod models;
pub mod database;
pub mod fem;
pub mod stability;
pub mod ground;
pub mod alert;
pub mod mqtt_client;
pub mod handlers;
pub mod routes;
pub mod metrics;
pub mod moat_analyzer;

pub mod structure_comparator;
pub mod era_comparator;
pub mod foundation_analyzer;
pub mod vr_siege_tower;
pub mod fem_executor;

pub mod dtu_receiver;
pub mod structural_simulator;
pub mod soil_analyzer;
pub mod alarm_mqtt;

use serde::{Serialize, Deserialize};
use tokio::sync::oneshot;

use crate::models::{
    BatchSensorData, SensorData, StructureAnalysis,
    GroundAnalysis, AlertEvent, TowerMetadata,
};

#[derive(Debug, Clone)]
pub enum SensorBroadcast {
    DataReceived {
        batch: BatchSensorData,
        expanded: Vec<SensorData>,
        tower: TowerMetadata,
    },
}

#[derive(Debug)]
pub enum SimCommand {
    RunFullAnalysis {
        tower: TowerMetadata,
        batch: BatchSensorData,
        sensor_data: Vec<SensorData>,
        resp_tx: oneshot::Sender<SimResponse>,
    },
    RunCustomAnalysis {
        tower: TowerMetadata,
        wind_speed: f64,
        tilt_deg: f64,
        resp_tx: oneshot::Sender<SimResponse>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SimResponse {
    pub analysis: StructureAnalysis,
    pub fem_sample: Option<Vec<crate::models::FEMNodeResult>>,
    pub fem_total_nodes: usize,
    pub layer_stresses: Vec<(u8, f64, f64, f64)>,
}

#[derive(Debug)]
pub enum SoilCommand {
    AnalyzeOne {
        tower: TowerMetadata,
        soil_type: crate::models::SoilType,
        wind_speed: f64,
        tilt_deg: f64,
        moisture_pct: Option<f64>,
        additional_settlement: Option<f64>,
        resp_tx: oneshot::Sender<GroundAnalysis>,
    },
    AnalyzeAll {
        tower: TowerMetadata,
        wind_speed: f64,
        tilt_deg: f64,
        moisture_pct: Option<f64>,
        resp_tx: oneshot::Sender<Vec<GroundAnalysis>>,
    },
}

#[derive(Debug)]
pub enum AlarmCommand {
    Evaluate {
        tower: TowerMetadata,
        sensor_data: Vec<SensorData>,
        analysis: StructureAnalysis,
        resp_tx: oneshot::Sender<Vec<AlertEvent>>,
    },
}

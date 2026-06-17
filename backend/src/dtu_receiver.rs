use crate::config::AppConfig;
use crate::database::ClickHouseClient;
use crate::models::{BatchSensorData, SensorData, TowerMetadata};
use crate::SensorBroadcast;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

pub struct DtuReceiver {
    config: Arc<AppConfig>,
    db: Arc<ClickHouseClient>,
    broadcast_tx: broadcast::Sender<SensorBroadcast>,
}

impl DtuReceiver {
    pub fn new(
        config: Arc<AppConfig>,
        db: Arc<ClickHouseClient>,
    ) -> (Self, broadcast::Receiver<SensorBroadcast>) {
        let (broadcast_tx, rx) = broadcast::channel(256);
        (
            Self {
                config,
                db,
                broadcast_tx,
            },
            rx,
        )
    }

    pub fn broadcast_sender(&self) -> broadcast::Sender<SensorBroadcast> {
        self.broadcast_tx.clone()
    }

    pub fn validate(
        &self,
        batch: &BatchSensorData,
        tower: &TowerMetadata,
    ) -> Result<(), String> {
        if batch.tower_id != tower.tower_id {
            return Err(format!(
                "塔ID不匹配: 批次={}, 塔配置={}",
                batch.tower_id, tower.tower_id
            ));
        }

        let expected_layers = tower.total_layers as usize;
        if batch.layers.len() != expected_layers {
            return Err(format!(
                "层数不匹配: 批次={}, 期望={}",
                batch.layers.len(),
                expected_layers
            ));
        }

        for (i, layer) in batch.layers.iter().enumerate() {
            if layer.layer_id != (i as u8) + 1 {
                return Err(format!(
                    "层ID不连续: 第{}层报告 layer_id={}",
                    i, layer.layer_id
                ));
            }
            if layer.stress_von_mises < 0.0 || layer.stress_von_mises > 200.0 {
                return Err(format!(
                    "L{} 应力值异常: {} MPa (范围 0-200)",
                    layer.layer_id, layer.stress_von_mises
                ));
            }
            if layer.tilt_total < 0.0 || layer.tilt_total > 30.0 {
                return Err(format!(
                    "L{} 倾斜值异常: {}° (范围 0-30)",
                    layer.layer_id, layer.tilt_total
                ));
            }
        }

        if let Some(env) = &batch.environment {
            if env.temperature_c < -40.0 || env.temperature_c > 60.0 {
                return Err(format!("温度异常: {}°C", env.temperature_c));
            }
            if env.wind_speed_mps < 0.0 || env.wind_speed_mps > 80.0 {
                return Err(format!("风速异常: {} m/s", env.wind_speed_mps));
            }
        }

        Ok(())
    }

    pub fn expand_batch(
        &self,
        batch: &BatchSensorData,
        tower: &TowerMetadata,
    ) -> Vec<SensorData> {
        let now = Utc::now();
        let mut result = Vec::with_capacity(batch.layers.len());

        let (wind_speed, ground_pressure, temperature, humidity) =
            if let Some(env) = &batch.environment {
                (
                    env.wind_speed_mps,
                    env.ground_pressure_kpa,
                    env.temperature_c,
                    env.humidity_pct,
                )
            } else {
                (0.0, 0.0, 20.0, 50.0)
            };

        for layer in &batch.layers {
            let layer_name = format!("L{}", layer.layer_id);
            let wind_dir_rad = batch.environment.as_ref().map(|e| e.wind_direction_deg.to_radians()).unwrap_or(0.0);
            let wind_load = 0.613 * wind_speed * wind_speed * 1.3;
            result.push(SensorData {
                timestamp: now,
                tower_id: batch.tower_id,
                tower_name: tower.tower_name.clone(),
                layer_id: layer.layer_id,
                layer_name,
                stress_x: layer.stress_x,
                stress_y: layer.stress_y,
                stress_z: layer.stress_z,
                stress_von_mises: layer.stress_von_mises,
                tilt_x: layer.tilt_x,
                tilt_y: layer.tilt_y,
                tilt_total: layer.tilt_total,
                wind_load_x: wind_load * wind_dir_rad.cos(),
                wind_load_y: wind_load * wind_dir_rad.sin(),
                wind_speed_mps: wind_speed,
                ground_pressure,
                ground_settlement: 0.0,
                soil_type: "loam".to_string(),
                temperature_c: temperature,
                humidity_pct: humidity,
                vibration_freq: layer.vibration_freq_hz,
                vibration_amp: layer.vibration_amplitude,
                is_alert: 0,
                alert_level: 0,
            });
        }

        result
    }

    pub async fn process_batch(
        &self,
        batch: BatchSensorData,
        tower: TowerMetadata,
    ) -> Result<Vec<SensorData>, String> {
        self.validate(&batch, &tower)?;

        let expanded = self.expand_batch(&batch, &tower);

        let _ = self.db.insert_sensor_data(&expanded).await;

        let broadcast_msg = SensorBroadcast::DataReceived {
            batch: batch.clone(),
            expanded: expanded.clone(),
            tower: tower.clone(),
        };

        let _ = self.broadcast_tx.send(broadcast_msg);

        Ok(expanded)
    }
}

pub async fn run_sensor_ingestion_pipeline(
    mut broadcast_rx: broadcast::Receiver<SensorBroadcast>,
    sim_cmd_tx: mpsc::Sender<crate::SimCommand>,
    soil_cmd_tx: mpsc::Sender<crate::SoilCommand>,
    alarm_cmd_tx: mpsc::Sender<crate::AlarmCommand>,
    config: Arc<AppConfig>,
) {
    while let Ok(msg) = broadcast_rx.recv().await {
        match msg {
            SensorBroadcast::DataReceived { batch, expanded, tower } => {
                let (sim_tx, sim_rx) = tokio::sync::oneshot::channel();
                let sim_cmd = crate::SimCommand::RunFullAnalysis {
                    tower: tower.clone(),
                    batch,
                    sensor_data: expanded.clone(),
                    resp_tx: sim_tx,
                };

                if let Err(e) = sim_cmd_tx.send(sim_cmd).await {
                    tracing::warn!("向结构仿真模块发送命令失败: {}", e);
                    continue;
                }

                let config_clone = config.clone();
                let tower_clone = tower.clone();
                let expanded_clone = expanded.clone();
                let alarm_cmd_tx_clone = alarm_cmd_tx.clone();

                tokio::spawn(async move {
                    match sim_rx.await {
                        Ok(sim_resp) => {
                            let _ = crate::handlers::forward_analysis_sse(&sim_resp.analysis).await;

                            let (alarm_tx, alarm_rx) = tokio::sync::oneshot::channel();
                            let alarm_cmd = crate::AlarmCommand::Evaluate {
                                tower: tower_clone,
                                sensor_data: expanded_clone,
                                analysis: sim_resp.analysis.clone(),
                                resp_tx: alarm_tx,
                            };

                            if let Err(e) = alarm_cmd_tx_clone.send(alarm_cmd).await {
                                tracing::warn!("向告警模块发送命令失败: {}", e);
                            }
                        }
                        Err(e) => {
                            tracing::warn!("结构仿真响应失败: {}", e);
                        }
                    }
                });
            }
        }
    }
}

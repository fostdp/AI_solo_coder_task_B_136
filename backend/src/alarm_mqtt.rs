use crate::alert::AlertManager;
use crate::config::AppConfig;
use crate::database::ClickHouseClient;
use crate::mqtt_client::MqttService;
use crate::models::{AlertEvent, SensorData, StructureAnalysis, TowerMetadata};
use crate::AlarmCommand;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};

pub struct AlarmMqttService {
    config: Arc<AppConfig>,
    db: Arc<ClickHouseClient>,
    alert_manager: Mutex<AlertManager>,
    mqtt: Arc<Mutex<MqttService>>,
    alert_broadcast: broadcast::Sender<AlertEvent>,
}

impl AlarmMqttService {
    pub fn new(
        config: Arc<AppConfig>,
        db: Arc<ClickHouseClient>,
        mqtt: Arc<Mutex<MqttService>>,
    ) -> (Self, broadcast::Receiver<AlertEvent>) {
        let alert_manager = AlertManager::new();
        let (alert_broadcast, rx) = broadcast::channel(128);

        (
            Self {
                config,
                db,
                alert_manager: Mutex::new(alert_manager),
                mqtt,
                alert_broadcast,
            },
            rx,
        )
    }

    pub async fn evaluate(
        &self,
        tower: &TowerMetadata,
        sensor_data: &[SensorData],
        analysis: &StructureAnalysis,
    ) -> Vec<AlertEvent> {
        let mut manager = self.alert_manager.lock().await;
        let mut all_alerts = Vec::new();

        let sensor_alerts = manager.check_sensor_alerts(
            tower,
            sensor_data,
            &self.config.tower.alert_thresholds,
        );

        let structure_alerts = manager.check_structure_alerts(
            analysis,
            &self.config.tower.alert_thresholds,
        );

        all_alerts.extend(sensor_alerts);
        all_alerts.extend(structure_alerts);

        if !all_alerts.is_empty() {
            let _ = self.db.insert_alert_events(&all_alerts).await;

            for alert in &all_alerts {
                let _ = self.alert_broadcast.send(alert.clone());
            }

            if let Ok(mut mqtt) = self.mqtt.try_lock() {
                for alert in &all_alerts {
                    let _ = mqtt.publish_alert(alert).await;
                }
            }
        }

        all_alerts
    }
}

pub async fn run_alarm_mqtt(
    mut cmd_rx: mpsc::Receiver<AlarmCommand>,
    service: Arc<AlarmMqttService>,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            AlarmCommand::Evaluate { tower, sensor_data, analysis, resp_tx } => {
                let alerts = service.evaluate(&tower, &sensor_data, &analysis).await;
                let _ = resp_tx.send(alerts);
            }
        }
    }
    tracing::info!("告警MQTT模块退出");
}

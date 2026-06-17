use crate::config::MqttConfig;
use crate::models::AlertEvent;
use rumqttc::{AsyncClient, MqttOptions, QoS, Event, Packet};
use std::sync::Arc;
use parking_lot::Mutex;
use serde_json;
use tracing::{info, error, warn};

pub struct MqttService {
    client: Option<AsyncClient>,
    config: MqttConfig,
    connected: Arc<Mutex<bool>>,
}

impl MqttService {
    pub fn new(config: MqttConfig) -> Self {
        Self {
            client: None,
            config,
            connected: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn connect(&mut self) -> anyhow::Result<()> {
        let mut mqtt_options = MqttOptions::new(
            &self.config.client_id,
            &self.config.broker,
            self.config.port,
        );
        mqtt_options.set_keep_alive(std::time::Duration::from_secs(60));
        mqtt_options.set_clean_session(true);

        if let (Some(ref user), Some(ref pass)) = (&self.config.username, &self.config.password) {
            mqtt_options.set_credentials(user, pass);
        }

        let (client, mut eventloop) = AsyncClient::new(mqtt_options, 256);
        self.client = Some(client);
        let connected = self.connected.clone();

        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(Event::Incoming(Packet::ConnAck(_))) => {
                        info!("MQTT 连接成功");
                        *connected.lock() = true;
                    }
                    Ok(Event::Incoming(Packet::Disconnect)) => {
                        warn!("MQTT 断开连接");
                        *connected.lock() = false;
                        break;
                    }
                    Err(e) => {
                        error!("MQTT 错误: {:?}", e);
                        *connected.lock() = false;
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                    _ => {}
                }
            }
        });

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        *self.connected.lock()
    }

    pub async fn publish_alert(&self, alert: &AlertEvent) -> anyhow::Result<()> {
        if let Some(ref client) = self.client {
            let payload = serde_json::to_string(alert)?;
            let topic = format!("{}/{}", self.config.alert_topic, alert.tower_id);
            client.publish(topic, QoS::AtLeastOnce, false, payload).await?;
            info!("告警已推送 MQTT: tower={} type={}", alert.tower_id, alert.alert_type);
        }
        Ok(())
    }

    pub async fn publish_sensor_data(&self, tower_id: u32, data: &serde_json::Value) -> anyhow::Result<()> {
        if let Some(ref client) = self.client {
            let payload = serde_json::to_string(data)?;
            let topic = format!("{}/{}", self.config.sensor_topic, tower_id);
            client.publish(topic, QoS::AtMostOnce, false, payload).await?;
        }
        Ok(())
    }

    pub async fn publish_analysis_result(&self, tower_id: u32, result: &serde_json::Value) -> anyhow::Result<()> {
        if let Some(ref client) = self.client {
            let payload = serde_json::to_string(result)?;
            let topic = format!("siege/tower/analysis/{}", tower_id);
            client.publish(topic, QoS::AtLeastOnce, false, payload).await?;
        }
        Ok(())
    }

    pub async fn broadcast_alerts(&self, alerts: &[AlertEvent]) -> Vec<anyhow::Result<()>> {
        let mut results = Vec::new();
        for alert in alerts {
            results.push(self.publish_alert(alert).await);
        }
        results
    }
}

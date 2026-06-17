use crate::config::ClickHouseConfig;
use crate::models::{AlertEvent, FEMNodeResult, GroundAnalysis, SensorData, StructureAnalysis, TowerMetadata};
use anyhow::{Result, Context};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Serialize;

pub struct ClickHouseClient {
    config: ClickHouseConfig,
    http: Client,
    base_url: String,
}

impl ClickHouseClient {
    pub fn new(config: ClickHouseConfig) -> Self {
        let base_url = if config.user.is_empty() || config.password.is_empty() {
            config.url.clone()
        } else {
            let scheme = if config.url.starts_with("https://") { "https" } else { "http" };
            let host_part = config.url.trim_start_matches("http://").trim_start_matches("https://");
            format!("{}://{}:{}@{}", scheme, urlencode(&config.user), urlencode(&config.password), host_part)
        };
        Self {
            config,
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| Client::new()),
            base_url,
        }
    }

    pub async fn connect(&mut self) -> Result<()> {
        Ok(())
    }

    async fn insert_rows<T: Serialize>(&self, table: &str, rows: &[T]) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        let json_lines: Vec<String> = rows.iter()
            .map(|r| serde_json::to_string(r).unwrap_or_default())
            .filter(|s| !s.is_empty())
            .collect();
        if json_lines.is_empty() {
            return Ok(());
        }
        let body = json_lines.join("\n");
        let query = format!(
            "INSERT INTO {}.{} FORMAT JSONEachRow",
            self.config.database, table
        );
        let url = format!("{}/?query={}", self.base_url, urlencode(&query));
        let resp = self.http.post(&url)
            .body(body)
            .send()
            .await
            .context("ClickHouse HTTP request failed")?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("ClickHouse insert {} failed: {} - {}", table, status, text);
        }
        Ok(())
    }

    pub async fn insert_sensor_data(&self, data: &[SensorData]) -> Result<()> {
        self.insert_rows("sensor_data", data).await
    }

    pub async fn insert_structure_analysis(&self, analysis: &StructureAnalysis) -> Result<()> {
        self.insert_rows("structure_analysis", &[analysis]).await
    }

    pub async fn insert_alert_events(&self, events: &[AlertEvent]) -> Result<()> {
        self.insert_rows("alert_events", events).await
    }

    pub async fn insert_ground_analysis(&self, analysis: &[GroundAnalysis]) -> Result<()> {
        self.insert_rows("ground_analysis", analysis).await
    }

    pub async fn insert_fem_results(&self, results: &[FEMNodeResult]) -> Result<()> {
        self.insert_rows("fem_node_results", results).await
    }

    pub async fn get_tower_metadata(&self, tower_id: u32) -> Result<Option<TowerMetadata>> {
        Ok(Some(get_default_tower(tower_id)))
    }

    pub async fn query_recent_sensor_data(
        &self,
        tower_id: u32,
        limit: u64,
    ) -> Result<Vec<SensorData>> {
        let _ = (tower_id, limit);
        Ok(Vec::new())
    }

    pub async fn query_sensor_by_time(
        &self,
        tower_id: u32,
        layer_id: Option<u8>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<SensorData>> {
        let _ = (tower_id, layer_id, start, end);
        Ok(Vec::new())
    }

    pub async fn query_latest_analysis(&self, tower_id: u32) -> Result<Option<StructureAnalysis>> {
        let _ = tower_id;
        Ok(None)
    }

    pub async fn query_alert_events(
        &self,
        tower_id: u32,
        limit: u64,
    ) -> Result<Vec<AlertEvent>> {
        let _ = (tower_id, limit);
        Ok(Vec::new())
    }

    pub async fn query_all_towers(&self) -> Result<Vec<TowerMetadata>> {
        Ok(vec![get_default_tower(1), get_default_tower(2), get_default_tower(3), get_default_tower(4), get_default_tower(5)])
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

pub fn get_default_tower(tower_id: u32) -> TowerMetadata {
    match tower_id {
        2 => TowerMetadata {
            tower_id: 2,
            tower_name: "临冲吕公车-二号".to_string(),
            build_date: "1452-07-22".to_string(),
            material: "柏木+楠木".to_string(),
            total_height: 21.0,
            total_layers: 6,
            base_width: 6.8,
            base_depth: 5.2,
            total_weight: 36.8,
            design_load: 1020.0,
            design_wind_speed: 40.0,
            material_strength: 52.0,
            elastic_modulus: 13500.0,
            poisson_ratio: 0.36,
        },
        3 => TowerMetadata {
            tower_id: 3,
            tower_name: "云梯车".to_string(),
            build_date: "1368-01-01".to_string(),
            material: "松木+竹".to_string(),
            total_height: 12.0,
            total_layers: 3,
            base_width: 3.5,
            base_depth: 2.8,
            total_weight: 8.5,
            design_load: 280.0,
            design_wind_speed: 25.0,
            material_strength: 35.0,
            elastic_modulus: 9000.0,
            poisson_ratio: 0.40,
        },
        4 => TowerMetadata {
            tower_id: 4,
            tower_name: "冲车".to_string(),
            build_date: "0350-01-01".to_string(),
            material: "硬木+铁箍".to_string(),
            total_height: 5.5,
            total_layers: 2,
            base_width: 4.2,
            base_depth: 3.0,
            total_weight: 15.0,
            design_load: 450.0,
            design_wind_speed: 20.0,
            material_strength: 50.0,
            elastic_modulus: 11000.0,
            poisson_ratio: 0.37,
        },
        5 => TowerMetadata {
            tower_id: 5,
            tower_name: "现代塔吊".to_string(),
            build_date: "2024-01-01".to_string(),
            material: "Q345B钢材".to_string(),
            total_height: 60.0,
            total_layers: 12,
            base_width: 8.0,
            base_depth: 8.0,
            total_weight: 85.0,
            design_load: 6000.0,
            design_wind_speed: 55.0,
            material_strength: 345.0,
            elastic_modulus: 206000.0,
            poisson_ratio: 0.30,
        },
        _ => TowerMetadata {
            tower_id: 1,
            tower_name: "临冲吕公车-一号".to_string(),
            build_date: "1450-03-15".to_string(),
            material: "松木+铁木".to_string(),
            total_height: 18.5,
            total_layers: 5,
            base_width: 6.2,
            base_depth: 4.8,
            total_weight: 28.5,
            design_load: 850.0,
            design_wind_speed: 35.0,
            material_strength: 45.0,
            elastic_modulus: 12000.0,
            poisson_ratio: 0.38,
        },
    }
}

impl Default for ClickHouseClient {
    fn default() -> Self {
        Self::new(ClickHouseConfig {
            url: "http://localhost:8123".to_string(),
            user: "default".to_string(),
            password: "".to_string(),
            database: "siege_tower".to_string(),
        })
    }
}

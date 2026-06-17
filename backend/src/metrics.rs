
use once_cell::sync::Lazy;
use prometheus::{
    IntCounter, IntCounterVec, IntGauge, IntGaugeVec,
    Histogram, HistogramVec, HistogramOpts, Opts, Registry,
    CounterVec, GaugeVec,
};
use std::time::Instant;

pub static REGISTRY: Lazy<Registry> = Lazy::new(|| {
    let r = Registry::new_custom(
        Some("siege_tower".to_string()),
        Some(vec!["system".to_string()])
    ).unwrap_or_else(|_| Registry::new());

    r.register(Box::new(HTTP_REQUESTS_TOTAL.clone())).unwrap();
    r.register(Box::new(HTTP_REQUEST_DURATION.clone())).unwrap();
    r.register(Box::new(HTTP_ERRORS_TOTAL.clone())).unwrap();

    r.register(Box::new(SENSOR_DATA_RECEIVED.clone())).unwrap();
    r.register(Box::new(SENSOR_DATA_VALID.clone())).unwrap();
    r.register(Box::new(SENSOR_DATA_INVALID.clone())).unwrap();
    r.register(Box::new(SENSOR_DATA_BYTES.clone())).unwrap();

    r.register(Box::new(FEM_ANALYSIS_TOTAL.clone())).unwrap();
    r.register(Box::new(FEM_ANALYSIS_DURATION.clone())).unwrap();
    r.register(Box::new(FEM_ANALYSIS_NODES.clone())).unwrap();
    r.register(Box::new(FEM_ANALYSIS_ERRORS.clone())).unwrap();

    r.register(Box::new(SOIL_ANALYSIS_TOTAL.clone())).unwrap();
    r.register(Box::new(SOIL_ANALYSIS_DURATION.clone())).unwrap();

    r.register(Box::new(ALERTS_TRIGGERED.clone())).unwrap();
    r.register(Box::new(ALERTS_MQTT_SENT.clone())).unwrap();
    r.register(Box::new(ALERTS_MQTT_ERRORS.clone())).unwrap();

    r.register(Box::new(STRUCTURE_SAFETY_FACTOR.clone())).unwrap();
    r.register(Box::new(STRUCTURE_STABLE.clone())).unwrap();
    r.register(Box::new(SOIL_BEARING_RATIO.clone())).unwrap();

    r.register(Box::new(CLICKHOUSE_INSERTS.clone())).unwrap();
    r.register(Box::new(CLICKHOUSE_ERRORS.clone())).unwrap();

    r.register(Box::new(ACTIVE_CONNECTIONS_SSE.clone())).unwrap();
    r.register(Box::new(MODULE_CHANNEL_DEPTH.clone())).unwrap();

    r
});

pub static HTTP_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("http_requests_total", "Total number of HTTP requests")
            .namespace("siege_tower")
            .subsystem("http"),
        &["method", "endpoint", "status"]
    ).unwrap()
});

pub static HTTP_REQUEST_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    HistogramVec::new(
        HistogramOpts::new("http_request_duration_seconds", "HTTP request duration in seconds")
            .namespace("siege_tower")
            .subsystem("http")
            .buckets(vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0]),
        &["method", "endpoint"]
    ).unwrap()
});

pub static HTTP_ERRORS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("http_errors_total", "Total number of HTTP errors")
            .namespace("siege_tower")
            .subsystem("http"),
        &["method", "endpoint", "error_type"]
    ).unwrap()
});

pub static SENSOR_DATA_RECEIVED: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("sensor_data_received_total", "Total sensor data records received")
            .namespace("siege_tower")
            .subsystem("sensor"),
        &["tower_id", "source"]
    ).unwrap()
});

pub static SENSOR_DATA_VALID: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("sensor_data_valid_total", "Total valid sensor data records")
            .namespace("siege_tower")
            .subsystem("sensor"),
        &["tower_id"]
    ).unwrap()
});

pub static SENSOR_DATA_INVALID: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("sensor_data_invalid_total", "Total invalid sensor data records")
            .namespace("siege_tower")
            .subsystem("sensor"),
        &["tower_id", "reason"]
    ).unwrap()
});

pub static SENSOR_DATA_BYTES: Lazy<IntCounter> = Lazy::new(|| {
    IntCounter::with_opts(
        Opts::new("sensor_data_bytes_total", "Total bytes of sensor data received")
            .namespace("siege_tower")
            .subsystem("sensor")
    ).unwrap()
});

pub static FEM_ANALYSIS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("fem_analysis_total", "Total FEM analyses executed")
            .namespace("siege_tower")
            .subsystem("fem"),
        &["tower_id", "type"]
    ).unwrap()
});

pub static FEM_ANALYSIS_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    HistogramVec::new(
        HistogramOpts::new("fem_analysis_duration_seconds", "FEM analysis duration in seconds")
            .namespace("siege_tower")
            .subsystem("fem")
            .buckets(vec![0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 20.0, 30.0, 45.0, 60.0, 90.0, 120.0]),
        &["tower_id", "type"]
    ).unwrap()
});

pub static FEM_ANALYSIS_NODES: Lazy<IntGaugeVec> = Lazy::new(|| {
    IntGaugeVec::new(
        Opts::new("fem_analysis_nodes", "Number of nodes in last FEM analysis")
            .namespace("siege_tower")
            .subsystem("fem"),
        &["tower_id"]
    ).unwrap()
});

pub static FEM_ANALYSIS_ERRORS: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("fem_analysis_errors_total", "Total FEM analysis errors")
            .namespace("siege_tower")
            .subsystem("fem"),
        &["tower_id", "error_type"]
    ).unwrap()
});

pub static SOIL_ANALYSIS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("soil_analysis_total", "Total soil analyses executed")
            .namespace("siege_tower")
            .subsystem("soil"),
        &["tower_id", "soil_type"]
    ).unwrap()
});

pub static SOIL_ANALYSIS_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    HistogramVec::new(
        HistogramOpts::new("soil_analysis_duration_seconds", "Soil analysis duration in seconds")
            .namespace("siege_tower")
            .subsystem("soil")
            .buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0]),
        &["tower_id", "soil_type"]
    ).unwrap()
});

pub static ALERTS_TRIGGERED: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("alerts_triggered_total", "Total alerts triggered")
            .namespace("siege_tower")
            .subsystem("alarm"),
        &["tower_id", "alert_type", "level"]
    ).unwrap()
});

pub static ALERTS_MQTT_SENT: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("alerts_mqtt_sent_total", "Total alerts pushed via MQTT")
            .namespace("siege_tower")
            .subsystem("alarm"),
        &["tower_id", "alert_type"]
    ).unwrap()
});

pub static ALERTS_MQTT_ERRORS: Lazy<IntCounter> = Lazy::new(|| {
    IntCounter::with_opts(
        Opts::new("alerts_mqtt_errors_total", "Total MQTT push errors")
            .namespace("siege_tower")
            .subsystem("alarm")
    ).unwrap()
});

pub static STRUCTURE_SAFETY_FACTOR: Lazy<GaugeVec> = Lazy::new(|| {
    GaugeVec::new(
        Opts::new("structure_safety_factor", "Current structural safety factor")
            .namespace("siege_tower")
            .subsystem("structure"),
        &["tower_id"]
    ).unwrap()
});

pub static STRUCTURE_STABLE: Lazy<IntGaugeVec> = Lazy::new(|| {
    IntGaugeVec::new(
        Opts::new("structure_stable", "Structure stability status (1=stable, 0=unstable)")
            .namespace("siege_tower")
            .subsystem("structure"),
        &["tower_id"]
    ).unwrap()
});

pub static SOIL_BEARING_RATIO: Lazy<GaugeVec> = Lazy::new(|| {
    GaugeVec::new(
        Opts::new("soil_bearing_ratio", "Soil bearing capacity utilization ratio (0-1)")
            .namespace("siege_tower")
            .subsystem("soil"),
        &["tower_id", "soil_type"]
    ).unwrap()
});

pub static CLICKHOUSE_INSERTS: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("clickhouse_inserts_total", "Total rows inserted into ClickHouse")
            .namespace("siege_tower")
            .subsystem("database"),
        &["table"]
    ).unwrap()
});

pub static CLICKHOUSE_ERRORS: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("clickhouse_errors_total", "Total ClickHouse operation errors")
            .namespace("siege_tower")
            .subsystem("database"),
        &["operation", "table"]
    ).unwrap()
});

pub static ACTIVE_CONNECTIONS_SSE: Lazy<IntGaugeVec> = Lazy::new(|| {
    IntGaugeVec::new(
        Opts::new("active_sse_connections", "Number of active SSE connections")
            .namespace("siege_tower")
            .subsystem("sse"),
        &["stream_type"]
    ).unwrap()
});

pub static MODULE_CHANNEL_DEPTH: Lazy<IntGaugeVec> = Lazy::new(|| {
    IntGaugeVec::new(
        Opts::new("module_channel_depth", "Current depth of inter-module mpsc channels")
            .namespace("siege_tower")
            .subsystem("module"),
        &["channel"]
    ).unwrap()
});

pub fn record_http(method: &str, endpoint: &str, status: u16, started: Instant) {
    let duration = started.elapsed().as_secs_f64();
    HTTP_REQUESTS_TOTAL
        .with_label_values(&[method, endpoint, &status.to_string()])
        .inc();
    HTTP_REQUEST_DURATION
        .with_label_values(&[method, endpoint])
        .observe(duration);
}

pub fn record_fem(tower_id: u32, analysis_type: &str, started: Instant, nodes: usize, success: bool) {
    let tower = tower_id.to_string();
    let duration = started.elapsed().as_secs_f64();
    FEM_ANALYSIS_TOTAL
        .with_label_values(&[&tower, analysis_type])
        .inc();
    FEM_ANALYSIS_DURATION
        .with_label_values(&[&tower, analysis_type])
        .observe(duration);
    if success {
        FEM_ANALYSIS_NODES
            .with_label_values(&[&tower])
            .set(nodes as i64);
    } else {
        FEM_ANALYSIS_ERRORS
            .with_label_values(&[&tower, "fem_panic"])
            .inc();
    }
}

pub fn record_alert(tower_id: u32, alert_type: &str, level: &str) {
    ALERTS_TRIGGERED
        .with_label_values(&[&tower_id.to_string(), alert_type, level])
        .inc();
}

pub fn gather_metrics() -> String {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();
    let metric_families = REGISTRY.gather();
    let mut default_metrics = prometheus::gather();
    metric_families.iter().for_each(|m| default_metrics.push(m.clone()));
    encoder.encode(&default_metrics, &mut buffer).unwrap_or_default();
    String::from_utf8(buffer).unwrap_or_default()
}

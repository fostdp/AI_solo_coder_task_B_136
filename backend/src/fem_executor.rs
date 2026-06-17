use crate::fem::FEMAnalysis;
use crate::models::{FEMNodeResult, TowerMetadata};
use chrono::Utc;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Semaphore};
use tokio::task;
use tracing::{error, info};

pub type FEMJobId = u64;

static JOB_COUNTER: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Clone)]
pub struct FEMRequest {
    pub tower: TowerMetadata,
    pub wind_speed: f64,
    pub tilt_deg: f64,
    pub gravity: f64,
    pub air_density: f64,
    pub wind_drag_coefficient: f64,
}

#[derive(Debug, Clone)]
pub struct FEMResponse {
    pub node_results: Vec<FEMNodeResult>,
    pub layer_stresses: Vec<(u8, f64, f64, f64)>,
    pub total_nodes: usize,
    pub job_id: FEMJobId,
}

#[derive(Debug)]
enum PoolMessage {
    Job {
        request: FEMRequest,
        resp_tx: oneshot::Sender<Option<FEMResponse>>,
    },
    Shutdown,
}

pub struct FEMExecutor {
    sender: mpsc::Sender<PoolMessage>,
    semaphore: Arc<Semaphore>,
    pool_size: usize,
}

impl FEMExecutor {
    pub fn new(pool_size: usize) -> Self {
        let (sender, receiver) = mpsc::channel::<PoolMessage>(pool_size * 4);
        let semaphore = Arc::new(Semaphore::new(pool_size));

        tokio::spawn(Self::dispatch_loop(pool_size, receiver, semaphore.clone()));

        Self { sender, semaphore, pool_size }
    }

    async fn dispatch_loop(
        pool_size: usize,
        mut receiver: mpsc::Receiver<PoolMessage>,
        semaphore: Arc<Semaphore>,
    ) {
        info!("[FEM-Executor] 线程池启动，pool_size={}", pool_size);

        while let Some(msg) = receiver.recv().await {
            match msg {
                PoolMessage::Job { request, resp_tx } => {
                    let permit = match semaphore.clone().acquire_owned().await {
                        Ok(p) => p,
                        Err(e) => {
                            error!("[FEM-Executor] 获取信号量失败: {}", e);
                            let _ = resp_tx.send(None);
                            continue;
                        }
                    };

                    let job_id = JOB_COUNTER.fetch_add(1, Ordering::Relaxed) as u64;
                    let req = request.clone();

                    task::spawn_blocking(move || {
                        let _permit_guard = permit;
                        Self::run_job_blocking(req, job_id, resp_tx);
                    });
                }
                PoolMessage::Shutdown => {
                    info!("[FEM-Executor] 收到关闭信号");
                    break;
                }
            }
        }

        info!("[FEM-Executor] 调度循环结束");
    }

    fn run_job_blocking(
        request: FEMRequest,
        job_id: FEMJobId,
        resp_tx: oneshot::Sender<Option<FEMResponse>>,
    ) {
        info!("[FEM-Job#{}] 开始: tower={}", job_id, request.tower.tower_id);
        let ts = std::time::Instant::now();

        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let mut fem = FEMAnalysis::new();
            fem.build_tower_mesh(&request.tower);
            fem.assemble_matrices();
            fem.apply_loads(
                &request.tower,
                request.wind_speed,
                request.tilt_deg,
                request.gravity,
                request.air_density,
                request.wind_drag_coefficient,
            );
            fem.apply_boundary_conditions(&request.tower);
            fem.solve();

            let node_results = fem.get_node_results(request.tower.tower_id, Utc::now(), request.tower.material_strength);
            let layer_stresses = fem.get_layer_stresses(&request.tower, request.wind_speed);
            let total_nodes = fem.nodes.len();

            FEMResponse {
                node_results,
                layer_stresses,
                total_nodes,
                job_id,
            }
        }));

        match result {
            Ok(resp) => {
                let elapsed = ts.elapsed().as_millis();
                info!(
                    "[FEM-Job#{}] 完成: nodes={}, layers={}, elapsed={}ms",
                    job_id, resp.total_nodes, resp.layer_stresses.len(), elapsed,
                );
                let _ = resp_tx.send(Some(resp));
            }
            Err(e) => {
                error!("[FEM-Job#{}] panic: {:?}", job_id, e);
                let _ = resp_tx.send(None);
            }
        }
    }

    pub fn pool_size(&self) -> usize {
        self.pool_size
    }

    pub async fn execute(&self, request: FEMRequest) -> Option<FEMResponse> {
        let (resp_tx, resp_rx) = oneshot::channel();
        let msg = PoolMessage::Job { request, resp_tx };

        if let Err(e) = self.sender.send(msg).await {
            error!("[FEM-Executor] 投递任务失败: {}", e);
            return None;
        }

        match resp_rx.await {
            Ok(resp) => resp,
            Err(e) => {
                error!("[FEM-Executor] 接收响应失败: {}", e);
                None
            }
        }
    }

    pub async fn execute_with_defaults(
        &self,
        tower: &TowerMetadata,
        wind_speed: f64,
        tilt_deg: f64,
    ) -> Option<FEMResponse> {
        let request = FEMRequest {
            tower: tower.clone(),
            wind_speed,
            tilt_deg,
            gravity: 9.81,
            air_density: 1.225,
            wind_drag_coefficient: 1.3,
        };
        self.execute(request).await
    }

    pub async fn shutdown(&self) {
        let _ = self.sender.send(PoolMessage::Shutdown).await;
    }
}

impl Drop for FEMExecutor {
    fn drop(&mut self) {
        let _ = self.sender.try_send(PoolMessage::Shutdown);
    }
}

pub struct FEMHandle {
    inner: Arc<FEMExecutor>,
}

impl FEMHandle {
    pub fn new(pool_size: usize) -> Self {
        Self { inner: Arc::new(FEMExecutor::new(pool_size)) }
    }

    pub fn executor(&self) -> Arc<FEMExecutor> {
        self.inner.clone()
    }

    pub fn pool_size(&self) -> usize {
        self.inner.pool_size()
    }
}

impl Default for FEMExecutor {
    fn default() -> Self {
        Self::new(num_cpus())
    }
}

impl Default for FEMHandle {
    fn default() -> Self {
        Self::new(num_cpus())
    }
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .max(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::get_default_tower;
    use tokio::runtime::Runtime;

    fn async_rt() -> Runtime {
        Runtime::new().unwrap()
    }

    #[test]
    fn test_fem_executor_constructs() {
        let rt = async_rt();
        rt.block_on(async {
            let exec = FEMExecutor::new(2);
            assert_eq!(exec.pool_size(), 2);
            exec.shutdown().await;
        });
    }

    #[test]
    fn test_fem_request_defaults() {
        let tower = get_default_tower(1);
        let rt = async_rt();
        rt.block_on(async {
            let exec = FEMExecutor::new(2);
            let resp = exec.execute_with_defaults(&tower, 15.0, 0.5).await;

            assert!(resp.is_some());
            let r = resp.unwrap();
            assert!(r.total_nodes > 0);
            assert!(!r.layer_stresses.is_empty());
            assert!(r.job_id > 0);
            exec.shutdown().await;
        });
    }

    #[test]
    fn test_fem_job_ids_increment() {
        let tower = get_default_tower(1);
        let rt = async_rt();
        rt.block_on(async {
            let exec = FEMExecutor::new(4);
            let r1 = exec.execute_with_defaults(&tower, 10.0, 0.3).await;
            let r2 = exec.execute_with_defaults(&tower, 20.0, 0.7).await;

            assert!(r1.is_some() && r2.is_some());
            assert_ne!(r1.unwrap().job_id, r2.unwrap().job_id);
            exec.shutdown().await;
        });
    }

    #[test]
    fn test_fem_handle_clones() {
        let rt = async_rt();
        rt.block_on(async {
            let handle = FEMHandle::new(2);
            let h2 = handle.executor();
            assert_eq!(h2.pool_size(), 2);
        });
    }

    #[test]
    fn test_fem_default_uses_parallelism() {
        let rt = async_rt();
        rt.block_on(async {
            let exec = FEMExecutor::default();
            assert!(exec.pool_size() >= 2);
            exec.shutdown().await;
        });
    }

    #[test]
    fn test_node_results_are_sampled() {
        let tower = get_default_tower(2);
        let rt = async_rt();
        rt.block_on(async {
            let exec = FEMExecutor::new(2);
            let resp = exec.execute_with_defaults(&tower, 15.0, 0.5).await;
            let r = resp.unwrap();
            for nr in &r.node_results {
                assert!(nr.von_mises >= 0.0);
            }
            exec.shutdown().await;
        });
    }
}

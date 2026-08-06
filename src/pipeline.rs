use std::sync::Arc;

use r_config::config::PipelineConfig;
use r_consumer::kafka::consumer::Work;
use r_consumer::process::process::Processor;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

mod dispatcher;
mod metrics;
mod worker;

use metrics::{spawn_pipeline_metrics, METRICS_INTERVAL};

pub struct Pipeline {
    ingress_tx: mpsc::Sender<Work>,
    dispatcher: JoinHandle<()>,
    workers: Vec<JoinHandle<()>>,
    metrics: JoinHandle<()>,
}

impl Pipeline {
        pub fn spawn(cfg: &PipelineConfig, process: Arc<Processor>) -> Self {
        let (ingress_tx, ingress_rx) = mpsc::channel::<Work>(cfg.ingress_queue);

        let chunk_max = cfg.chunk_max.max(1);

        let mut worker_txs: Vec<mpsc::Sender<Work>> = Vec::with_capacity(cfg.concurrency);
        let mut workers = Vec::with_capacity(cfg.concurrency);
        for _ in 0..cfg.concurrency {
            let (wtx, wrx) = mpsc::channel::<Work>(cfg.per_worker_queue);
            worker_txs.push(wtx);
            workers.push(tokio::spawn(worker::run_worker(
                wrx,
                process.clone(),
                chunk_max,
            )));
        }

        let metric_ingress = ingress_tx.downgrade();
        let metric_workers: Vec<_> = worker_txs.iter().map(mpsc::Sender::downgrade).collect();
        let metrics = spawn_pipeline_metrics(metric_ingress, metric_workers, METRICS_INTERVAL);

        let dispatcher = tokio::spawn(dispatcher::run_dispatcher(ingress_rx, worker_txs));

        Self {
            ingress_tx,
            dispatcher,
            workers,
            metrics,
        }
    }

    pub fn ingress(&self) -> mpsc::Sender<Work> {
        self.ingress_tx.clone()
    }

    pub async fn drain(self) {
        self.metrics.abort();
        drop(self.ingress_tx);
        let _ = self.dispatcher.await;
        for h in self.workers {
            let _ = h.await;
        }
    }
}

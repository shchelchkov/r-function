use std::time::Duration;

use r_consumer::kafka::consumer::Work;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub(super) const METRICS_INTERVAL: Duration = Duration::from_secs(10);

pub(super) fn spawn_pipeline_metrics(
    ingress: mpsc::WeakSender<Work>,
    workers: Vec<mpsc::WeakSender<Work>>,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(interval);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            let Some(ingress) = ingress.upgrade() else {
                break; 
            };
            let ingress_depth = ingress.max_capacity() - ingress.capacity();

            let mut worker_max = 0usize;
            let mut worker_sum = 0usize;
            let mut worker_cap = 0usize;
            for w in &workers {
                if let Some(s) = w.upgrade() {
                    worker_cap = s.max_capacity();
                    let depth = s.max_capacity() - s.capacity();
                    worker_sum += depth;
                    worker_max = worker_max.max(depth);
                }
            }

            tracing::info!(
                target: "metrics::pipeline",
                ingress_depth,
                ingress_cap = ingress.max_capacity(),
                worker_max_depth = worker_max,
                worker_sum_depth = worker_sum,
                worker_cap,
                workers = workers.len(),
                "queue depth"
            );
        }
    })
}

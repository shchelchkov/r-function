use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures::StreamExt;
use futures::stream::FuturesUnordered;
use r_producer::kafka::producer::{DeliveryFuture, Enqueued, Producer};
use tokio::sync::mpsc::{self, Sender};
use tokio::task::AbortHandle;
use tracing::{info, warn};

use crate::runtime::host::SendJob;

const SEND_QUEUE_CAP: usize = 4096;
const SEND_SHARDS: usize = 8;
const SEND_MAX_INFLIGHT: usize = 1024;
const SEND_METRICS_INTERVAL: Duration = Duration::from_secs(10);

pub struct SendPipeline {
    aborts: Vec<AbortHandle>,
}

impl SendPipeline {
    pub fn new(producer: Producer) -> (Self, Arc<[Sender<SendJob>]>) {
        let mut send_txs = Vec::with_capacity(SEND_SHARDS);
        let mut aborts = Vec::with_capacity(SEND_SHARDS + 1);
        let mut dropped: Vec<Arc<AtomicU64>> = Vec::with_capacity(SEND_SHARDS);
        for _ in 0..SEND_SHARDS {
            let (tx, rx) = mpsc::channel::<SendJob>(SEND_QUEUE_CAP);
            let shard_dropped = Arc::new(AtomicU64::new(0));
            aborts.push(spawn_send_dispatcher(
                producer.clone(),
                rx,
                shard_dropped.clone(),
            ));
            dropped.push(shard_dropped);
            send_txs.push(tx);
        }
        let metric_txs: Vec<_> = send_txs.iter().map(mpsc::Sender::downgrade).collect();
        aborts.push(spawn_send_metrics(
            metric_txs,
            dropped,
            SEND_METRICS_INTERVAL,
        ));

        (Self { aborts }, send_txs.into())
    }
}

impl Drop for SendPipeline {
    fn drop(&mut self) {
        for h in &self.aborts {
            h.abort();
        }
    }
}

fn spawn_send_dispatcher(
    producer: Producer,
    mut rx: mpsc::Receiver<SendJob>,
    dropped: Arc<AtomicU64>,
) -> AbortHandle {
    let h = tokio::spawn(async move {
        let mut inflight: FuturesUnordered<DeliveryFuture> = FuturesUnordered::new();
        loop {
            tokio::select! {
                biased;
                Some(res) = inflight.next(), if !inflight.is_empty() => {
                    log_delivery_result(res, &dropped);
                }
                maybe = rx.recv() => {
                    let Some(job) = maybe else { break };
                    while inflight.len() >= SEND_MAX_INFLIGHT {
                        if let Some(res) = inflight.next().await {
                            log_delivery_result(res, &dropped);
                        } else {
                            break;
                        }
                    }
                    enqueue_with_backpressure(&producer, &mut inflight, job, &dropped).await;
                }
            }
        }
        while let Some(res) = inflight.next().await {
            log_delivery_result(res, &dropped);
        }
    });
    h.abort_handle()
}

async fn enqueue_with_backpressure(
    producer: &Producer,
    inflight: &mut FuturesUnordered<DeliveryFuture>,
    job: SendJob,
    dropped: &AtomicU64,
) {
    let mut from = 0usize;
    loop {
        match producer.enqueue(
            &job.setting_code,
            job.channel.as_deref(),
            job.key.as_deref(),
            &job.payload,
            from,
        ) {
            Ok(Enqueued::Queued(futs)) => {
                inflight.extend(futs);
                return;
            }
            Ok(Enqueued::QueueFull {
                queued,
                from_position,
            }) => {
                inflight.extend(queued);
                from = from_position;
                match inflight.next().await {
                    Some(res) => log_delivery_result(res, dropped),
                    None => tokio::task::yield_now().await,
                }
            }
            Err(e) => {
                warn!(error = %e, setting_code = %job.setting_code, "send_value enqueue failed (dropped)");
                dropped.fetch_add(1, Ordering::Relaxed);
                return;
            }
        }
    }
}

fn log_delivery_result<D, E1, E2>(res: Result<Result<D, E1>, E2>, dropped: &AtomicU64)
where
    E1: std::fmt::Debug,
    E2: std::fmt::Debug,
{
    match res {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => {
            warn!(error = ?e, "send_value delivery failed (dropped)");
            dropped.fetch_add(1, Ordering::Relaxed);
        }
        Err(e) => {
            warn!(error = ?e, "send_value delivery canceled (dropped)");
            dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn spawn_send_metrics(
    txs: Vec<mpsc::WeakSender<SendJob>>,
    dropped: Vec<Arc<AtomicU64>>,
    interval: Duration,
) -> AbortHandle {
    let h = tokio::spawn(async move {
        let mut tick = tokio::time::interval(interval);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            let mut max_depth = 0usize;
            let mut sum_depth = 0usize;
            let mut cap = 0usize;
            let mut alive = 0usize;
            for w in &txs {
                if let Some(s) = w.upgrade() {
                    alive += 1;
                    cap = s.max_capacity();
                    let depth = s.max_capacity() - s.capacity();
                    sum_depth += depth;
                    max_depth = max_depth.max(depth);
                }
            }
            if alive == 0 {
                break; 
            }
            let dropped_total: u64 = dropped.iter().map(|d| d.load(Ordering::Relaxed)).sum();
            info!(
                target: "metrics::send",
                shards = alive,
                cap_per_shard = cap,
                max_depth,
                sum_depth,
                dropped = dropped_total,
                "send queue depth"
            );
        }
    });
    h.abort_handle()
}

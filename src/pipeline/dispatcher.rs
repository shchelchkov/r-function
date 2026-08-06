use r_consumer::kafka::consumer::Work;
use tokio::sync::mpsc;

pub(super) async fn run_dispatcher(
    mut ingress_rx: mpsc::Receiver<Work>,
    worker_txs: Vec<mpsc::Sender<Work>>,
) {
    while let Some(work) = ingress_rx.recv().await {
        let idx = worker_index(work.msg.topic(), work.msg.partition(), worker_txs.len());
        if worker_txs[idx].send(work).await.is_err() {
            break;
        }
    }
}

fn worker_index(_topic: &str, partition: i32, worker_count: usize) -> usize {
    (partition as usize) % worker_count
}

#[cfg(test)]
mod tests {
    use super::worker_index;
    use std::collections::HashSet;

    #[test]
    fn same_partition_is_stable() {
        assert_eq!(
            worker_index("trades", 3, 8),
            worker_index("trades", 3, 8),
            "same (topic, partition) must always map to the same worker"
        );
    }

    #[test]
    fn index_stays_within_bounds() {
        for partition in 0..1000 {
            assert!(worker_index("topic", partition, 8) < 8);
        }
    }

    #[test]
    fn distributes_across_all_workers() {
        let seen: HashSet<usize> = (0..1000)
            .map(|partition| worker_index("topic", partition, 8))
            .collect();
        assert_eq!(seen.len(), 8, "every worker should receive some partitions");
    }
}

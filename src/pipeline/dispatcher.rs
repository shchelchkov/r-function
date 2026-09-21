use r_consumer::kafka::consumer::Work;
use r_consumer::process::process::Message;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tokio::sync::mpsc;

pub(super) async fn run_dispatcher(
    mut ingress_rx: mpsc::Receiver<Work>,
    worker_txs: Vec<mpsc::Sender<Work>>,
) {
    while let Some(work) = ingress_rx.recv().await {
        let idx = worker_index(&work.msg, worker_txs.len());
        if worker_txs[idx].send(work).await.is_err() {
            break;
        }
    }
}

fn worker_index(msg: &Message, worker_count: usize) -> usize {
    match msg.payload.as_deref().and_then(<[_]>::first) {
        Some(v) => {
            let mut hasher = DefaultHasher::new();
            v.key.hash(&mut hasher);
            (hasher.finish() as usize) % worker_count
        }
        None => (msg.partition() as usize) % worker_count,
    }
}

#[cfg(test)]
mod tests {
    use super::worker_index;
    use r_consumer::process::process::Message;
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    fn msg(partition: i32, key: Option<&str>) -> Message {
        let mut m = Message::new("src".into(), partition, 0, None, None, HashMap::new(), None);
        m.payload = key.map(|k| {
            vec![value_rs::Value {
                value_key: vec!["topic".to_string()],
                setting_code: "sc".into(),
                key: Arc::from(k),
                value: sonic_rs::Value::default(),
            }]
        });
        m
    }

    #[test]
    fn same_key_different_partition_is_stable() {
        let key = "orderbook.50.BTCUSDT";
        assert_eq!(
            worker_index(&msg(3, Some(key)), 8),
            worker_index(&msg(5, Some(key)), 8),
            "same key on two different partitions must map to the same worker"
        );
    }

    #[test]
    fn distinct_keys_spread_across_workers() {
        let seen: HashSet<usize> = (0..1000)
            .map(|i| worker_index(&msg(0, Some(&format!("orderbook.50.SYM{i}USDT"))), 8))
            .collect();
        assert_eq!(
            seen.len(),
            8,
            "distinct keys should spread across every worker"
        );
    }

    #[test]
    fn no_key_falls_back_to_partition_routing() {
        for partition in 0..8 {
            assert_eq!(worker_index(&msg(partition, None), 8), partition as usize);
        }
    }

    #[test]
    fn no_key_index_stays_within_bounds() {
        for partition in 0..1000 {
            assert!(worker_index(&msg(partition, None), 8) < 8);
        }
    }
}

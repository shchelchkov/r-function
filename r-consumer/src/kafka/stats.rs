use rdkafka::{ClientContext, Statistics, consumer::ConsumerContext};

pub(crate) struct StatsContext;

impl ClientContext for StatsContext {
    fn stats(&self, stats: Statistics) {
        for (topic, t) in &stats.topics {
            for (pid, p) in &t.partitions {
                tracing::debug!(
                    target: "kafka::lag",
                    topic, partition = pid,
                    committed = p.committed_offset,
                    stored = p.stored_offset,
                    hi = p.hi_offset,
                    lag = p.consumer_lag,
                    "partition state"
                );
            }
        }

        let max_lag: i64 = stats
            .topics
            .values()
            .flat_map(|t| t.partitions.values())
            .map(|p| p.consumer_lag)
            .filter(|l| *l >= 0)
            .max()
            .unwrap_or(0);

        tracing::info!(
            target: "kafka::stats",
            client = %stats.name,
            client_type = %stats.client_type,
            rxmsgs = stats.rxmsgs,
            rxmsg_bytes = stats.rxmsg_bytes,
            rx_bytes = stats.rx_bytes,
            tx = stats.tx,
            rx = stats.rx,
            max_consumer_lag = max_lag,
            "rdkafka stats"
        );
    }
}

impl ConsumerContext for StatsContext {}

use futures::StreamExt;
use futures::stream::FuturesUnordered;
use r_config::config::KafkaConfig;
use r_error::kafka::KafkaError;
use r_process::process::Message;
use rdkafka::consumer::{CommitMode, Consumer as _, StreamConsumer};
use rdkafka::error::{KafkaError as RdKafkaError, RDKafkaErrorCode};
use rdkafka::{ClientConfig, Offset, TopicPartitionList};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error};

use crate::kafka::convert::into_inbound;
use crate::kafka::offset::Tracker;
use crate::kafka::stats::StatsContext;

pub struct Work {
    pub msg: Message,
    pub ack: oneshot::Sender<Result<(), ()>>,
}

pub struct Consumer {
    consumer: StreamConsumer<StatsContext>,
}

impl Consumer {
    pub fn new(cfg: &KafkaConfig) -> Result<Self, KafkaError> {
        let mut client = ClientConfig::new();
        client
            .set("bootstrap.servers", &cfg.bootstrap_servers)
            .set("client.id", &cfg.client_id)
            .set("group.id", &cfg.group_id)
            .set("enable.auto.commit", "true")
            .set("enable.auto.offset.store", "false")
            .set("auto.commit.interval.ms", "5000");

        if !cfg.parameter.contains_key("statistics.interval.ms") {
            client.set("statistics.interval.ms", "10000");
        }
        for (k, v) in &cfg.parameter {
            client.set(k, v);
        }

        let consumer: StreamConsumer<StatsContext> = client.create_with_context(StatsContext)?;

        if !cfg.topics.is_empty() {
            let refs: Vec<&str> = cfg.topics.iter().map(|s| s.as_str()).collect();
            consumer
                .subscribe(&refs)
                .map_err(|e| KafkaError::Subscribe(e.to_string()))?;
        }

        Ok(Self { consumer })
    }

    pub async fn run<S>(
        &self,
        tx: mpsc::Sender<Work>,
        max_inflight: usize,
        shutdown: S,
    ) -> Result<(), KafkaError>
    where
        S: Future<Output = ()>,
    {
        let tracker: Tracker = Arc::new(Mutex::new(HashMap::new()));
        let max_inflight = max_inflight.max(1);
        let mut inflight = FuturesUnordered::new();

        let stream = self.consumer.stream().take_until(shutdown);
        tokio::pin!(stream);

        loop {
            if inflight.len() >= max_inflight {
                if let Some(done) = inflight.next().await {
                    self.apply_commit(done);
                }
                continue;
            }

            tokio::select! {
                biased;
                Some(done) = inflight.next(), if !inflight.is_empty() => {
                    self.apply_commit(done);
                }
                res = stream.next() => {
                    let Some(res) = res else { break; };
                    let inbound = match res {
                        Ok(b) => into_inbound(&b),
                        Err(e) => { error!(%e); continue; }
                    };
                    let topic = inbound.topic().to_string();
                    let partition = inbound.partition();
                    let offset = inbound.offset();

                    tracker.lock().unwrap()
                        .entry((topic.clone(), partition))
                        .or_default()
                        .observe(offset);

                    let (ack_tx, ack_rx) = oneshot::channel();
                    if tx.send(Work { msg: inbound, ack: ack_tx }).await.is_err() {
                        error!("processor channel closed");
                        break;
                    }

                    let tracker = tracker.clone();
                    inflight.push(async move {
                        match ack_rx.await {
                            Ok(Ok(())) => tracker.lock().unwrap()
                                .entry((topic.clone(), partition))
                                .or_default()
                                .complete(offset)
                                .map(|next| (topic, partition, next)),
                            _ => {
                                error!(%topic, partition, offset, "processing failed; offset not advanced");
                                None
                            }
                        }
                    });
                }
            }
        }

        while let Some(done) = inflight.next().await {
            self.apply_commit(done);
        }

        if let Err(e) = self.consumer.commit_consumer_state(CommitMode::Sync) {
            error!(error=%e, "final commit failed");
        }
        self.consumer.unsubscribe();
        Ok(())
    }

    fn apply_commit(&self, done: Option<(String, i32, i64)>) {
        let Some((topic, partition, next)) = done else {
            return;
        };
        let mut tpl = TopicPartitionList::new();
        if tpl
            .add_partition_offset(&topic, partition, Offset::Offset(next))
            .is_ok()
        {
            match self.consumer.store_offsets(&tpl) {
                Ok(()) => {}
                Err(RdKafkaError::StoreOffset(RDKafkaErrorCode::State)) => {
                    debug!(%topic, partition, next, "store_offsets skipped: partition not assigned (rebalanced away)");
                }
                Err(e) => error!(error=%e, %topic, partition, next, "store_offsets"),
            }
        }
    }
}

use crate::kafka::convert::into_inbound;
use crate::kafka::offset::Tracker;
use crate::kafka::stats::StatsContext;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use r_config::config::KafkaConfig;
use r_error::kafka::KafkaError;
use r_process::process::{Message, MessagePublisher, Resolver};
use r_process::producer::kafka::producer::DlqContext;
use rdkafka::consumer::{CommitMode, Consumer as _, StreamConsumer};
use rdkafka::error::{KafkaError as RdKafkaError, RDKafkaErrorCode};
use rdkafka::{ClientConfig, Message as _, Offset, TopicPartitionList};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::MissedTickBehavior;
use tracing::{debug, error};

pub struct Work {
    pub msg: Message,
    pub ack: oneshot::Sender<Result<(), ()>>,
}

pub struct Consumer {
    resolver: Arc<Resolver>,
    publisher: Arc<dyn MessagePublisher>,
    consumer: StreamConsumer<StatsContext>,
}

impl Consumer {
    pub fn new(
        cfg: &KafkaConfig,
        resolver: Arc<Resolver>,
        publisher: Arc<dyn MessagePublisher>,
    ) -> Result<Self, KafkaError> {
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

        let consumer: StreamConsumer<StatsContext> =
            client.create_with_context(StatsContext::default())?;

        if !cfg.topics.is_empty() {
            let refs: Vec<&str> = cfg.topics.iter().map(|s| s.as_str()).collect();
            consumer
                .subscribe(&refs)
                .map_err(|e| KafkaError::Subscribe(e.to_string()))?;
        }

        Ok(Self {
            resolver,
            publisher,
            consumer,
        })
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

        let rps = self.consumer.context().rps.clone();
        let ticker = tokio::spawn({
            let rps = rps.clone();
            async move {
                let mut tick = tokio::time::interval(Duration::from_secs(1));
                tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
                let mut last = tick.tick().await;
                loop {
                    let now = tick.tick().await;
                    rps.sample(now - last);
                    last = now;
                }
            }
        });

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
                    let b = match res {
                        Ok(b) => b,
                        Err(e) => { error!(%e); continue; }
                    };
                    rps.inc();

                    let resolved = match b.payload() {
                        Some(raw) => self.resolver.resolve_value(raw).await,
                        None => Ok(None),
                    };

                    let topic = b.topic().to_string();
                    let partition = b.partition();
                    let offset = b.offset();

                    tracker.lock().unwrap()
                        .entry((topic.clone(), partition))
                        .or_default()
                        .observe(offset);

                    let (ack_tx, ack_rx) = oneshot::channel();
                    match resolved {
                        Ok(value) => {
                            let msg = into_inbound(&b, value);
                            if tx.send(Work { msg, ack: ack_tx }).await.is_err() {
                                error!("processor channel closed");
                                break;
                            }
                        }
                        Err(e) => spawn_dlq(self.publisher.clone(), &b, e.to_string(), ack_tx),
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
        ticker.abort();

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

fn spawn_dlq<M: rdkafka::Message>(
    publisher: Arc<dyn MessagePublisher>,
    m: &M,
    reason: String,
    ack: oneshot::Sender<Result<(), ()>>,
) {
    let raw = m.payload().unwrap_or_default().to_vec();
    let key = m.key().map(<[u8]>::to_vec);
    let topic = m.topic().to_string();
    let (partition, offset) = (m.partition(), m.offset());
    tokio::spawn(async move {
        let ctx = DlqContext {
            reason: &reason,
            source_topic: &topic,
            source_partition: partition,
            source_offset: offset,
        };
        let res = publisher
            .send_dlq(&raw, key.as_deref(), ctx)
            .await
            .map_err(|e| error!(error = %e, "failed to route poison message to DLQ"));
        let _ = ack.send(res);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use r_process::producer::kafka::producer::KafkaSendError;
    use rdkafka::message::{OwnedMessage, Timestamp};

    #[derive(Debug, PartialEq)]
    struct DlqRecord {
        payload: Vec<u8>,
        key: Option<Vec<u8>>,
        reason: String,
        topic: String,
        partition: i32,
        offset: i64,
    }

    #[derive(Default)]
    struct DlqRecorder {
        sent: Mutex<Vec<DlqRecord>>,
    }

    #[async_trait]
    impl MessagePublisher for DlqRecorder {
        async fn send_objects(
            &self,
            _setting_code: &str,
            _key: Option<&[u8]>,
            _objects: &[sonic_rs::Value],
        ) -> Result<(), KafkaSendError> {
            Ok(())
        }

        async fn send_dlq(
            &self,
            payload: &[u8],
            key: Option<&[u8]>,
            ctx: DlqContext<'_>,
        ) -> Result<(), KafkaSendError> {
            self.sent.lock().unwrap().push(DlqRecord {
                payload: payload.to_vec(),
                key: key.map(<[u8]>::to_vec),
                reason: ctx.reason.to_string(),
                topic: ctx.source_topic.to_string(),
                partition: ctx.source_partition,
                offset: ctx.source_offset,
            });
            Ok(())
        }
    }

    #[tokio::test]
    async fn broken_message_goes_to_dlq_and_acks() {
        let publisher = Arc::new(DlqRecorder::default());
        let m = OwnedMessage::new(
            Some(b"{not json".to_vec()),
            Some(b"k".to_vec()),
            "src".into(),
            Timestamp::NotAvailable,
            3,
            42,
            None,
        );
        let (ack_tx, ack_rx) = oneshot::channel();

        spawn_dlq(publisher.clone(), &m, "invalid JSON payload".into(), ack_tx);

        assert_eq!(ack_rx.await, Ok(Ok(())), "DLQ delivered -> offset advances");
        assert_eq!(
            *publisher.sent.lock().unwrap(),
            vec![DlqRecord {
                payload: b"{not json".to_vec(),
                key: Some(b"k".to_vec()),
                reason: "invalid JSON payload".to_string(),
                topic: "src".to_string(),
                partition: 3,
                offset: 42,
            }],
            "raw bytes, key and source coordinates go to DLQ as is"
        );
    }
}

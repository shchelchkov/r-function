use std::time::Duration;

use rdkafka::message::{Header, OwnedHeaders};
use rdkafka::producer::{FutureProducer, FutureRecord};

use crate::kafka::error::KafkaSendError;

pub struct DlqContext<'a> {
    pub reason: &'a str,
    pub source_topic: &'a str,
    pub source_partition: i32,
    pub source_offset: i64,
}

pub(crate) async fn send_dlq(
    producer: &FutureProducer,
    dlq_topic: Option<&str>,
    payload: &[u8],
    key: Option<&[u8]>,
    ctx: DlqContext<'_>,
    timeout: Duration,
) -> Result<(), KafkaSendError> {
    let Some(topic) = dlq_topic else {
        tracing::error!(
            reason = ctx.reason,
            source_topic = ctx.source_topic,
            source_partition = ctx.source_partition,
            source_offset = ctx.source_offset,
            payload_len = payload.len(),
            "poison message dropped: no dlq_topic configured (skip-and-commit)"
        );
        return Ok(());
    };

    let partition = ctx.source_partition.to_string();
    let offset = ctx.source_offset.to_string();
    let headers = OwnedHeaders::new()
        .insert(Header {
            key: "dlq.reason",
            value: Some(ctx.reason),
        })
        .insert(Header {
            key: "dlq.source.topic",
            value: Some(ctx.source_topic),
        })
        .insert(Header {
            key: "dlq.source.partition",
            value: Some(&partition),
        })
        .insert(Header {
            key: "dlq.source.offset",
            value: Some(&offset),
        });

    let mut record = FutureRecord::<[u8], [u8]>::to(topic)
        .payload(payload)
        .headers(headers);
    if let Some(k) = key {
        record = record.key(k);
    }

    producer
        .send(record, timeout)
        .await
        .map_err(|(e, _)| KafkaSendError {
            details: format!("dlq send to `{topic}` failed: {e:?}"),
        })?;

    tracing::warn!(
        dlq_topic = topic,
        reason = ctx.reason,
        source_topic = ctx.source_topic,
        source_partition = ctx.source_partition,
        source_offset = ctx.source_offset,
        "poison message routed to DLQ"
    );
    Ok(())
}

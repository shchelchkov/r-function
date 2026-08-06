use std::time::Duration;

use futures::StreamExt;
use futures::stream::FuturesUnordered;
use r_config::config::KafkaConfig;
use r_error::kafka::KafkaError;
use r_feed::FeedHub;
use r_setting::streams::stream::Stream;
use rdkafka::error::{KafkaError as RdKafkaError, RDKafkaErrorCode};
use rdkafka::producer::{FutureProducer, FutureRecord};

pub use rdkafka::producer::DeliveryFuture;
use sonic_rs::{JsonContainerTrait, JsonValueMutTrait, JsonValueTrait, Value};

use crate::kafka::client;
use crate::kafka::dlq;
pub use crate::kafka::dlq::DlqContext;
pub use crate::kafka::error::KafkaSendError;
use crate::kafka::route::RouteSource;

const SEND_TIMEOUT: Duration = Duration::from_secs(5);
const OBJ_MAX_INFLIGHT: usize = 1024;

pub enum Enqueued {
    Queued(Vec<DeliveryFuture>),
    QueueFull {
        queued: Vec<DeliveryFuture>,
        from_position: usize,
    },
}

#[derive(Clone)]
pub struct Producer {
    pub topics: Vec<String>,
    pub producer: FutureProducer,
    stream: Stream,
    dlq_topic: Option<String>,
    feed: FeedHub,
}

impl Producer {
    pub fn new(cfg: &KafkaConfig, stream: Stream) -> Result<Self, KafkaError> {
        Ok(Self {
            topics: cfg.topics.clone(),
            producer: client::build_producer(cfg)?,
            stream,
            dlq_topic: cfg.dlq_topic.clone(),
            feed: FeedHub::new(),
        })
    }

    pub fn with_feed(mut self, feed: FeedHub) -> Self {
        self.feed = feed;
        self
    }

    fn resolve(&self, setting_code: &str) -> RouteSource {
        match self.stream.get_stream_setting(setting_code) {
            Some(s) => RouteSource::Streams(s),
            None => RouteSource::Static,
        }
    }

    pub fn enqueue(
        &self,
        setting_code: &str,
        channel: Option<&[u8]>,
        key: Option<&[u8]>,
        payload: &[u8],
        from: usize,
    ) -> Result<Enqueued, KafkaSendError> {
        if setting_code.is_empty() {
            return Err(KafkaSendError {
                details: "setting_code is empty".to_string(),
            });
        }

        let route = self.resolve(setting_code);
        let routes: Vec<(&str, Option<&str>)> = route.routes(&self.topics).collect();
        if routes.is_empty() {
            return Err(KafkaSendError {
                details: format!("no active route for `{setting_code}`"),
            });
        }
        let objects = parse_objects(payload)?;

        self.enqueue_to(&routes, channel, key, &objects, from, &setting_code)
    }

    fn enqueue_to(
        &self,
        routes: &[(&str, Option<&str>)],
        channel_ws: Option<&[u8]>,
        key: Option<&[u8]>,
        objects: &[Value],
        from: usize,
        _setting_code: &str,
    ) -> Result<Enqueued, KafkaSendError> {
        let n = routes.len();
        let channel_ws = channel_ws.and_then(|b| std::str::from_utf8(b).ok());
        let mut queued = Vec::with_capacity((objects.len() * n).saturating_sub(from));
        let mut cur_obj: Option<usize> = None;
        let mut base: Option<Vec<u8>> = None;
        let mut rewritten: Vec<(&str, Vec<u8>)> = Vec::new();
        for (obj_idx, route_idx) in grid_positions(objects.len(), n, from) {
            if cur_obj != Some(obj_idx) {
                cur_obj = Some(obj_idx);
                base = None;
                rewritten.clear();
            }
            let obj = &objects[obj_idx];
            let (channel, scs) = routes[route_idx];
            let payload: &[u8] = match scs {
                Some(code) => {
                    if rewritten.iter().all(|(c, _)| *c != code) {
                        let p = serialize_with_setting_code(obj, code)?;
                        rewritten.push((code, p));
                    }
                    rewritten
                        .iter()
                        .find_map(|(c, p)| (*c == code).then_some(p.as_slice()))
                        .expect("just inserted")
                }
                None => {
                    if base.is_none() {
                        base = Some(serialize_object(obj)?);
                    }
                    base.as_deref().expect("just inserted")
                }
            };
            if let Some(ch) = channel_ws {
                self.feed.publish(ch, key, payload);
            } else {
                let key_ws = key.and_then(|b| std::str::from_utf8(b).ok());
                if let Some(k) = key_ws {
                    self.feed.publish(k, key, payload);
                }
            }

            let mut record = FutureRecord::<[u8], [u8]>::to(channel).payload(payload);
            if let Some(k) = key {
                record = record.key(k);
            }
            match self.producer.send_result(record) {
                Ok(fut) => queued.push(fut),
                Err((RdKafkaError::MessageProduction(RDKafkaErrorCode::QueueFull), _)) => {
                    return Ok(Enqueued::QueueFull {
                        queued,
                        from_position: obj_idx * n + route_idx,
                    });
                }
                Err((e, _)) => {
                    return Err(KafkaSendError {
                        details: format!("enqueue to `{channel}` failed: {e:?}"),
                    });
                }
            }
        }
        Ok(Enqueued::Queued(queued))
    }

    pub async fn send_dlq(
        &self,
        payload: &[u8],
        key: Option<&[u8]>,
        ctx: DlqContext<'_>,
    ) -> Result<(), KafkaSendError> {
        dlq::send_dlq(
            &self.producer,
            self.dlq_topic.as_deref(),
            payload,
            key,
            ctx,
            SEND_TIMEOUT,
        )
        .await
    }

    pub async fn send_objects(
        &self,
        setting_code: &str,
        key: Option<&[u8]>,
        payload: Vec<u8>,
    ) -> Result<(), KafkaSendError> {
        if setting_code.is_empty() {
            return Err(KafkaSendError {
                details: "setting_code is empty".to_string(),
            });
        }

        let route = self.resolve(setting_code);
        let routes: Vec<(&str, Option<&str>)> = route.routes(&self.topics).collect();
        if routes.is_empty() {
            return Ok(());
        }

        let objects = parse_objects(&payload)?;

        let mut inflight: FuturesUnordered<DeliveryFuture> = FuturesUnordered::new();
        for obj in &objects {
            let mut base: Option<Vec<u8>> = None;
            let mut rewritten: Vec<(&str, Vec<u8>)> = Vec::new();
            for &(channel, scs) in &routes {
                let payload: &[u8] = match scs {
                    Some(code) => {
                        if rewritten.iter().all(|(c, _)| *c != code) {
                            let p = serialize_with_setting_code(obj, code)?;
                            rewritten.push((code, p));
                        }
                        rewritten
                            .iter()
                            .find_map(|(c, p)| (*c == code).then_some(p.as_slice()))
                            .expect("just inserted")
                    }
                    None => {
                        if base.is_none() {
                            base = Some(serialize_object(obj)?);
                        }
                        base.as_deref().expect("just inserted")
                    }
                };
                self.feed.publish(channel, key, payload);
                self.enqueue_payload(payload, channel, key, &mut inflight)
                    .await?;
            }
        }

        while let Some(res) = inflight.next().await {
            Self::check_delivery(res)?;
        }
        Ok(())
    }

    async fn enqueue_payload(
        &self,
        payload: &[u8],
        channel: &str,
        key: Option<&[u8]>,
        inflight: &mut FuturesUnordered<DeliveryFuture>,
    ) -> Result<(), KafkaSendError> {
        loop {
            while inflight.len() >= OBJ_MAX_INFLIGHT {
                match inflight.next().await {
                    Some(res) => Self::check_delivery(res)?,
                    None => break,
                }
            }
            let mut record = FutureRecord::<[u8], [u8]>::to(channel).payload(payload);
            if let Some(k) = key {
                record = record.key(k);
            }
            match self.producer.send_result(record) {
                Ok(fut) => {
                    inflight.push(fut);
                    return Ok(());
                }
                Err((RdKafkaError::MessageProduction(RDKafkaErrorCode::QueueFull), _)) => {
                    match inflight.next().await {
                        Some(res) => Self::check_delivery(res)?,
                        None => {
                            return Err(KafkaSendError {
                                details: "queue full with no in-flight deliveries".to_string(),
                            });
                        }
                    }
                }
                Err((e, _)) => {
                    return Err(KafkaSendError {
                        details: format!("enqueue to `{channel}` failed: {e:?}"),
                    });
                }
            }
        }
    }

    fn check_delivery<D, E1, E2>(res: Result<Result<D, E1>, E2>) -> Result<(), KafkaSendError>
    where
        E1: std::fmt::Debug,
        E2: std::fmt::Debug,
    {
        match res {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(KafkaSendError {
                details: format!("delivery failed: {e:?}"),
            }),
            Err(e) => Err(KafkaSendError {
                details: format!("delivery canceled: {e:?}"),
            }),
        }
    }
}

fn grid_positions(
    n_objects: usize,
    n_routes: usize,
    from: usize,
) -> impl Iterator<Item = (usize, usize)> {
    (from..n_objects * n_routes).map(move |pos| (pos / n_routes, pos % n_routes))
}

fn parse_objects(payload: &[u8]) -> Result<Vec<Value>, KafkaSendError> {
    let v: Value = sonic_rs::from_slice(payload).map_err(|e| KafkaSendError {
        details: format!("decode: {e}"),
    })?;

    if let Some(values) = v.as_array() {
        Ok(values
            .iter()
            .filter(|value| value.is_object())
            .cloned()
            .collect())
    } else if v.is_object() {
        Ok(vec![v])
    } else {
        Ok(Vec::new())
    }
}

fn serialize_object(obj: &Value) -> Result<Vec<u8>, KafkaSendError> {
    sonic_rs::to_vec(obj).map_err(|e| KafkaSendError {
        details: format!("serialize: {e}"),
    })
}

fn serialize_with_setting_code(obj: &Value, code: &str) -> Result<Vec<u8>, KafkaSendError> {
    let mut o = obj.clone();
    o.as_object_mut()
        .ok_or_else(|| KafkaSendError {
            details: "value is not an object".to_string(),
        })?
        .insert("setting_code", code);
    serialize_object(&o)
}

#[cfg(test)]
mod tests {
    use super::{grid_positions, parse_objects, serialize_with_setting_code};
    use sonic_rs::JsonValueTrait;

    #[test]
    fn grid_positions_yields_full_grid_in_order() {
        let pos: Vec<(usize, usize)> = grid_positions(2, 3, 0).collect();
        assert_eq!(pos, [(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (1, 2)]);
    }

    #[test]
    fn grid_positions_resumes_from_flat_offset() {
        let pos: Vec<(usize, usize)> = grid_positions(2, 3, 4).collect();
        assert_eq!(pos, [(1, 1), (1, 2)]);
    }

    #[test]
    fn grid_positions_empty_when_no_objects_or_routes() {
        assert_eq!(grid_positions(0, 3, 0).count(), 0);
        assert_eq!(grid_positions(2, 0, 0).count(), 0);
    }

    #[test]
    fn grid_positions_from_at_or_past_end_yields_nothing() {
        assert_eq!(grid_positions(2, 3, 6).count(), 0);
        assert_eq!(grid_positions(2, 3, 99).count(), 0);
    }

    #[test]
    fn array_yields_one_object_per_element() {
        let input = br#"[{"a":1},{"a":2},{"a":3}]"#;
        let out = parse_objects(input).expect("valid json");
        assert_eq!(out.len(), 3);
        for v in &out {
            assert!(v.is_object());
        }
    }

    #[test]
    fn single_object_yields_one_value() {
        let out = parse_objects(br#"{"a":1}"#).expect("valid json");
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn empty_array_yields_no_values() {
        let out = parse_objects(b"[]").expect("valid json");
        assert!(out.is_empty());
    }

    #[test]
    fn non_object_array_elements_are_skipped() {
        let out = parse_objects(br#"[{"a":1}, 42, "x", {"b":2}]"#).expect("valid json");
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn malformed_json_is_error() {
        assert!(parse_objects(b"{not json").is_err());
        assert!(parse_objects(b"").is_err());
    }

    #[test]
    fn rewrite_replaces_existing_setting_code() {
        let obj = &parse_objects(br#"{"setting_code":"old","x":1}"#).unwrap()[0];
        let bytes = serialize_with_setting_code(obj, "new").unwrap();
        let v: sonic_rs::Value = sonic_rs::from_slice(&bytes).unwrap();
        assert_eq!(v.get("setting_code").as_str(), Some("new"));
        assert_eq!(v.get("x").as_i64(), Some(1));
        assert_eq!(obj.get("setting_code").as_str(), Some("old"));
    }

    #[test]
    fn rewrite_adds_setting_code_when_absent() {
        let obj = &parse_objects(br#"{"x":1}"#).unwrap()[0];
        let bytes = serialize_with_setting_code(obj, "new").unwrap();
        let v: sonic_rs::Value = sonic_rs::from_slice(&bytes).unwrap();
        assert_eq!(v.get("setting_code").as_str(), Some("new"));
    }
}

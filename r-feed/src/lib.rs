use std::sync::Arc;

use bytes::Bytes;
use dashmap::DashMap;
use tokio::sync::broadcast;
use tracing::info;

pub const DEFAULT_CAPACITY: usize = 1024;

#[derive(Clone, Debug)]
pub struct FeedEvent {
    pub channel: Arc<str>,
    pub key: Option<Bytes>,
    pub payload: Bytes,
}

#[derive(Clone)]
pub struct FeedHub {
    inner: Arc<Inner>,
}

struct Inner {
    channels: DashMap<Arc<str>, broadcast::Sender<FeedEvent>>,
    capacity: usize,
}

impl FeedHub {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Inner {
                channels: DashMap::new(),
                capacity: capacity.max(1),
            }),
        }
    }

    pub fn subscribe(&self, channel: &str) -> broadcast::Receiver<FeedEvent> {
        if let Some(tx) = self.inner.channels.get(channel) {
            return tx.subscribe();
        }
        let capacity = self.inner.capacity;
        self.inner
            .channels
            .entry(Arc::from(channel))
            .or_insert_with(|| broadcast::channel(capacity).0)
            .subscribe()
    }

    pub fn has_subscribers(&self, channel: &str) -> bool {
        self.inner
            .channels
            .get(channel)
            .is_some_and(|tx| tx.receiver_count() > 0)
    }

    pub fn publish(&self, channel: &str, key: Option<&[u8]>, payload: &[u8]) {
        let (tx, chan) = match self.inner.channels.get(channel) {
            Some(r) if r.value().receiver_count() > 0 => (r.value().clone(), r.key().clone()),
            _ => return,
        };
        let _ = tx.send(FeedEvent {
            channel: chan,
            key: key.map(Bytes::copy_from_slice),
            payload: Bytes::copy_from_slice(payload),
        });
    }

    pub fn gc(&self, channel: &str) {
        self.inner
            .channels
            .remove_if(channel, |_, tx| tx.receiver_count() == 0);
    }

    pub fn channel_count(&self) -> usize {
        self.inner.channels.len()
    }
}

impl Default for FeedHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast::error::RecvError;

    #[tokio::test]
    async fn subscribe_then_publish_delivers() {
        let hub = FeedHub::new();
        let mut rx = hub.subscribe("c");
        hub.publish("c", None, br#"{"x":1}"#);
        let ev = rx.recv().await.expect("event");
        assert_eq!(&*ev.channel, "c");
        assert_eq!(ev.payload.as_ref(), br#"{"x":1}"#);
        assert!(ev.key.is_none());
    }

    #[tokio::test]
    async fn publish_without_subscriber_is_noop() {
        let hub = FeedHub::new();
        hub.publish("nope", None, b"{}"); 
        assert!(!hub.has_subscribers("nope"));
        assert_eq!(hub.channel_count(), 0);
    }

    #[tokio::test]
    async fn publish_after_receiver_dropped_is_noop() {
        let hub = FeedHub::new();
        let rx = hub.subscribe("c");
        drop(rx);
        hub.publish("c", None, b"{}"); 
        assert!(!hub.has_subscribers("c"));
    }

    #[tokio::test]
    async fn lagged_reports_skips_then_resumes() {
        let hub = FeedHub::with_capacity(2);
        let mut rx = hub.subscribe("c");
        for i in 0..5u8 {
            hub.publish("c", None, &[b'0' + i]);
        }
        match rx.recv().await {
            Err(RecvError::Lagged(n)) => assert!(n >= 1),
            other => panic!("expected Lagged, got {other:?}"),
        }
        let ev = rx.recv().await.expect("event after lag");
        assert_eq!(ev.payload.len(), 1);
    }

    #[tokio::test]
    async fn key_roundtrips() {
        let hub = FeedHub::new();
        let mut rx = hub.subscribe("c");
        hub.publish("c", Some(b"k1"), b"{}");
        let ev = rx.recv().await.expect("event");
        assert_eq!(ev.key.as_deref(), Some(b"k1".as_ref()));
    }

    #[tokio::test]
    async fn gc_removes_channel_without_subscribers() {
        let hub = FeedHub::new();
        let rx = hub.subscribe("c");
        assert_eq!(hub.channel_count(), 1);
        drop(rx);
        hub.gc("c");
        assert_eq!(hub.channel_count(), 0);
    }

    #[tokio::test]
    async fn gc_keeps_channel_with_live_subscriber() {
        let hub = FeedHub::new();
        let _rx = hub.subscribe("c");
        hub.gc("c");
        assert_eq!(hub.channel_count(), 1);
    }
}

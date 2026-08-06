use std::hash::{Hash, Hasher};
use std::sync::Arc;

use async_trait::async_trait;
use r_error::runtime::error::RuntimeError;
use serde::Deserialize;
use sonic_rs::Value;
use tokio::sync::mpsc::Sender;

use super::HostFn;

pub struct SendJob {
    pub setting_code: String,
    pub key: Option<Vec<u8>>,
    pub channel: Option<Vec<u8>>,
    pub payload: Vec<u8>,
}

pub struct SendValue {
    pub txs: Arc<[Sender<SendJob>]>,
}

fn shard_for(key: Option<&[u8]>, shards: usize) -> usize {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut h);
    (h.finish() % shards as u64) as usize
}

#[derive(Deserialize)]
struct Req {
    setting_code: String,
    #[serde(default)]
    channel: String,
    #[serde(default)]
    key: String,
    value: Vec<Value>, 
}

#[async_trait]
impl HostFn for SendValue {
    fn name(&self) -> &'static str {
        "send_value"
    }

    async fn call(&self, input: &[u8]) -> Result<Option<Vec<u8>>, RuntimeError> {
        let req: Req =
            sonic_rs::from_slice(input).map_err(|e| RuntimeError::Decode(e.to_string()))?;
        let payload =
            sonic_rs::to_vec(&req.value).map_err(|e| RuntimeError::Payload(e.to_string()))?;

        let channel = (!req.channel.is_empty()).then(|| req.channel.into_bytes());
        let key = (!req.key.is_empty()).then(|| req.key.into_bytes());

        let shard = shard_for(key.as_deref(), self.txs.len());
        self.txs[shard]
            .send(SendJob {
                setting_code: req.setting_code,
                key,
                channel,
                payload,
            })
            .await
            .map_err(|_| RuntimeError::Producer("send queue closed".into()))?;

        Ok(None)
    }
}

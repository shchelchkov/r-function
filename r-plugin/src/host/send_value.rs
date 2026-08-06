use crate::host::slice_from_raw_parts;
use crate::plugin::PluginContext;
use r_error::runtime::error::RuntimeError;
use r_plugin_api::Buffer;
use r_producer::host::send_pipeline::{Req, SendJob, SendValue};
use std::ffi::c_void;
use std::hash::{Hash, Hasher};

fn shard_for(key: Option<&[u8]>, shards: usize) -> usize {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut h);
    (h.finish() % shards as u64) as usize
}

pub unsafe extern "C" fn host_send_value(
    ctx: *mut c_void,
    _setting_code_ptr: *const u8,
    _setting_code_len: usize,
    _key_ptr: *const u8,
    _key_len: usize,
    _channel_ptr: *const u8,
    _channel_len: usize,
    payload_ptr: *const u8,
    payload_len: usize,
) -> Buffer {
    let Some(ctx) = (unsafe { (ctx as *const PluginContext).as_ref() }) else {
        return Buffer::empty();
    };

    let Some(payload) = (unsafe { slice_from_raw_parts(payload_ptr, payload_len) }) else {
        return Buffer::empty();
    };

    if let Err(error) = host_send_value_impl(&ctx.send_value, payload) {
        tracing::error!(%error, "host_send_value failed");
    }

    Buffer::empty()
}

pub fn host_send_value_impl(state: &SendValue, input: &[u8]) -> Result<(), RuntimeError> {
    let req: Req = sonic_rs::from_slice(input).map_err(|e| RuntimeError::Decode(e.to_string()))?;
    let payload = sonic_rs::to_vec(&req.value).map_err(|e| RuntimeError::Payload(e.to_string()))?;

    let channel = (!req.channel.is_empty()).then(|| req.channel.into_bytes());
    let key = (!req.key.is_empty()).then(|| req.key.into_bytes());

    let shard = shard_for(key.as_deref(), state.txs.len());

    let job = SendJob {
        setting_code: req.setting_code,
        key,
        channel,
        payload,
    };

    state.txs[shard]
        .blocking_send(job)
        .map_err(|_| RuntimeError::Producer("send queue closed".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::mpsc;

    fn queue(cap: usize) -> (SendValue, mpsc::Receiver<SendJob>) {
        let (tx, rx) = mpsc::channel(cap);
        let txs = Arc::from(vec![tx]);
        (SendValue { txs }, rx)
    }

    fn req(n: u32) -> Vec<u8> {
        format!(r#"{{"setting_code":"sc","key":"k","value":[{{"n":{n}}}]}}"#).into_bytes()
    }

    fn payload(n: u32) -> Vec<u8> {
        format!(r#"[{{"n":{n}}}]"#).into_bytes()
    }

    #[test]
    fn sends_in_call_order() {
        let (state, mut rx) = queue(8);
        for n in 0..3 {
            host_send_value_impl(&state, &req(n)).unwrap();
        }
        for n in 0..3 {
            let job = rx.try_recv().unwrap();
            assert_eq!(job.payload, payload(n));
            assert_eq!(job.key.as_deref(), Some(&b"k"[..]));
        }
    }

    #[tokio::test]
    async fn full_queue_waits_in_plugin_thread() {
        let (state, mut rx) = queue(1);
        let state = Arc::new(state);
        let send = |n| {
            let state = state.clone();
            tokio::task::spawn_blocking(move || host_send_value_impl(&state, &req(n)))
        };

        send(0).await.unwrap().unwrap();
        let second = send(1);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!second.is_finished(), "очередь полна - поток плагина ждёт");

        assert_eq!(rx.recv().await.unwrap().payload, payload(0));
        second.await.unwrap().unwrap();
        assert_eq!(rx.recv().await.unwrap().payload, payload(1));
    }

    #[test]
    fn closed_queue_is_an_error() {
        let (state, rx) = queue(1);
        drop(rx);
        assert!(matches!(
            host_send_value_impl(&state, &req(0)),
            Err(RuntimeError::Producer(_))
        ));
    }
}

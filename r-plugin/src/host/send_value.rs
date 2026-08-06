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

    let payload = payload.to_vec();

    let state = &ctx.send_value;
    let handle = tokio::runtime::Handle::current();

    handle.spawn(async move {
        if let Err(error) = host_send_value_impl(state, &payload).await {
            tracing::error!(%error, "host_send_value failed");
        }
    });

    Buffer::empty()
}

pub async fn host_send_value_impl(state: &SendValue, input: &[u8]) -> Result<Buffer, RuntimeError> {
    let req: Req =
        sonic_rs::from_slice(input).map_err(|e| RuntimeError::Decode(e.to_string()))?;
    let payload =
        sonic_rs::to_vec(&req.value).map_err(|e| RuntimeError::Payload(e.to_string()))?;

    let channel = (!req.channel.is_empty()).then(|| req.channel.into_bytes());
    let key = (!req.key.is_empty()).then(|| req.key.into_bytes());

    let shard = shard_for(key.as_deref(), state.txs.len());

    let job = SendJob {
        setting_code: req.setting_code,
        key: key,
        channel: channel,
        payload: payload,
    };

    state.txs[shard]
        .send(job)
        .await
        .map_err(|_| RuntimeError::Producer("send queue closed".into()))?;

    Ok(Buffer::empty())
}

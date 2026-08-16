use crate::plugin::PluginContext;
use r_error::runtime::error::RuntimeError;
use r_plugin_api::Buffer;
use r_producer::host::send_pipeline::{SendJob, SendValue};
use std::ffi::c_void;
use std::hash::{Hash, Hasher};


fn shard_for(key: Option<&[u8]>, shards: usize) -> usize {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut h);
    (h.finish() % shards as u64) as usize
}

pub unsafe extern "C" fn host_send_value(
    ctx: *mut c_void,
    setting_code_ptr: *const u8,
    setting_code_len: usize,
    key_ptr: *const u8,
    key_len: usize,
    channel_ptr: *const u8,
    channel_len: usize,
    payload_ptr: *const u8,
    payload_len: usize,
) -> Buffer {
    let ctx = unsafe {
        &*(ctx as *const PluginContext)
    };
    // let Some(ctx) = (unsafe {
    //     (ctx as *const PluginContext).as_ref()
    // }) else {
    //     return Buffer::empty();
    // };

    let Some(payload) = (unsafe {
        slice_from_raw_parts(
            payload_ptr,
            payload_len,
        )
    }) else {
        return Buffer::empty();
    };

    let Some(setting_code) = (unsafe {
        slice_from_raw_parts(
            setting_code_ptr,
            setting_code_len,
        )
    }) else {
        return Buffer::empty();
    };

    let Some(key) = (unsafe {
        slice_from_raw_parts(
            key_ptr,
            key_len,
        )
    }) else {
        return Buffer::empty();
    };

    let Some(channel) = (unsafe {
        slice_from_raw_parts(
            channel_ptr,
            channel_len,
        )
    }) else {
        return Buffer::empty();
    };

    let setting_code = setting_code.to_vec();
    let key = key.to_vec();
    let channel = channel.to_vec();
    let payload = payload.to_vec();

    let state = &ctx.send_value;
    let handle = tokio::runtime::Handle::current();

    handle.spawn(async move {
        if let Err(err) = host_send_value_impl(
            state,
            &setting_code,
            &key,
            &channel,
            &payload,
        )
            .await
        {
            eprintln!("host_send_value failed: {err}");
        }
    });

    Buffer::empty()
}

unsafe fn slice_from_raw_parts<'a>(
    ptr: *const u8,
    len: usize,
) -> Option<&'a [u8]> {
    if ptr.is_null() && len != 0 {
        return None;
    }

    Some(unsafe {
        std::slice::from_raw_parts(ptr, len)
    })
}

pub async fn host_send_value_impl(
    state: &SendValue,
    setting_code: &[u8],
    key: &[u8],
    channel: &[u8],
    payload: &[u8],
) -> Result<Buffer, RuntimeError> {
    let setting_code = String::from_utf8_lossy(setting_code).into_owned();
    let key = key.to_vec();
    let channel = channel.to_vec();

    let shard = shard_for(
        Some(&key),
        state.txs.len(),
    );

    let job = SendJob {
        setting_code,
        key: Some(key),
        channel: Some(channel),
        payload: payload.to_vec(),
    };

    state.txs[shard]
        .send(job)
        .await
        .map_err(|_| RuntimeError::Producer("send queue closed".into()))?;

    Ok(Buffer::empty())
}
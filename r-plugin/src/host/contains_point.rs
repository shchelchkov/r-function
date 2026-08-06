use crate::host::{ctx_setting_code_key, ctx_value};
use crate::plugin::PluginContext;
use r_plugin_api::Buffer;
use std::ffi::c_void;

pub unsafe extern "C" fn host_contains_point(
    ctx: *mut c_void,
    setting_code_ptr: *const u8,
    setting_code_len: usize,
    key_ptr: *const u8,
    key_len: usize,
    value_ptr: *const u8,
    value_len: usize,
) -> Buffer {
    let Some(ctx) = (unsafe { (ctx as *const PluginContext).as_ref() }) else {
        return Buffer::empty();
    };

    let Some((setting_code, key)) =
        (unsafe { ctx_setting_code_key(setting_code_ptr, setting_code_len, key_ptr, key_len) })
    else {
        return Buffer::empty();
    };

    let Some(value) = (unsafe { ctx_value(value_ptr, value_len) }) else {
        return Buffer::empty();
    };

    let Some(found) = ctx
        .polygon
        .contains_point(&setting_code, key.as_str(), &value)
    else {
        return Buffer::empty();
    };

    let mut json = match sonic_rs::to_vec(&found) {
        Ok(json) => json,
        Err(error) => {
            tracing::error!(%error, %setting_code, "contains_point: serialize failed");
            return Buffer::empty();
        }
    };

    let buffer = Buffer {
        ptr: json.as_mut_ptr(),
        len: json.len(),
        capacity: json.capacity(),
    };

    std::mem::forget(json);

    buffer
}

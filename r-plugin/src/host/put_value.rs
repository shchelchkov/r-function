use crate::host::{ctx_setting_code_key, ctx_value};
use crate::plugin::PluginContext;
use std::ffi::c_void;
use std::sync::Arc;

pub unsafe extern "C" fn host_put_value(
    ctx: *mut c_void,
    setting_code_ptr: *const u8,
    setting_code_len: usize,
    key_ptr: *const u8,
    key_len: usize,
    value_ptr: *const u8,
    value_len: usize,
) {
    let ctx = unsafe {
        &*(ctx as *const PluginContext)
    };

    let (setting_code, key) = unsafe {
        ctx_setting_code_key(
            setting_code_ptr,
            setting_code_len,
            key_ptr,
            key_len,
        )
    };

    let value = unsafe {
        ctx_value(
            value_ptr,
            value_len,
        )
    };

    ctx.values.put_value(
        &setting_code,
        Arc::from(key),
        value,
    );
}



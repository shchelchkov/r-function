use std::ffi::c_void;
use std::sync::Arc;

use r_value::value::value::Values;
use crate::plugin::PluginContext;

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

    let setting_code = unsafe {
        std::slice::from_raw_parts(
            setting_code_ptr,
            setting_code_len,
        )
    };

    let key = unsafe {
        std::slice::from_raw_parts(
            key_ptr,
            key_len,
        )
    };

    let value = unsafe {
        std::slice::from_raw_parts(
            value_ptr,
            value_len,
        )
    };

    let setting_code = String::from_utf8_lossy(setting_code);
    let key = String::from_utf8_lossy(key);

    let value: sonic_rs::Value =
        sonic_rs::from_slice(value)
            .expect("invalid JSON value");

    ctx.values.put_value(
        &setting_code,
        Arc::from(key),
        value,
    );
}

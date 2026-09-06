use crate::host::ctx_setting_code_key;
use crate::plugin::PluginContext;
use r_plugin_api::Buffer;
use std::ffi::c_void;

pub unsafe extern "C" fn host_get_function_value(
    ctx: *mut c_void,
    setting_code_ptr: *const u8,
    setting_code_len: usize,
    key_ptr: *const u8,
    key_len: usize,
) -> Buffer {
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
        ctx.function_value.get_function_value(
            &setting_code,
            &key,
        )
    };

    let Some(value) = value else {
        return Buffer {
            ptr: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
    };

    let mut bytes = match sonic_rs::to_vec(value.as_ref()) {
        Ok(bytes) => bytes,
        Err(_) => {
            return Buffer {
                ptr: std::ptr::null_mut(),
                len: 0,
                capacity: 0,
            };
        }
    };

    let buffer = Buffer {
        ptr: bytes.as_mut_ptr(),
        len: bytes.len(),
        capacity: bytes.capacity(),
    };

    std::mem::forget(bytes);

    buffer
}
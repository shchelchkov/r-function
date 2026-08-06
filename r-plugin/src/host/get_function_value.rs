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
    let Some(ctx) = (unsafe { (ctx as *const PluginContext).as_ref() }) else {
        return Buffer::empty();
    };

    let Some((setting_code, key)) =
        (unsafe { ctx_setting_code_key(setting_code_ptr, setting_code_len, key_ptr, key_len) })
    else {
        return Buffer::empty();
    };

    let Some(value) = ctx.function_value.get_function_value(&setting_code, &key) else {
        return Buffer::empty();
    };

    let mut bytes = match sonic_rs::to_vec(value.as_ref()) {
        Ok(bytes) => bytes,
        Err(_) => return Buffer::empty(),
    };

    let buffer = Buffer {
        ptr: bytes.as_mut_ptr(),
        len: bytes.len(),
        capacity: bytes.capacity(),
    };

    std::mem::forget(bytes);

    buffer
}

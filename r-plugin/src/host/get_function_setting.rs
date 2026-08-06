use crate::host::ctx_setting_code;
use crate::plugin::PluginContext;
use r_plugin_api::Buffer;
use std::ffi::c_void;

pub unsafe extern "C" fn host_get_function_setting(
    ctx: *mut c_void,
    setting_code_ptr: *const u8,
    setting_code_len: usize,
) -> Buffer {
    let Some(ctx) = (unsafe { (ctx as *const PluginContext).as_ref() }) else {
        return Buffer::empty();
    };

    let Some(setting_code) = (unsafe { ctx_setting_code(setting_code_ptr, setting_code_len) })
    else {
        return Buffer::empty();
    };

    let Some(value) = ctx.function.get_function_setting(&setting_code) else {
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

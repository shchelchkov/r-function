use std::ffi::c_void;

use crate::host::ctx_setting_code_key;
use crate::plugin::PluginContext;
use r_plugin_api::Buffer;

pub unsafe extern "C" fn host_get_value(
    ctx: *mut c_void,
    setting_code_ptr: *const u8,
    setting_code_len: usize,
    key_ptr: *const u8,
    key_len: usize,
) -> Buffer {
    unsafe {
        get_value(
            ctx,
            setting_code_ptr,
            setting_code_len,
            key_ptr,
            key_len,
            None,
        )
    }
}

pub unsafe extern "C" fn host_get_value_at(
    ctx: *mut c_void,
    setting_code_ptr: *const u8,
    setting_code_len: usize,
    key_ptr: *const u8,
    key_len: usize,
    at_ms: i64,
) -> Buffer {
    unsafe {
        get_value(
            ctx,
            setting_code_ptr,
            setting_code_len,
            key_ptr,
            key_len,
            Some(at_ms),
        )
    }
}

unsafe fn get_value(
    ctx: *mut c_void,
    setting_code_ptr: *const u8,
    setting_code_len: usize,
    key_ptr: *const u8,
    key_len: usize,
    at: Option<i64>,
) -> Buffer {
    let Some(ctx) = (unsafe { (ctx as *const PluginContext).as_ref() }) else {
        return Buffer::empty();
    };

    let Some((setting_code, key)) =
        (unsafe { ctx_setting_code_key(setting_code_ptr, setting_code_len, key_ptr, key_len) })
    else {
        return Buffer::empty();
    };

    let found = match at {
        Some(at) => ctx.values.get_value_at(&setting_code, &key, at),
        None => ctx.values.get_value(&setting_code, &key),
    };
    let mut json = match found {
        Ok(Some(v)) => match sonic_rs::to_vec(&*v) {
            Ok(json) => json,
            Err(_) => return Buffer::empty(),
        },

        Ok(None) => return Buffer::empty(),

        Err(_) => return Buffer::empty(),
    };

    let buffer = Buffer {
        ptr: json.as_mut_ptr(),
        len: json.len(),
        capacity: json.capacity(),
    };

    std::mem::forget(json);

    buffer
}

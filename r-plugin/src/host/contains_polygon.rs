use crate::host::{ctx_key, ctx_value};
use crate::plugin::PluginContext;
use r_plugin_api::Buffer;
use std::ffi::c_void;

pub unsafe extern "C" fn host_contains_polygon(
    ctx: *mut c_void,
    setting_code_ptr: *const u8,
    setting_code_len: usize,
    key_ptr: *const u8,
    key_len: usize,
    value_ptr: *const u8,
    value_len: usize,
) -> Buffer {
    let ctx = unsafe {
        &*(ctx as *const PluginContext)
    };

    let (setting_code, key) = unsafe {
        ctx_key(
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

    let mut json = match ctx.polygon.contains_polygon(
        &setting_code,
        key.as_str(),
        &value,
    ) {
        Some(v) => {
            sonic_rs::to_vec(&v)
                .expect("serialize value")
        }

        None => {
            return Buffer {
                ptr: std::ptr::null_mut(),
                len: 0,
                capacity: 0,
            };
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



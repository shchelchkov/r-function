use std::ffi::c_void;

use r_plugin_api::Buffer;
use r_value::value::value::Values;
use crate::plugin::PluginContext;

pub unsafe extern "C" fn host_get_value(
    ctx: *mut c_void,
    setting_code_ptr: *const u8,
    setting_code_len: usize,
    key_ptr: *const u8,
    key_len: usize,
) -> Buffer {
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

    let setting_code = String::from_utf8_lossy(setting_code);
    let key = String::from_utf8_lossy(key);

    let mut json = match ctx.values.get_value(
        &setting_code,
        &key,
    ) {
        Some(value) => {
            sonic_rs::to_vec(value.as_ref())
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

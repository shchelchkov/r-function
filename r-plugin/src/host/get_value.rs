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
    let Some(ctx) = (unsafe { (ctx as *const PluginContext).as_ref() }) else {
        return Buffer::empty();
    };

    let Some((setting_code, key)) =
        (unsafe { ctx_setting_code_key(setting_code_ptr, setting_code_len, key_ptr, key_len) })
    else {
        return Buffer::empty();
    };

    let mut json = match ctx.values.get_value(&setting_code, &key) {
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

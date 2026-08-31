use crate::host::contains_point::host_contains_point;
use crate::host::contains_polygon::host_contains_polygon;
use crate::host::free_buffer::host_free_buffer;
use crate::host::get_value::host_get_value;
use crate::host::put_polygon::host_put_polygon;
use crate::host::put_value::host_put_value;
use crate::host::remove_polygon::host_remove_polygon;
use crate::host::send_value::host_send_value;
use crate::plugin::PluginContext;
use r_plugin_api::HostApi;
use sonic_rs::Value;
use std::ffi::c_void;

mod free_buffer;
mod get_value;
mod put_value;
mod send_value;
mod remove_polygon;
mod contains_polygon;
mod contains_point;
mod put_polygon;

pub fn create_api(context: &PluginContext) -> HostApi {
    HostApi {
        ctx: context as *const PluginContext as *mut c_void,
        put_value: host_put_value,
        get_value: host_get_value,
        send_value: host_send_value,
        contains_point: host_contains_point,
        contains_polygon: host_contains_polygon,
        put_polygon: host_put_polygon,
        remove_polygon: host_remove_polygon,
        free_buffer: host_free_buffer,
    }
}

pub unsafe fn ctx_key(
    setting_code_ptr: *const u8,
    setting_code_len: usize,
    key_ptr: *const u8,
    key_len: usize,
) -> (String, String) {
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

    let setting_code = String::from_utf8_lossy(setting_code).into_owned();
    let key = String::from_utf8_lossy(key).into_owned();

    (setting_code, key)
}

pub unsafe fn ctx_value(
    value_ptr: *const u8,
    value_len: usize,
) -> Value {
    let value = unsafe {
        std::slice::from_raw_parts(
            value_ptr,
            value_len,
        )
    };

    let value: sonic_rs::Value =
        sonic_rs::from_slice(value)
            .expect("invalid JSON value");

    value
}
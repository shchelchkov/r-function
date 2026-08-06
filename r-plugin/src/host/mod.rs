use crate::host::append_value::host_append_value;
use crate::host::contains_point::host_contains_point;
use crate::host::contains_polygon::host_contains_polygon;
use crate::host::free_buffer::host_free_buffer;
use crate::host::get_function_setting::host_get_function_setting;
use crate::host::get_function_value::host_get_function_value;
use crate::host::get_value::host_get_value;
use crate::host::insert_value::host_insert_value;
use crate::host::put_polygon::host_put_polygon;
use crate::host::put_value::host_put_value;
use crate::host::remove_polygon::host_remove_polygon;
use crate::host::send_value::host_send_value;
use crate::plugin::PluginContext;
use r_plugin_api::HostApi;
use sonic_rs::Value;
use std::ffi::c_void;

mod append_value;
mod contains_point;
mod contains_polygon;
mod free_buffer;
mod get_function_setting;
mod get_function_value;
mod get_value;
mod insert_value;
mod put_polygon;
mod put_value;
mod remove_polygon;
mod send_value;

pub fn create_api(context: &PluginContext) -> HostApi {
    HostApi {
        ctx: context as *const PluginContext as *mut c_void,
        put_value: host_put_value,
        insert_value: host_insert_value,
        append_value: host_append_value,
        get_value: host_get_value,
        send_value: host_send_value,
        contains_point: host_contains_point,
        contains_polygon: host_contains_polygon,
        put_polygon: host_put_polygon,
        remove_polygon: host_remove_polygon,
        get_function_value: host_get_function_value,
        get_function_setting: host_get_function_setting,
        free_buffer: host_free_buffer,
    }
}

pub(crate) unsafe fn ctx_setting_code_key(
    setting_code_ptr: *const u8,
    setting_code_len: usize,
    key_ptr: *const u8,
    key_len: usize,
) -> Option<(String, String)> {
    let setting_code = unsafe { slice_from_raw_parts(setting_code_ptr, setting_code_len) }?;
    let key = unsafe { slice_from_raw_parts(key_ptr, key_len) }?;

    Some((
        String::from_utf8_lossy(setting_code).into_owned(),
        String::from_utf8_lossy(key).into_owned(),
    ))
}

pub(crate) unsafe fn ctx_setting_code(
    setting_code_ptr: *const u8,
    setting_code_len: usize,
) -> Option<String> {
    let setting_code = unsafe { slice_from_raw_parts(setting_code_ptr, setting_code_len) }?;

    Some(String::from_utf8_lossy(setting_code).into_owned())
}

pub(crate) unsafe fn ctx_value(value_ptr: *const u8, value_len: usize) -> Option<Value> {
    let value = unsafe { slice_from_raw_parts(value_ptr, value_len) }?;

    sonic_rs::from_slice(value).ok()
}

pub(crate) unsafe fn ctx_u64(value_ptr: *const u8, value_len: usize) -> Option<u64> {
    let bytes = unsafe { slice_from_raw_parts(value_ptr, value_len) }?;
    let bytes: [u8; 8] = bytes.try_into().ok()?;

    Some(u64::from_le_bytes(bytes))
}

pub(crate) unsafe fn slice_from_raw_parts<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    if len == 0 {
        return Some(&[]);
    }

    if ptr.is_null() {
        return None;
    }

    Some(unsafe { std::slice::from_raw_parts(ptr, len) })
}

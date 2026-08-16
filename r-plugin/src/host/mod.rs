use crate::host::free_buffer::host_free_buffer;
use crate::host::get_value::host_get_value;
use crate::host::put_value::host_put_value;
use crate::host::send_value::host_send_value;
use crate::plugin::PluginContext;
use r_plugin_api::HostApi;
use std::ffi::c_void;

mod free_buffer;
mod get_value;
mod put_value;
mod send_value;

pub fn create_api(context: &PluginContext) -> HostApi {
    HostApi {
        ctx: context as *const PluginContext as *mut c_void,
        put_value: host_put_value,
        get_value: host_get_value,
        send_value: host_send_value,
        free_buffer: host_free_buffer,
    }
}

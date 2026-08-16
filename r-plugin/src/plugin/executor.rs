use std::sync::Arc;
use crate::host::create_api;
use crate::plugin::plugin::Plugin;
use r_error::runtime::error::RuntimeError;
use r_value::value::value::Values;
use crate::plugin::PluginContext;

pub struct PluginExecutor {}

impl PluginExecutor {
    pub fn new() -> Result<Self, RuntimeError> {
        Ok(Self {})
    }

    pub async fn run(
        &self,
        plugin: &Plugin,
        payload: Vec<u8>,
        ctx: Arc<PluginContext>,
    ) -> Result<Vec<u8>, RuntimeError> {
        let api = create_api(&ctx);
        let result = unsafe {
            plugin.process(&api, &payload)
        };

        let output = if result.ptr.is_null() || result.len == 0 {
            Vec::new()
        } else {
            unsafe {
                std::slice::from_raw_parts(result.ptr, result.len).to_vec()
            }
        };

        unsafe {
            plugin.free_buffer(result);
        }

        Ok(output)
    }

}

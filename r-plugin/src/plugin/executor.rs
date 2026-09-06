use crate::host::create_api;
use crate::plugin::PluginContext;
use crate::plugin::plugin::Plugin;
use r_plugin_api::{Buffer, PluginError};
use std::sync::Arc;
use r_error::runtime::error::RuntimeError;

pub struct PluginExecutor {}

impl PluginExecutor {
    pub fn new() -> Result<Self, PluginError> {
        Ok(Self {})
    }

    pub async fn run(
        &self,
        plugin: Arc<Plugin>,
        payload: Vec<u8>,
        ctx: Arc<PluginContext>,
    ) -> Result<Vec<u8>, RuntimeError> {
        let api = create_api(&ctx);

        let output = tokio::task::spawn_blocking(move || unsafe {
            let result = plugin.process(&api, &payload);

            let output = buffer_to_vec(result);

            plugin.free_buffer(result);

            output
        })
            .await
            .map_err(|e| {
                RuntimeError::Trap(format!("plugin execution failed: {e}"))
            })?;

        Ok(output)
    }

}

fn buffer_to_vec(result: Buffer) -> Vec<u8> {
    if result.ptr.is_null() || result.len == 0 {
        return Vec::new();
    }

    unsafe {
        std::slice::from_raw_parts(result.ptr, result.len).to_vec()
    }
}
use r_error::process::error::ProcessError;
use r_plugin_api::Plugin;
use r_runtime_api::Runtime;
use r_setting::functions::function_setting::FunctionSetting;
use sonic_rs::Value;
use std::sync::Arc;

pub(crate) async fn execute_batch(
    runtime: &Arc<dyn Runtime>,
    batch: &[Value],
    function_settings: &[FunctionSetting],
) -> Result<(), ProcessError> {
    for fs in function_settings {
        if !fs.is_active() || fs.key().is_none() {
            continue;
        }

        let mut modules = fs
            .modules()
            .unwrap_or(&[])
            .iter()
            .map(String::as_str)
            .filter(|module| !module.is_empty());

        let Some(first) = modules.next() else {
            continue;
        };

        let mut current = runtime.invoke(first, batch).await.map_err(|e| {
            tracing::error!(error = %e,module = first,transient = e.is_transient(),"wasm invoke failed");
            ProcessError::Runtime(e)
        })?;

        for module in modules {
            current = runtime.invoke(module, &current).await.map_err(|e| {
                tracing::error!(error = %e,module,transient = e.is_transient(),"wasm invoke failed");
                ProcessError::Runtime(e)
            })?;
        }
    }

    Ok(())
}

pub(crate) async fn execute_plugin(
    plugin: &Arc<dyn Plugin>,
    batch: &[Value],
    function_settings: &[FunctionSetting],
) -> Result<(), ProcessError> {
    for fs in function_settings {
        if !fs.is_active() || fs.key().is_none() {
            continue;
        }

        let mut plugins = fs
            .plugin()
            .unwrap_or(&[])
            .iter()
            .map(String::as_str)
            .filter(|module| !module.is_empty());

        let Some(first) = plugins.next() else {
            continue;
        };

        let mut current = plugin.invoke(first, batch).await.map_err(|e| {
            tracing::error!(error = %e,module = first,transient = e.is_transient(),"plugin invoke failed");
            ProcessError::Plugin(e)
        })?;

        for module in plugins {
            current = plugin.invoke(module, &current).await.map_err(|e| {
                tracing::error!(error = %e,module,transient = e.is_transient(),"plugin invoke failed");
                ProcessError::Plugin(e)
            })?;
        }
    }

    Ok(())
}

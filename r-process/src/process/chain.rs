use r_error::plugin::error::PluginError;
use r_error::process::error::ProcessError;
use r_error::runtime::error::RuntimeError;
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

        let modules: Vec<&str> = fs
            .modules()
            .unwrap_or(&[])
            .iter()
            .map(String::as_str)
            .filter(|module| !module.is_empty())
            .collect();

        let Some((&last, init)) = modules.split_last() else {
            continue;
        };

        let mut current: Option<Vec<Value>> = None;
        for &module in init {
            let input = current.as_deref().unwrap_or(batch);
            current = Some(
                runtime
                    .invoke(module, input)
                    .await
                    .map_err(|e| wasm_failed(module, e))?,
            );
        }

        let input = current.as_deref().unwrap_or(batch);
        runtime
            .invoke_discard(last, input)
            .await
            .map_err(|e| wasm_failed(last, e))?;
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

        let plugins: Vec<&str> = fs
            .plugin()
            .unwrap_or(&[])
            .iter()
            .map(String::as_str)
            .filter(|module| !module.is_empty())
            .collect();

        let Some((&last, init)) = plugins.split_last() else {
            continue;
        };

        let mut current: Option<Vec<Value>> = None;
        for &module in init {
            let input = current.as_deref().unwrap_or(batch);
            current = Some(
                plugin
                    .invoke(module, input)
                    .await
                    .map_err(|e| plugin_failed(module, e))?,
            );
        }

        let input = current.as_deref().unwrap_or(batch);
        plugin
            .invoke_discard(last, input)
            .await
            .map_err(|e| plugin_failed(last, e))?;
    }

    Ok(())
}

fn wasm_failed(module: &str, e: RuntimeError) -> ProcessError {
    tracing::error!(error = %e,module,transient = e.is_transient(),"wasm invoke failed");
    ProcessError::Runtime(e)
}

fn plugin_failed(module: &str, e: PluginError) -> ProcessError {
    tracing::error!(error = %e,module,transient = e.is_transient(),"plugin invoke failed");
    ProcessError::Plugin(e)
}

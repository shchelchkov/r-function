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
) -> Result<Option<Vec<u8>>, ProcessError> {
    for fs in function_settings.iter() {
        if !fs.is_active() {
            continue;
        }
        if fs.key().is_none() {
            continue;
        }

        let modules: Vec<&str> = fs
            .module()
            .unwrap_or(&[])
            .iter()
            .map(String::as_str)
            .filter(|m| !m.is_empty())
            .collect();
        let Some((&first, rest)) = modules.split_first() else {
            continue;
        };
        let payload = sonic_rs::to_vec(batch).map_err(|e| ProcessError::Payload(e.to_string()))?;
        let mut current = runtime.run_runtime(first, payload).await.map_err(|e| {
            tracing::error!(error = %e, module = first, transient = e.is_transient(), "wasm invoke failed");
            ProcessError::Runtime(e)
        })?;
        for &module in rest {
            current = runtime.run_runtime(module, current).await.map_err(|e| {
                tracing::error!(error = %e, module, transient = e.is_transient(), "wasm invoke failed");
                ProcessError::Runtime(e)
            })?;
        }


        return Ok(Some(current));
    }
    Ok(None)
}

pub(crate) async fn execute_plugin(
    plugin: &Arc<dyn Plugin>,
    batch: &[Value],
    function_settings: &[FunctionSetting],
) -> Result<Option<Vec<u8>>, ProcessError> {
    for fs in function_settings.iter() {
        if !fs.is_active() {
            continue;
        }
        if fs.key().is_none() {
            continue;
        }

        let plugins: Vec<&str> = fs
            .plugin()
            .unwrap_or(&[])
            .iter()
            .map(String::as_str)
            .filter(|m| !m.is_empty())
            .collect();
        let Some((&first, rest)) = plugins.split_first() else {
            continue;
        };
        let payload = sonic_rs::to_vec(batch).map_err(|e| ProcessError::Payload(e.to_string()))?;
        let mut current = plugin.run_plugin(first, payload).await
            .map_err(|e| {
                tracing::error!(error = %e, module = first, transient = e.is_transient(), "plugin:::::::::::::::::::::::::::::::::: invoke failed");
            ProcessError::Runtime(e)
        })?;
        for &module in rest {
            current = plugin.run_plugin(module, current).await
                .map_err(|e| {
                    tracing::error!(error = %e, module, transient = e.is_transient(), "plugin:::::::::::::::::::::::::::::::::: invoke failed");
                ProcessError::Runtime(e)
            })?;
        }


        return Ok(Some(current));
    }
    Ok(None)
}

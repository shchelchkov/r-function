use r_error::process::error::ProcessError;
use r_setting::functions::function_setting::FunctionSetting;
use sonic_rs::JsonValueTrait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::process::Message;
use crate::process::grouping::Resolved;
use crate::process::provider::SettingProvider;

pub(crate) async fn resolve_all(
    function: &Arc<dyn SettingProvider>,
    msgs: &[Message],
) -> HashMap<String, Resolved> {
    let mut resolved: HashMap<String, Resolved> = HashMap::new();
    for msg in msgs {
        let Some(raw) = msg.payload.as_deref() else {
            continue;
        };
        let Some(sc) = sonic_rs::get_from_slice(raw, &["setting_code"])
            .as_str()
            .map(str::to_owned)
        else {
            continue;
        };
        if !resolved.contains_key(&sc) {
            let r = resolve(function, &sc).await;
            resolved.insert(sc, r);
        }
    }
    resolved
}

async fn resolve(function: &Arc<dyn SettingProvider>, setting_code: &str) -> Resolved {
    if let Some(settings) = function.get_cached_setting(setting_code) {
        return match function.get_value_key(setting_code) {
            Some(value_key) => Resolved::Ready(settings, value_key),
            None => Resolved::Skip,
        };
    }

    match resolve_setting(function, setting_code).await {
        Ok(Some((settings, Some(value_key)))) => Resolved::Ready(settings, value_key),
        Ok(Some((_, None))) | Ok(None) => Resolved::Skip,
        Err(e) => {
            tracing::warn!(error = %e, setting_code, "resolve_setting failed (transient)");
            Resolved::Failed
        }
    }
}

async fn resolve_setting(
    function: &Arc<dyn SettingProvider>,
    setting_code: &str,
) -> Result<Option<(Arc<Vec<FunctionSetting>>, Option<Arc<Vec<String>>>)>, ProcessError> {
    let function = function.clone();
    let sc = setting_code.to_owned();

    tokio::task::spawn_blocking(move || {
        function
            .get_function_setting(&sc)
            .map(|fs| (fs, function.get_value_key(&sc)))
    })
    .await
    .map_err(|e| ProcessError::Producer(e.to_string()))
}

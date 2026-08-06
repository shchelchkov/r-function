use r_error::process::error::ProcessError;
use r_setting::functions::function_setting::FunctionSetting;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;

use crate::process::Message;
use crate::process::grouping::Resolved;
use crate::process::provider::SettingProvider;

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error(transparent)]
    Value(#[from] value_rs::ValueRsError),
    #[error("function setting `{0}` not resolved")]
    Unresolved(String),
}

#[derive(Clone)]
pub struct Resolver {
    provider: Arc<dyn SettingProvider>,
}

impl Resolver {
    pub fn new(provider: Arc<dyn SettingProvider>) -> Self {
        Self { provider }
    }

    pub async fn resolve_value(&self, raw: &[u8]) -> Result<Option<value_rs::Value>, ResolveError> {
        let Some(sc) = value_rs::setting_code_from_raw(raw) else {
            return Err(value_rs::ValueRsError::SettingCodeNotFound.into());
        };
        match self.resolve(&sc).await {
            Resolved::Ready(_, value_key) => {
                Ok(value_rs::from_slice_and_build_key(raw, &value_key, &sc)?)
            }
            Resolved::Skip => Ok(None),
            Resolved::Failed => Err(ResolveError::Unresolved(sc)),
        }
    }

    pub(crate) async fn resolve_all(&self, msgs: &[Message]) -> HashMap<String, Resolved> {
        let mut resolved: HashMap<String, Resolved> = HashMap::new();
        for msg in msgs {
            let Some(sc) = setting_code(msg) else {
                continue;
            };
            if let Entry::Vacant(slot) = resolved.entry(sc) {
                let r = self.resolve(slot.key()).await;
                slot.insert(r);
            }
        }
        resolved
    }

    async fn resolve(&self, setting_code: &str) -> Resolved {
        if let Some(settings) = self.provider.get_cached_setting(setting_code) {
            return match self.provider.get_value_key(setting_code) {
                Some(value_key) => Resolved::Ready(settings, value_key),
                None => Resolved::Skip,
            };
        }

        match self.resolve_setting(setting_code).await {
            Ok(Some((settings, Some(value_key)))) => Resolved::Ready(settings, value_key),
            Ok(Some((_, None))) | Ok(None) => Resolved::Skip,
            Err(e) => {
                tracing::warn!(error = %e, setting_code, "resolve_setting failed (transient)");
                Resolved::Failed
            }
        }
    }

    async fn resolve_setting(
        &self,
        setting_code: &str,
    ) -> Result<Option<(Arc<Vec<FunctionSetting>>, Option<Arc<Vec<String>>>)>, ProcessError> {
        let provider = self.provider.clone();
        let sc = setting_code.to_owned();

        tokio::task::spawn_blocking(move || {
            provider
                .get_function_setting(&sc)
                .map(|fs| (fs, provider.get_value_key(&sc)))
        })
        .await
        .map_err(|e| ProcessError::Producer(e.to_string()))
    }
}

fn setting_code(msg: &Message) -> Option<String> {
    msg.payload
        .as_deref()?
        .first()
        .map(|v| v.setting_code.to_string())
}

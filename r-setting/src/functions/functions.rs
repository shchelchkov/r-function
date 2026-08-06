use dashmap::DashMap;
use gix::ObjectId;
use r_config::config::FunctionConfig;
use std::sync::Arc;

use crate::functions::function_setting::FunctionSetting;
use crate::git::setting_store::SettingStore;
use crate::git::{GitHandle, HeadObserver, fetch_setting};

#[derive(Clone)]
pub struct Function {
    shared: Arc<Shared>,
}

struct Shared {
    settings: SettingStore<FunctionSetting>,
    value_key: DashMap<String, Arc<Vec<String>>>,
}

impl Function {
    pub fn new(git: Arc<GitHandle>, function_config: &FunctionConfig) -> Function {
        let shared = Arc::new(Shared {
            settings: SettingStore::new(
                git,
                function_config.git_function_settings.clone(),
                "function setting",
            ),
            value_key: DashMap::new(),
        });

        Function { shared }
    }

    pub fn settings_store(&self) -> SettingStore<FunctionSetting> {
        self.shared.settings.clone()
    }

    pub fn get_function_setting(&self, setting_code: &str) -> Option<Arc<Vec<FunctionSetting>>> {
        self.shared
            .settings
            .get_or_load(setting_code, fetch_setting)
    }

    pub fn get_cached_setting(&self, setting_code: &str) -> Option<Arc<Vec<FunctionSetting>>> {
        self.shared.settings.get(setting_code)
    }

    pub fn get_value_key(&self, setting_code: &str) -> Option<Arc<Vec<String>>> {
        if let Some(v) = self.shared.value_key.get(setting_code) {
            return Some(Arc::clone(&*v));
        }

        if let Some(settings) = self.get_function_setting(setting_code) {
            let k: Vec<String> = settings
                .iter()
                .filter(|s| s.is_key())
                .filter_map(|s| s.key().map(str::to_owned))
                .collect();
            if !k.is_empty() {
                let kv = Arc::new(k);
                self.shared
                    .value_key
                    .insert(setting_code.into(), kv.clone());
                Some(kv)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn set_function_setting(
        &self,
        catalog_setting: &str,
        function_settings: Vec<FunctionSetting>,
    ) {
        self.shared.settings.set(catalog_setting, function_settings);
    }
}

impl HeadObserver for Function {
    fn on_revision_changed(&self, _new_head: ObjectId, changed_paths: &[String]) {
        self.shared.settings.invalidate(changed_paths);
    }
}

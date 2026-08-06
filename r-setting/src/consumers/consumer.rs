use gix::ObjectId;
use r_config::config::FunctionConfig;
use std::sync::Arc;

use crate::consumers::consumer_setting::ConsumerSetting;
use crate::git::setting_store::SettingStore;
use crate::git::{GitHandle, HeadObserver, fetch_setting};

#[derive(Clone)]
pub struct Consumer {
    settings: SettingStore<ConsumerSetting>,
}

impl Consumer {
    pub fn new(git: Arc<GitHandle>, function_config: &FunctionConfig) -> Consumer {
        Consumer {
            settings: SettingStore::new(
                git,
                function_config.git_consumer_setting.clone(),
                "consumer setting",
            ),
        }
    }

    pub fn get_consumer_setting(&self, setting_code: &str) -> Option<Arc<Vec<ConsumerSetting>>> {
        self.settings.get_or_load(setting_code, fetch_setting)
    }

    pub fn set_consumer_setting(
        &self,
        setting_code: &str,
        consumer_settings: Vec<ConsumerSetting>,
    ) {
        self.settings.set(setting_code, consumer_settings);
    }
}

impl HeadObserver for Consumer {
    fn on_revision_changed(&self, _new_head: ObjectId, changed_paths: &[String]) {
        self.settings.invalidate(changed_paths);
    }
}

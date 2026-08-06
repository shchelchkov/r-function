use gix::ObjectId;
use r_config::config::FunctionConfig;
use std::sync::Arc;

use crate::git::setting_store::SettingStore;
use crate::git::{GitHandle, HeadObserver, fetch_setting};
use crate::streams::stream_setting::StreamSetting;

#[derive(Clone)]
pub struct Stream {
    settings: SettingStore<StreamSetting>,
}

impl Stream {
    pub fn new(git: Arc<GitHandle>, function_config: &FunctionConfig) -> Stream {
        Stream {
            settings: SettingStore::new(
                git,
                function_config.git_stream_setting.clone(),
                "stream setting",
            ),
        }
    }

    pub fn get_stream_setting(&self, setting_code: &str) -> Option<Arc<Vec<StreamSetting>>> {
        self.settings.get_or_load(setting_code, fetch_setting)
    }

    pub fn set_stream_setting(&self, catalog_setting: &str, stream_settings: Vec<StreamSetting>) {
        self.settings.set(catalog_setting, stream_settings);
    }
}

impl HeadObserver for Stream {
    fn on_revision_changed(&self, _new_head: ObjectId, changed_paths: &[String]) {
        self.settings.invalidate(changed_paths);
    }
}

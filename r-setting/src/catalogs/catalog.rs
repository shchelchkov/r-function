use gix::ObjectId;
use r_config::config::FunctionConfig;
use std::sync::Arc;

use crate::catalogs::catalog_setting::CatalogSetting;
use crate::git::setting_store::SettingStore;
use crate::git::{GitHandle, HeadObserver, fetch_setting};

#[derive(Clone)]
pub struct Catalog {
    settings: SettingStore<CatalogSetting>,
}

impl Catalog {
    pub fn new(git: Arc<GitHandle>, function_config: &FunctionConfig) -> Catalog {
        Catalog {
            settings: SettingStore::new(
                git,
                function_config.git_catalog_setting.clone(),
                "catalog setting",
            ),
        }
    }

    pub fn get_catalog_setting(&self, setting_code: &str) -> Option<Arc<Vec<CatalogSetting>>> {
        self.settings.get_or_load(setting_code, fetch_setting)
    }

    pub fn set_catalog_setting(&self, setting_code: &str, catalog_settings: Vec<CatalogSetting>) {
        self.settings.set(setting_code, catalog_settings);
    }
}

impl HeadObserver for Catalog {
    fn on_revision_changed(&self, _new_head: ObjectId, changed_paths: &[String]) {
        self.settings.invalidate(changed_paths);
    }
}

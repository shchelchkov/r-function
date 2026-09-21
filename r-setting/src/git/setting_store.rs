use std::sync::Arc;

use dashmap::DashMap;
use gix::ObjectId;
use gix::Repository;
use r_error::runtime::error::RuntimeError;
use serde::de::DeserializeOwned;
use tracing::{info, warn};

use crate::git::GitHandle;

pub struct SettingStore<T> {
    shared: Arc<StoreShared<T>>,
}

impl<T> Clone for SettingStore<T> {
    fn clone(&self) -> Self {
        SettingStore {
            shared: Arc::clone(&self.shared),
        }
    }
}

struct StoreShared<T> {
    git: Arc<GitHandle>,
    git_json_path: Arc<str>,
    def_setting_code: Arc<str>,
    kind: &'static str,
    cache: DashMap<String, Arc<Vec<T>>>,
}

impl<T: DeserializeOwned + Send + Sync + 'static> SettingStore<T> {
    pub fn new(
        git: Arc<GitHandle>,
        git_json_path: impl Into<Arc<str>>,
        def_setting_code: impl Into<Arc<str>>,
        kind: &'static str,
    ) -> SettingStore<T> {
        SettingStore {
            shared: Arc::new(StoreShared {
                git,
                git_json_path: git_json_path.into(),
                def_setting_code: def_setting_code.into(),
                kind,
                cache: DashMap::new(),
            }),
        }
    }

    pub fn def_setting_code(&self) -> &str {
        &self.shared.def_setting_code
    }

    pub fn get(&self, code: &str) -> Option<Arc<Vec<T>>> {
        self.shared.cache.get(code).map(|v| Arc::clone(&*v))
    }

            pub fn get_or_default(&self, code: &str) -> Option<Arc<Vec<T>>> {
        self.get(code)
            .or_else(|| self.get(&self.shared.def_setting_code))
    }

    pub fn get_or_load<F>(&self, value_code: &str, loader: F) -> Option<Arc<Vec<T>>>
    where
        F: FnOnce(&Repository, &str) -> Result<(ObjectId, Vec<T>), RuntimeError>,
    {
        if let Some(v) = self.get(value_code) {
            return Some(v);
        }

        let oid = **self.shared.git.head.load();
        let spec = format!("{}:{}/{}.json", oid, self.shared.git_json_path, value_code);
        info!(
            "SettingStore.get_or_load:::::::::::: spec {:?} loader",
            &spec
        );

        match loader(&self.shared.git.repo().to_thread_local(), &spec) {
            Ok((_oid, items)) => {
                info!(
                    "SettingStore.get_or_load:::::::::::: spec {:?} = Items",
                    &spec
                );
                let r = Arc::new(items);
                self.shared.cache.insert(value_code.into(), r.clone());
                Some(r)
            }
            Err(e) => {
                warn!(
                    error = %e,
                    "SettingStore.get_or_load:::::::::::: spec {:?} = None",
                    &spec
                );
                None
            }
        }
    }

    pub fn set(&self, code: &str, items: Vec<T>) {
        self.shared.cache.insert(code.to_string(), items.into());
    }

    pub fn invalidate(&self, changed_paths: &[String]) {
        let prefix = format!("{}/", self.shared.git_json_path);
        for path in changed_paths {
            if let Some(rest) = path.strip_prefix(&prefix)
                && let Some(key) = rest.strip_suffix(".json")
            {
                info!(
                    kind = self.shared.kind,
                    key, "::::::SETTING CACHE INVALIDATED::::::"
                );
                self.shared.cache.remove(key);
            }
        }
    }

    pub fn entries(&self) -> Vec<(String, Arc<Vec<T>>)> {
        self.shared
            .cache
            .iter()
            .map(|e| (e.key().clone(), Arc::clone(e.value())))
            .collect()
    }
}

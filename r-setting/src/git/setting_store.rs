use std::sync::Arc;

use dashmap::DashMap;
use gix::ObjectId;
use gix::Repository;
use r_error::runtime::error::RuntimeError;
use serde::de::DeserializeOwned;
use tracing::{error, info};

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
    kind: &'static str,
    cache: DashMap<String, Arc<Vec<T>>>,
}

impl<T: DeserializeOwned + Send + Sync + 'static> SettingStore<T> {
    pub fn new(
        git: Arc<GitHandle>,
        git_json_path: impl Into<Arc<str>>,
        kind: &'static str,
    ) -> SettingStore<T> {
        SettingStore {
            shared: Arc::new(StoreShared {
                git,
                git_json_path: git_json_path.into(),
                kind,
                cache: DashMap::new(),
            }),
        }
    }

    pub fn get(&self, code: &str) -> Option<Arc<Vec<T>>> {
        self.shared.cache.get(code).map(|v| Arc::clone(&*v))
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
            Err(_e) => {
                error!(
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
}

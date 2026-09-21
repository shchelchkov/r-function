use dashmap::DashMap;
use gix::ObjectId;
use r_config::config::FunctionConfig;
use std::sync::Arc;
use tracing::info;

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
                function_config.def_setting_code.clone(),
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
        let settings = &self.shared.settings;
        settings
            .get_or_load(setting_code, fetch_setting)
            .or_else(|| {
                let def = settings.def_setting_code();
                if def == setting_code {
                    return None;
                }
                info!(
                    setting_code,
                    def, "function setting not found, using default"
                );
                settings.get_or_load(def, fetch_setting)
            })
    }

            pub fn get_cached_setting(&self, setting_code: &str) -> Option<Arc<Vec<FunctionSetting>>> {
        self.shared.settings.get_or_default(setting_code)
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

    pub fn entries(&self) -> Vec<(String, Arc<Vec<FunctionSetting>>)> {
        self.shared.settings.entries()
    }
}

impl HeadObserver for Function {
    fn on_revision_changed(&self, _new_head: ObjectId, changed_paths: &[String]) {
        self.shared.settings.invalidate(changed_paths);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gix::actor::Signature;
    use gix::objs::tree::{Entry, EntryKind};
    use gix::objs::{Commit, Tree};
    use r_config::config::FunctionConfig;

    use super::Function;
    use crate::git::GitHandle;

    const PATH: &str = "function_settings";
    const DEF: &str = "def_code";

    fn config() -> FunctionConfig {
        FunctionConfig {
            def_setting_code: DEF.into(),
            git_repo_url: String::new(),
            git_workdir: String::new(),
            git_function_settings: PATH.into(),
            git_stream_setting: String::new(),
            git_catalog_setting: String::new(),
            git_consumer_setting: String::new(),
            git_function_value: String::new(),
            git_wasm_path: String::new(),
            git_plugin_path: String::new(),
            git_revision: "HEAD".into(),
            git_fetch_interval_secs: 30,
        }
    }

        fn repo_with(files: &[(&str, &str)]) -> (tempfile::TempDir, Arc<GitHandle>) {
        let dir = tempfile::tempdir().unwrap();
        let repo = gix::init(dir.path()).unwrap();

        let mut entries: Vec<Entry> = files
            .iter()
            .map(|(code, json)| Entry {
                mode: EntryKind::Blob.into(),
                filename: format!("{code}.json").into(),
                oid: repo.write_blob(json.as_bytes()).unwrap().detach(),
            })
            .collect();
        entries.sort();
        let sub = repo.write_object(Tree { entries }).unwrap().detach();
        let root = repo
            .write_object(Tree {
                entries: vec![Entry {
                    mode: EntryKind::Tree.into(),
                    filename: PATH.into(),
                    oid: sub,
                }],
            })
            .unwrap()
            .detach();

        let sig = Signature {
            name: "t".into(),
            email: "t@t".into(),
            time: gix::date::Time::now_utc(),
        };
        let head = repo
            .write_object(Commit {
                tree: root,
                parents: Default::default(),
                author: sig.clone(),
                committer: sig,
                encoding: None,
                message: "init".into(),
                extra_headers: Vec::new(),
            })
            .unwrap()
            .detach();

        let handle = GitHandle::new(
            repo.into_sync(),
            head,
            "HEAD".into(),
            "".into(),
            Arc::from(dir.path()),
        );
        (dir, Arc::new(handle))
    }

    fn keys(settings: &[super::FunctionSetting]) -> Vec<&str> {
        settings.iter().filter_map(|s| s.key()).collect()
    }

    #[test]
    fn known_code_returns_its_own_setting() {
        let (_dir, git) = repo_with(&[("own", r#"[{"key":"o"}]"#), (DEF, r#"[{"key":"d"}]"#)]);
        let f = Function::new(git, &config());

        let s = f.get_function_setting("own").unwrap();
        assert_eq!(keys(&s), ["o"]);
        assert!(f.settings_store().get(DEF).is_none());
    }

    #[test]
    fn unknown_code_falls_back_to_default_from_git() {
        let (_dir, git) = repo_with(&[(DEF, r#"[{"key":"d"}]"#)]);
        let f = Function::new(git, &config());
        assert!(f.get_cached_setting("nope").is_none());

        let s = f.get_function_setting("nope").unwrap();
        assert_eq!(keys(&s), ["d"]);
        assert!(f.settings_store().get("nope").is_none());
        assert!(f.settings_store().get(DEF).is_some());
        assert_eq!(keys(&f.get_cached_setting("nope").unwrap()), ["d"]);
    }

    #[test]
    fn missing_default_returns_none() {
        let (_dir, git) = repo_with(&[("own", r#"[{"key":"o"}]"#)]);
        let f = Function::new(git, &config());

        assert!(f.get_function_setting("nope").is_none());
        assert!(f.get_function_setting(DEF).is_none());
    }
}

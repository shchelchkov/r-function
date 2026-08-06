use gix::ObjectId;
use r_config::config::FunctionConfig;
use sonic_rs::{JsonValueTrait, Object, Value, json};
use std::sync::Arc;
use tracing::info;

use crate::functions::function_setting::FunctionSetting;
use crate::git::setting_store::SettingStore;
use crate::git::{GitHandle, HeadObserver, fetch_setting, fetch_setting_value};

#[derive(Clone)]
pub struct FunctionValue {
    shared: Arc<Shared>,
}

struct Shared {
    settings: SettingStore<FunctionSetting>,
    values: SettingStore<Value>,
}

impl FunctionValue {
    pub fn new(
        settings: SettingStore<FunctionSetting>,
        git: Arc<GitHandle>,
        function_config: &FunctionConfig,
    ) -> FunctionValue {
        let shared = Arc::new(Shared {
            settings,
            values: SettingStore::new(
                git,
                function_config.git_function_value.clone(),
                function_config.def_setting_code.clone(),
                "function value",
            ),
        });

        FunctionValue { shared }
    }

    pub fn get_function_value(&self, setting_code: &str, key: &str) -> Option<Arc<Vec<Value>>> {
        let value_code = format!("{setting_code}/{key}");
        if let Some(v) = self.shared.values.get(&value_code) {
            return Some(v);
        }

        match self
            .shared
            .settings
            .get_or_load(setting_code, fetch_setting)
        {
            Some(settings) => {
                let schema = build_schema(&settings);

                self.shared
                    .values
                    .get_or_load(&value_code, move |repo, spec| {
                        fetch_setting_value(repo, &schema, spec)
                    })
            }
            None => self.shared.values.get_or_load(&value_code, fetch_setting),
        }
    }

    pub fn get_function_values(
        &self,
        setting_code: &str,
    ) -> Option<Vec<(String, Arc<Vec<Value>>)>> {
        let keys = self.shared.values.list_keys(setting_code)?;
        Some(
            keys.into_iter()
                .filter_map(|key| {
                    let values = self.get_function_value(setting_code, &key)?;
                    Some((key, values))
                })
                .collect(),
        )
    }

    pub fn entries(&self) -> Vec<(String, Arc<Vec<FunctionSetting>>)> {
        self.shared.settings.entries()
    }
}

fn build_schema(settings: &[FunctionSetting]) -> Value {
    let mut obj = Object::new();
    for s in settings {
        let Some(key) = s.key() else { continue };
        obj.insert(key, default_value(s.type_data(), s.def_value()));
    }
    info!(
        "build_schema::::::::::::: obj = {}",
        sonic_rs::to_string(&obj).unwrap_or_default()
    );
    Value::from(obj)
}

fn default_value(type_data: Option<&str>, def_value: Option<&str>) -> Value {
    let def = def_value.map(str::trim).filter(|s| !s.is_empty());
    match type_data {
        Some("array") | Some("list") => def
            .and_then(|s| sonic_rs::from_str::<Value>(s).ok())
            .filter(|v| v.is_array())
            .unwrap_or_else(|| json!([])),
        Some("map") => def
            .and_then(|s| sonic_rs::from_str::<Value>(s).ok())
            .filter(|v| v.is_object())
            .unwrap_or_else(|| json!({})),
        Some("double") | Some("volume") | Some("sum") | Some("size") | Some("price") => def
            .and_then(|s| s.parse::<f64>().ok())
            .and_then(Value::new_f64)
            .unwrap_or_default(),
        Some("long") | Some("integer") | Some("id") => def
            .and_then(|s| s.parse::<i64>().ok())
            .map(Value::from)
            .unwrap_or_default(),
        Some("boolean") => def
            .and_then(|s| s.parse::<bool>().ok())
            .map(Value::from)
            .unwrap_or_default(),
        Some("text") | Some("code") | Some("varchar") | Some("date_time") | Some("date-time")
        | Some("instant") | Some("timestamp") | Some("time") | Some("period") | Some("seccode")
        | Some("code_last") => def.map(Value::from).unwrap_or_default(),
        _ => match def {
            None => Value::default(),
            Some(s) => {
                if let Ok(i) = s.parse::<i64>() {
                    Value::from(i)
                } else if let Some(f) = s.parse::<f64>().ok().and_then(Value::new_f64) {
                    f
                } else if let Ok(b) = s.parse::<bool>() {
                    Value::from(b)
                } else {
                    Value::from(s)
                }
            }
        },
    }
}

impl HeadObserver for FunctionValue {
    fn on_revision_changed(&self, _new_head: ObjectId, changed_paths: &[String]) {
        self.shared.values.invalidate(changed_paths);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(json: &str) -> Vec<FunctionSetting> {
        sonic_rs::from_str(json).expect("settings json")
    }

    #[test]
    fn structural_defaults_when_def_value_absent() {
        let s = settings(
            r#"[
                {"key": "tags", "typeData": "list"},
                {"key": "items", "typeData": "array"},
                {"key": "meta", "typeData": "map"},
                {"key": "price", "typeData": "double"},
                {"key": "count", "typeData": "long"},
                {"key": "active", "typeData": "boolean"},
                {"key": "name", "typeData": "text"}
            ]"#,
        );
        assert_eq!(
            build_schema(&s),
            json!({
                "tags": [],
                "items": [],
                "meta": {},
                "price": null,
                "count": null,
                "active": null,
                "name": null
            })
        );
    }

    #[test]
    fn typed_defaults_parsed_from_def_value() {
        let s = settings(
            r#"[
                {"key": "tags", "typeData": "list", "defValue": "[\"a\",\"b\"]"},
                {"key": "meta", "typeData": "map", "defValue": "{\"k\":1}"},
                {"key": "price", "typeData": "double", "defValue": "1.5"},
                {"key": "count", "typeData": "long", "defValue": "42"},
                {"key": "active", "typeData": "boolean", "defValue": "true"},
                {"key": "name", "typeData": "text", "defValue": "hello"}
            ]"#,
        );
        assert_eq!(
            build_schema(&s),
            json!({
                "tags": ["a", "b"],
                "meta": {"k": 1},
                "price": 1.5,
                "count": 42,
                "active": true,
                "name": "hello"
            })
        );
    }

    #[test]
    fn empty_def_value_falls_back_to_default() {
        let s = settings(
            r#"[
                {"key": "price", "typeData": "double", "defValue": ""},
                {"key": "tags", "typeData": "list", "defValue": "  "},
                {"key": "broken", "typeData": "long", "defValue": "not-a-number"}
            ]"#,
        );
        assert_eq!(
            build_schema(&s),
            json!({ "price": null, "tags": [], "broken": null })
        );
    }

    #[test]
    fn settings_without_key_are_skipped() {
        let s = settings(r#"[{"typeData": "text", "defValue": "x"}, {"key": "ok"}]"#);
        assert_eq!(build_schema(&s), json!({ "ok": null }));
    }

    use std::collections::BTreeMap;

    use gix::actor::Signature;
    use gix::objs::tree::{Entry, EntryKind};
    use gix::objs::{Commit, Tree};

    const SETTINGS: &str = "settings";
    const VALUES: &str = "values";

    fn config() -> FunctionConfig {
        FunctionConfig {
            def_setting_code: "def_code".into(),
            git_repo_url: String::new(),
            git_workdir: String::new(),
            git_function_settings: SETTINGS.into(),
            git_stream_setting: String::new(),
            git_catalog_setting: String::new(),
            git_consumer_setting: String::new(),
            git_function_value: VALUES.into(),
            git_wasm_path: String::new(),
            git_plugin_path: String::new(),
            git_revision: "HEAD".into(),
            git_fetch_interval_secs: 30,
        }
    }

    fn write_tree(repo: &gix::Repository, files: &[(&str, &str)]) -> ObjectId {
        let mut entries = Vec::new();
        let mut dirs: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
        for &(path, data) in files {
            match path.split_once('/') {
                Some((dir, rest)) => dirs.entry(dir).or_default().push((rest, data)),
                None => entries.push(Entry {
                    mode: EntryKind::Blob.into(),
                    filename: path.into(),
                    oid: repo.write_blob(data.as_bytes()).unwrap().detach(),
                }),
            }
        }
        for (dir, sub) in dirs {
            entries.push(Entry {
                mode: EntryKind::Tree.into(),
                filename: dir.into(),
                oid: write_tree(repo, &sub),
            });
        }
        entries.sort();
        repo.write_object(Tree { entries }).unwrap().detach()
    }

    fn repo_with(files: &[(&str, &str)]) -> (tempfile::TempDir, Arc<GitHandle>) {
        let dir = tempfile::tempdir().unwrap();
        let repo = gix::init(dir.path()).unwrap();
        let tree = write_tree(&repo, files);

        let sig = Signature {
            name: "t".into(),
            email: "t@t".into(),
            time: gix::date::Time::now_utc(),
        };
        let head = repo
            .write_object(Commit {
                tree,
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

    fn function_value(git: Arc<GitHandle>) -> FunctionValue {
        let cfg = config();
        let settings = SettingStore::new(
            git.clone(),
            SETTINGS,
            cfg.def_setting_code.clone(),
            "function setting",
        );
        FunctionValue::new(settings, git, &cfg)
    }

    #[test]
    fn values_listed_from_git_tree() {
        let (_dir, git) = repo_with(&[
            ("values/code/k2.json", r#"[{"v":2}]"#),
            ("values/code/k1.json", r#"[{"v":1},{"v":11}]"#),
            ("values/code/notes.txt", "not a value"),
            ("values/code/sub/k3.json", r#"[{"v":3}]"#),
            ("values/other/k9.json", r#"[{"v":9}]"#),
        ]);
        let fv = function_value(git);

        let values = fv.get_function_values("code").unwrap();
        let keys: Vec<&str> = values.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["k1", "k2"]);
        assert_eq!(*values[0].1, vec![json!({"v": 1}), json!({"v": 11})]);
        assert_eq!(*values[1].1, vec![json!({"v": 2})]);
    }

    #[test]
    fn unknown_setting_code_returns_none() {
        let (_dir, git) = repo_with(&[("values/code/k1.json", r#"[{"v":1}]"#)]);
        assert!(function_value(git).get_function_values("nope").is_none());
    }
}

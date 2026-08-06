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

        info!(
            "get_function_value:::::::::::: setting_code/key {:?}",
            &value_code
        );
        match self
            .shared
            .settings
            .get_or_load(setting_code, fetch_setting)
        {
            Some(settings) => {
                info!(
                    "get_function_value:::::::::::: 0000 setting_code={setting_code} key={key} spec {:?}",
                    &settings
                );
                let schema = build_schema(&settings);

                self.shared
                    .values
                    .get_or_load(&value_code, move |repo, spec| {
                        info!("get_function_value:::::::::::: 0001 setting_code={setting_code} key={key} spec {:?}", &spec);
                        fetch_setting_value(repo, &schema, spec)
                    })
            }
            None => {
                info!(
                    "get_function_value:::::::::::: 0002 setting_code={setting_code} key={key} spec None"
                );
                self.shared
                    .values
                    .get_or_load(&value_code, |repo, spec| fetch_setting(repo, spec))
            }
        }
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
}

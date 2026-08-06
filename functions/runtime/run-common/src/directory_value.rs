use crate::host;
use f_common::message::convert_value;
use f_common::{fun, val};
use sonic_rs::{Object, Value};
use std::collections::HashMap;

const SETTING_CODE: &str = "directory_value";

pub fn value_process(obj: &Object, keys: &mut HashMap<String, String>) {
    let setting_code = fun::f_value(obj, "setting_code");
    if !keys.contains_key(setting_code) {
        match value_code_key(setting_code) {
            Ok(key) => {
                keys.insert(setting_code.to_owned(), key);
            }
            Err(e) => {
                eprintln!("json_process: {setting_code}: {e}");
                return;
            }
        }
    }
    let value_code = keys[setting_code].as_str();
    let key = fun::f_value(obj, value_code);

    let mut envelope = Object::with_capacity(3);
    envelope.insert("setting_code", SETTING_CODE);
    envelope.insert("key", key);

    if let Some(values) = convert_value(obj, key, "setting_code") {
        envelope.insert("value", values);

        let out = match sonic_rs::to_vec(&envelope) {
            Ok(out) => out,
            Err(e) => {
                eprintln!("json_process:::::::::::::: сериализация не удалась: {e}");
                return;
            }
        };

        if let Err(rc) = host::send_value(&out) {
            eprintln!("json_process: send_value завершился с ошибкой: rc={rc}");
        }
    }
}

fn value_code_key(setting_code: &str) -> Result<String, String> {
    let settings: Vec<Value> = match host::get_function_setting(setting_code) {
        Ok(Some(bytes)) => sonic_rs::from_slice(&bytes).map_err(|e| e.to_string())?,
        Ok(None) => Vec::new(),
        Err(rc) => return Err(format!("get_function_setting rc={rc}")),
    };
    val::is_value_code(&settings).map(str::to_owned)
}

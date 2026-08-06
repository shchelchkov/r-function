use f_common::{fun, host};
use sonic_rs::{JsonContainerTrait, JsonValueMutTrait, JsonValueTrait, Object, Value, json};
use std::io::{self, Read, Write};

fn main() {
    eprintln!("wasm-function!");
    let mut buf = Vec::new();
    io::stdin().read_to_end(&mut buf).expect("read stdin");

    let mut v: Value = sonic_rs::from_slice(&buf).expect("parse json");

    let setting_code = match v.as_array() {
        Some(items) => items
            .first()
            .and_then(|it| it.get("setting_code"))
            .and_then(|c| c.as_str()),
        None => v.get("setting_code").and_then(|c| c.as_str()),
    }
    .map(str::to_owned);

    let settings: Option<Value> = match &setting_code {
        Some(code) => match host::get_function_setting(code) {
            Ok(Some(bytes)) => Some(sonic_rs::from_slice(&bytes).expect("parse settings")),
            Ok(None) => None,
            Err(rc) => panic!("get_function_setting host call failed: rc={rc}"),
        },
        None => None,
    };

    let stamp = |obj: &mut Object| {
        let mut errors: Vec<&str> = Vec::new();

        match settings.as_ref().and_then(|s| s.as_array()) {
            Some(settings) => {
                for setting in settings.iter() {
                    match fun::f_target_key(obj, setting) {
                        Ok((target, value)) => {
                            obj.insert(target, value);
                        }
                        Err(status) => errors.push(status),
                    }
                }
            }
            None => errors.push("no_settings"),
        }

        obj.insert(
            "check_function",
            json!({ "checked": true, "errors": errors }),
        );
    };

    if let Some(values) = v.as_array_mut() {
        for value in values.iter_mut() {
            if let Some(obj) = value.as_object_mut() {
                stamp(obj);
            }
        }
    } else if let Some(value) = v.as_object_mut() {
        stamp(value);
    }

    let out = sonic_rs::to_vec(&v).expect("serialize");
    if let Err(e) = io::stdout().write_all(&out) {
        eprintln!("write stdout failed ({} bytes): {e}", out.len());
        std::process::exit(1);
    }
}

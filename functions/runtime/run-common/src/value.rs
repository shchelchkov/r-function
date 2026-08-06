use sonic_rs::{Object, Value};

use crate::host;

pub trait FnState: Sized {
    const CODE: &'static str;

    fn from_value(values: &[Value]) -> Self;

    fn into_value(self) -> Value;
}

pub fn load<S: FnState>(key: &str) -> S {
    S::from_value(&load_raw(S::CODE, key))
}

pub fn save<S: FnState>(key: &str, state: S) -> bool {
    save_raw(S::CODE, key, state.into_value())
}

pub fn load_function_value<S: FnState>(key: &str) -> S {
    S::from_value(&load_function_value_raw(S::CODE, key))
}

fn load_function_value_raw(setting_code: &str, key: &str) -> Vec<Value> {
    let mut req = Object::with_capacity(2);
    req.insert("setting_code", setting_code);
    req.insert("key", key);
    let req_bytes = match sonic_rs::to_vec(&req) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };

    match host::get_function_value(&req_bytes) {
        Ok(Some(bytes)) => sonic_rs::from_slice::<Vec<Value>>(&bytes).unwrap_or_default(),
        Ok(None) => Vec::new(),
        Err(rc) => {
            eprintln!("value::load_function_value({setting_code}/{key}): rc={rc}");
            Vec::new()
        }
    }
}

fn load_raw(setting_code: &str, key: &str) -> Vec<Value> {
    let mut req = Object::with_capacity(2);
    req.insert("setting_code", setting_code);
    req.insert("key", key);
    let req_bytes = match sonic_rs::to_vec(&req) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };

    match host::get_value(&req_bytes) {
        Ok(Some(bytes)) => sonic_rs::from_slice::<Vec<Value>>(&bytes).unwrap_or_default(),
        Ok(None) => Vec::new(),
        Err(rc) => {
            eprintln!("state::load({setting_code}/{key}): get_value rc={rc}");
            Vec::new()
        }
    }
}

fn save_raw(setting_code: &str, key: &str, value: Value) -> bool {
    let mut req = Object::with_capacity(3);
    req.insert("setting_code", setting_code);
    req.insert("key", key);
    req.insert("value", value);

    let req_bytes = match sonic_rs::to_vec(&req) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("state::save({setting_code}/{key}): serialize: {e}");
            return false;
        }
    };

    match host::put_value(&req_bytes) {
        Ok(()) => true,
        Err(rc) => {
            eprintln!("state::save({setting_code}/{key}): put_value rc={rc}");
            false
        }
    }
}

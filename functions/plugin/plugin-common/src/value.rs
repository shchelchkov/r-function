use f_common::fun;
use r_plugin_api::HostApi;
use sonic_rs::{Value, json};

pub trait FnState: Sized {
    const CODE: &'static str;

    fn from_value(values: &[Value]) -> Self;

    fn into_value(self) -> Value;
}

pub struct StateApi<'a> {
    api: &'a HostApi,
}

impl<'a> StateApi<'a> {
    pub fn new(api: &'a HostApi) -> Self {
        Self { api }
    }

    pub fn load<S: FnState>(&self, key: &str) -> S {
        S::from_value(&load_raw(self.api, S::CODE, key))
    }

    pub fn save<S: FnState>(&self, key: &str, state: S) -> bool {
        save_raw(self.api, S::CODE, key, state.into_value())
    }

    pub fn load_function_value<S: FnState>(&self, key: &str) -> S {
        S::from_value(&load_function_value_raw(self.api, S::CODE, key))
    }
}

pub fn load<S: FnState>(api: &HostApi, key: &str) -> S {
    S::from_value(&load_raw(api, S::CODE, key))
}

pub fn save<S: FnState>(api: &HostApi, key: &str, state: S) -> bool {
    save_raw(api, S::CODE, key, state.into_value())
}

pub fn load_function_value<S: FnState>(api: &HostApi, key: &str) -> S {
    S::from_value(&load_function_value_raw(api, S::CODE, key))
}

fn load_function_value_raw(api: &HostApi, setting_code: &str, key: &str) -> Vec<Value> {
    if let Some(value) = api.get_function_value(setting_code, key) {
        return fun::from_value(&value);
    }

    eprintln!("value::load({setting_code}/{key}): get_function_value returned None");
    Vec::new()
}


fn load_raw(api: &HostApi, setting_code: &str, key: &str) -> Vec<Value> {
    if let Some(values) = api.get_value(setting_code, key) {
        return values;
    }

    eprintln!("state::load({setting_code}/{key}): get_value returned None");
    Vec::new()
}


fn save_raw(api: &HostApi, setting_code: &str, key: &str, value: Value) -> bool {
    match api.put_value(&setting_code, &key, &value) {
        Ok(()) => true,
        Err(rc) => {
            eprintln!("state::save({setting_code}/{key}): put_value rc={rc}");
            false
        }
    }
}

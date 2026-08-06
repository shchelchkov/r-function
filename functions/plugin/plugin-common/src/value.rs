use f_common::fun;
use r_plugin_api::HostApi;
use sonic_rs::{Value, json};

pub trait FnState: Sized {
    const CODE: &'static str;

    fn from_value(values: &[Value]) -> Self;

    fn into_value(self) -> Value;

            fn reloaded(&self) -> Self
    where
        Self: Clone,
    {
        Self::from_value(&[self.clone().into_value()])
    }
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

pub fn try_load<S: FnState>(api: &HostApi, key: &str) -> Option<S> {
    try_load_code(api, S::CODE, key)
}

pub fn try_load_code<S: FnState>(api: &HostApi, code: &str, key: &str) -> Option<S> {
    api.get_value(code, key).map(|values| S::from_value(&values))
}

pub fn try_load_at<S: FnState>(api: &HostApi, key: &str, at: i64) -> Option<S> {
    try_load_at_code(api, S::CODE, key, at)
}

pub fn try_load_at_code<S: FnState>(api: &HostApi, code: &str, key: &str, at: i64) -> Option<S> {
    api.get_value_at(code, key, at)
        .map(|values| S::from_value(&values))
}

pub fn save<S: FnState>(api: &HostApi, key: &str, state: S) -> bool {
    save_code(api, S::CODE, key, state)
}

pub fn save_code<S: FnState>(api: &HostApi, code: &str, key: &str, state: S) -> bool {
    save_raw(api, code, key, state.into_value())
}

pub fn load_function_value<S: FnState>(api: &HostApi, key: &str) -> S {
    S::from_value(&load_function_value_raw(api, S::CODE, key))
}

pub fn load_function_values<S: FnState>(api: &HostApi, key: &str) -> Vec<S> {
    from_list(&load_function_value_raw(api, S::CODE, key))
}

pub fn from_list<S: FnState>(raw: &[Value]) -> Vec<S> {
    if raw.is_empty() {
        return vec![S::from_value(&[])];
    }
    raw.iter()
        .map(|v| S::from_value(std::slice::from_ref(v)))
        .collect()
}

fn load_function_value_raw(api: &HostApi, setting_code: &str, key: &str) -> Vec<Value> {
    if let Some(value) = api.get_function_value(setting_code, key) {
        return fun::from_value(&value);
    }

    tracing::warn!("value::load({setting_code}/{key}): get_function_value returned None");
    Vec::new()
}

fn load_raw(api: &HostApi, setting_code: &str, key: &str) -> Vec<Value> {
    if let Some(values) = api.get_value(setting_code, key) {
        return values;
    }

    tracing::info!("state::load({setting_code}/{key}): get_value returned None");
    Vec::new()
}

fn save_raw(api: &HostApi, setting_code: &str, key: &str, value: Value) -> bool {
    match api.put_value(&setting_code, &key, &value) {
        Ok(()) => true,
        Err(rc) => {
            tracing::error!("state::save({setting_code}/{key}): put_value rc={rc}");
            false
        }
    }
}

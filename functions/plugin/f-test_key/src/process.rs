use f_common::fun;
use plugin_common::buffer::{empty_buffer, into_buffer};
use r_plugin_api::{Buffer, HostApi, PluginError};
use sonic_rs::{JsonContainerTrait, JsonValueTrait, Object, Value, json};

pub unsafe fn process(api: *const HostApi, input_ptr: *const u8, input_len: usize) -> Buffer {
    let Some(api) = (unsafe { api.as_ref() }) else {
        return empty_buffer();
    };
    if input_ptr.is_null() && input_len != 0 {
        return empty_buffer();
    }
    let input = unsafe { std::slice::from_raw_parts(input_ptr, input_len) };
    let input: Value = match sonic_rs::from_slice(input) {
        Ok(value) => value,
        Err(_) => return empty_buffer(),
    };
    let mut v = Vec::new();
    if let Some(values) = input.as_array() {
        for value in values.iter() {
            if let Some(obj) = value.as_object() {
                if let Ok(processed_value) = value_process(api, obj) {
                    v.push(processed_value);
                }
            }
        }
    } else if let Some(obj) = input.as_object() {
        if let Ok(processed_value) = value_process(api, obj) {
            v.push(processed_value);
        }
    }
    let output = match sonic_rs::to_vec(&v) {
        Ok(bytes) => bytes,
        Err(_) => return empty_buffer(),
    };
    into_buffer(output)
}

fn value_process(api: &HostApi, input: &Object) -> Result<Value, PluginError> {
    let setting_code = match input.get(&"setting_code").and_then(|value| value.as_str()) {
        Some(value) => value,
        None => return Err(PluginError::InvalidInput("Missing setting_code".into())),
    };
    let value = match fun::to_value(input) {
        Ok(b) => b,
        Err(e) => {
            return Err(PluginError::Encode(e.to_string()));
        }
    };

    let key = "value";

    if api.put_value(setting_code, key, &value).is_err() {
        return Err(PluginError::Processing("Failed to put value in cache".into()));
    }
    let cached_value = api.get_value(setting_code, key).unwrap_or_default();
    let output = json!({
        "setting_code": setting_code,
        "key": key,
        "cached": cached_value,
    });
    Ok(output)
}


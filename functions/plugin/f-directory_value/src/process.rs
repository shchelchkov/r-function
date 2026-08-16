use f_common::fun;
use f_common::message::convert_value;
use plugin_common::buffer::{empty_buffer, into_buffer};
use r_plugin_api::{Buffer, HostApi, PluginError};
use sonic_rs::{JsonContainerTrait, JsonValueTrait, Object, Value};

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

    if let Some(values) = convert_value(&input, key, "setting_code") {
        for value in values.iter() {

            let v = fun::to_value(value).map_err(|e| PluginError::Encode(e.to_string()))?;
            let attribute_code = match value.get(&"attribute_code").and_then(|value| value.as_str()) {
                Some(value) => value,
                None => return Err(PluginError::InvalidInput("Missing attribute_code".into())),
            };

            if api.put_value(setting_code, attribute_code, &v).is_err() {
                return Err(PluginError::Processing("Failed to put value in cache".into()));
            }

            let payload = sonic_rs::to_vec(&value)
                .map_err(|e| PluginError::Encode(e.to_string()))?;
            api.send_value(
                "value_process",
                Some(b"directory_value"),
                Some(b"directory_value_channel"),
                &payload,
            );
        }
    }

    Ok(value)
}

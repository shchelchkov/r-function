use f_common::fun;
use plugin_common::buffer::{empty_buffer, into_buffer};
use plugin_common::val;
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
    let input: Value = fun::from_slice(input);
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
    let output = fun::to_vec(&mut v);
    into_buffer(output)
}

fn value_process(api: &HostApi, input: &Object) -> Result<Value, PluginError> {
    let (setting_code, value) = val::get_setting_code_value(input)?;
    
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




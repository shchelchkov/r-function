use f_common::fun;
use r_plugin_api::PluginError;
use sonic_rs::{JsonValueTrait, Object, Value};

pub fn get_setting_code_value(input: &Object) -> Result<(&str, Value), PluginError> {

    let setting_code = input.get(&"setting_code")
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            PluginError::InvalidInput("Missing setting_code".into())
        })?;

    let value = fun::to_value(input)
        .map_err(|e| PluginError::Encode(e.to_string()))?;

    Ok((setting_code, value))
}

pub fn is_key(input: &[Value]) -> Result<&str, PluginError> {
    for value in input {
        let is_key = value.get("isKey").map(fun::to_bool).unwrap_or(false);
        if is_key {
            return value.get("code")
                .and_then(Value::as_str)
                .ok_or_else(|| { PluginError::InvalidInput("Missing value for isKey".into()) });
        }
    }

    Ok("value")
}
use crate::val;
use f_common::fun;
use f_common::message::convert_value;
use r_plugin_api::{HostApi, PluginError};
use sonic_rs::{JsonContainerTrait, JsonValueMutTrait, JsonValueTrait, Object, Value};

const SETTING_CODE: &str = "value_process";
const CHANNEL: &str = "directory_value_channel";

pub fn value_process(api: &HostApi, input: &Object) -> Result<Value, PluginError> {
    let (setting_code, value) = val::get_setting_code_value(input)?;

    let function_setting = api.get_function_setting(setting_code).unwrap_or_default();
    let key = match val::is_key(&function_setting) {
        Ok(value) => value,
        Err(err) => return Err(err),
    };

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

            let mut cached_values = api.get_value(setting_code, attribute_code).unwrap_or_default();
            let mut envelope = cached_values
                .first()
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or_default();

            envelope.insert("setting_code", SETTING_CODE);
            envelope.insert("channel", CHANNEL);
            envelope.insert("key", key);

            if let Err(rc) = api.send_value(&envelope) {
                eprintln!("value_process: send_value завершился с ошибкой: rc={rc}");
            }
        }
    }

    Ok(value)
}




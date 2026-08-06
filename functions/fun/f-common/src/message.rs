use sonic_rs::{JsonValueTrait, Object, Value};

use crate::fun;

pub fn data_message<'a>(obj: &'a Object) -> Option<(String, &'a str, &'a str, bool)> {
    let user_id = obj.get(&"user_id").map(fun::to_f64)? as i64;
    let user_name = obj.get(&"user_name")?.as_str()?;
    let command = obj.get(&"command")?.as_str()?;

    let is_active = obj.get(&"is_active").map(fun::to_bool).unwrap_or(false);
    Some((user_id.to_string(), user_name, command, is_active))
}

pub fn text_message<'a>(obj: &'a Object) -> Option<&'a str> {
    let text = obj.get(&"text")?.as_str()?;
    Some(text)
}

pub fn data_message_json(obj: &Object, json: &str, data: &str) -> Option<Value> {
    let raw = obj.get(&json)?.as_str()?;
    let parsed: Value = sonic_rs::from_slice(raw.as_bytes()).ok()?;
    parsed.get(data).cloned()
}

pub fn convert_values(
    msg: &Object,
    item: &Object,
    key: &str,
    setting_code: &str,
    setting_code_json: &str,
) -> Option<Vec<Object>> {
    let mut values: Vec<Object> = Vec::new();
    let kv = fun::f_value(item, key);

    if let Some(caption) = msg.get(&setting_code) {
        if let Some(topic) = item.get(&"topic") {
            if let Some(date_time) = msg.get(&"date_time") {
                for (i, (k, v)) in item.iter().enumerate() {
                    let mut value = Object::with_capacity(item.len() + 2);
                    value.insert("directory_code", caption.clone());
                    value.insert("entity_code", topic.clone());
                    value.insert("attribute_code", k);
                    value.insert("attribute_value", v.clone());
                    value.insert("value_code", kv);
                    value.insert("order_n", (i + 1) as i64);
                    value.insert("setting_code", setting_code_json);
                    value.insert("date_time", date_time.clone());
                    values.push(value);
                }
            }
        }
    }
    Some(values)
}

pub fn convert_value(item: &Object, key: &str, setting_code: &str) -> Option<Vec<Object>> {
    let mut values: Vec<Object> = Vec::new();
    let kv = fun::f_value(item, key);

    if let Some(setting_code) = item.get(&setting_code) {
        if let Some(topic) = item.get(&"topic") {
            if let Some(date_time) = item.get(&"date_time") {
                for (i, (k, v)) in item.iter().enumerate() {
                    let mut value = Object::with_capacity(item.len() + 2);
                    value.insert("directory_code", setting_code.clone());
                    value.insert("entity_code", topic.clone());
                    value.insert("attribute_code", k);
                    value.insert("attribute_value", v.clone());
                    value.insert("value_code", kv);
                    value.insert("order_n", (i + 1) as i64);
                    value.insert("setting_code", setting_code.clone());
                    value.insert("date_time", date_time.clone());

                    values.push(value);
                }
            }
        }
    }

    Some(values)
}

use std::sync::Arc;

use sonic_rs::{JsonValueTrait, Value};

const SEPARATOR: &str = "|";

pub fn parse_and_build_key(
    raw: &[u8],
    value_key: &[String],
) -> Result<Option<(Value, Arc<str>)>, sonic_rs::Error> {
    let value: Value = sonic_rs::from_slice(raw)?;
    Ok(build_key(&value, value_key).map(|key| (value, key)))
}

pub fn build_key(json: &Value, value_key: &[String]) -> Option<Arc<str>> {
    if value_key.is_empty() {
        return None;
    }
    let mut buf = String::new();
    for (i, name) in value_key.iter().enumerate() {
        let part = scalar_to_string(json.get(name.as_str())?)?;
        if i > 0 {
            buf.push_str(SEPARATOR);
        }
        buf.push_str(&part);
    }
    Some(Arc::from(buf))
}

fn scalar_to_string(v: &Value) -> Option<String> {
    if let Some(s) = v.as_str() {
        Some(s.to_owned())
    } else if let Some(b) = v.as_bool() {
        Some(b.to_string())
    } else if let Some(i) = v.as_i64() {
        Some(i.to_string())
    } else if let Some(u) = v.as_u64() {
        Some(u.to_string())
    } else if let Some(f) = v.as_f64() {
        Some(f.to_string())
    } else {
        None
    }
}

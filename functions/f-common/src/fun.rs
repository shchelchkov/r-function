use sonic_rs::{JsonContainerTrait, JsonValueTrait, Object, PointerNode::Key, Value};
use crate::fun;

pub fn f_target_key<'s>(
    obj: &Object,
    setting: &'s Value,
) -> Result<(&'s str, Value), &'static str> {
    let key = setting
        .get("key")
        .and_then(|v| v.as_str())
        .filter(|k| !k.trim().is_empty())
        .ok_or("no_key")?;
    let target = setting
        .get("targetKey")
        .and_then(|v| v.as_str())
        .filter(|k| !k.trim().is_empty())
        .ok_or("no_target_key")?;
    let value = obj.get(&key).cloned().ok_or("no_value")?;

    Ok((target, value))
}

pub fn f_to_mmssms(s: &str) -> Option<String> {
    let mm = s.get(14..16)?;
    let ss = s.get(17..19)?;
    let ms = s.get(20..23)?;
    Some(format!("{mm}:{ss}:{ms}"))
}

pub fn f_dt_seconds(s: &str) -> &str {
    s.get(..19).unwrap_or(s)
}

pub fn f_topic(obj: &Object) -> &str {
    obj.get(&"topic")
        .and_then(|s| s.as_str())
        .unwrap_or_default()
}

pub fn f_f64(obj: &Object, k: &str) -> f64 {
    obj.get(&k).map(to_f64).unwrap_or(0.0)
}

pub fn f_value<'s>(obj: &'s Object, k: &'s str) -> &'s str {
    obj.get(&k).and_then(|s| s.as_str()).unwrap_or_default()
}

pub fn f_val(obj: &Object, key: &str) -> String {
    obj.get(&key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_owned()
}

pub fn f_date_time(obj: &Object) -> &str {
    obj.get(&"date_time")
        .and_then(|s| s.as_str())
        .unwrap_or_default()
}

pub fn f_cts(obj: &Object) -> i64 {
    obj.get(&"cts").and_then(|s| s.as_i64()).unwrap_or(0)
}

pub fn f_ts(obj: &Object) -> i64 {
    obj.get(&"ts").and_then(|s| s.as_i64()).unwrap_or(0)
}

pub fn f_t(obj: &Object) -> i64 {
    obj.get(&"T").and_then(|s| s.as_i64()).unwrap_or(0)
}

pub fn f_get_key_value<'s>(setting: &'s Value, k: &'s str) -> Option<&'s str> {
    setting
        .get(k)
        .and_then(|v| v.as_str())
        .filter(|k| !k.trim().is_empty())
}

pub fn to_bool(v: &Value) -> bool {
    if let Some(f) = v.as_bool() {
        return f;
    }
    if let Some(s) = v.as_str() {
        return s.trim().parse::<bool>().unwrap_or(false);
    }
    false
}

pub fn to_f64(v: &Value) -> f64 {
    if let Some(f) = v.as_f64() {
        return f;
    }
    if let Some(i) = v.as_i64() {
        return i as f64;
    }
    if let Some(u) = v.as_u64() {
        return u as f64;
    }
    if let Some(s) = v.as_str() {
        return s.trim().parse::<f64>().unwrap_or(0.0);
    }
    0.0
}

pub fn to_i64(v: &Value) -> i64 {
    if let Some(f) = v.as_i64() {
        return f;
    }
    if let Some(s) = v.as_str() {
        return s.trim().parse::<i64>().unwrap_or(0);
    }
    0
}

pub fn json_bool_int(b: bool) -> i8 {
    if b { 1 } else { 0 }
}

pub fn vol_at(arr: &Value, i: usize) -> f64 {
    let Some(level) = arr.as_array().and_then(|a| a.get(i)) else {
        return 0.0;
    };
    let Some(level) = level.as_array() else {
        return 0.0;
    };
    level.get(1).map(to_f64).unwrap_or(0.0)
}

pub fn json_f64(x: f64) -> Value {
    Value::new_f64(x).unwrap_or_else(|| Value::new_f64(0.0).unwrap())
}

pub fn ts(obj: &Object) -> i64 {
    let Some(t) = obj.get(&"ts") else {
        return 0;
    };
    if let Some(i) = t.as_i64() {
        return i;
    }
    if let Some(u) = t.as_u64() {
        return u as i64;
    }
    if let Some(f) = t.as_f64() {
        return f as i64;
    }
    if let Some(s) = t.as_str() {
        return s.trim().parse().unwrap_or(0);
    }
    0
}

pub fn timestamp(obj: &Object) -> i64 {
    let Some(t) = obj.get(&"timestamp") else {
        return 0;
    };
    if let Some(i) = t.as_i64() {
        return i;
    }
    if let Some(u) = t.as_u64() {
        return u as i64;
    }
    if let Some(f) = t.as_f64() {
        return f as i64;
    }
    if let Some(s) = t.as_str() {
        return s.trim().parse().unwrap_or(0);
    }
    0
}

pub fn instant(obj: &Object) -> i64 {
    let Some(t) = obj.get(&"instant") else {
        return 0;
    };
    if let Some(i) = t.as_i64() {
        return i;
    }
    if let Some(u) = t.as_u64() {
        return u as i64;
    }
    if let Some(f) = t.as_f64() {
        return f as i64;
    }
    if let Some(s) = t.as_str() {
        return s.trim().parse().unwrap_or(0);
    }
    0
}

pub fn f_vec(obj: &Object, key: &str) -> Vec<String> {
    obj.get(&key)
        .and_then(|v| v.as_array())
        .map(|array| {
            array
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

pub fn f_i64(obj: &Object, key: &str) -> i64 {
    obj.get(&key)
        .map(to_f64)
        .unwrap_or_default() as i64
}

pub fn f_bool(obj: &Object, key: &str) -> bool {
    obj.get(&key)
        .map(to_bool)
        .unwrap_or_default()
}
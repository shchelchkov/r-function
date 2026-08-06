use sonic_rs::{JsonValueTrait, Object, Value};

pub fn f_target_key<'s>(
    obj: &Object,
    setting: &'s Value,
) -> Result<(&'s str, Value), &'static str> {
    let key = setting
        .get("key")
        .and_then(|v| v.as_str())
        .filter(|k| !k.is_empty() || k.trim() != "")
        .ok_or("no_key")?;
    let target = setting
        .get("targetKey")
        .and_then(|v| v.as_str())
        .filter(|k| !k.is_empty() || k.trim() != "")
        .ok_or("no_target_key")?;
    let value = obj.get(&key).cloned().ok_or("no_value")?;

    Ok((target, value))
}

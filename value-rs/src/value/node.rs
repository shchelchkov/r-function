use std::sync::Arc;

use crate::error::ValueRsError;
use crate::parse_and_build_key;

pub type Result<T> = std::result::Result<T, ValueRsError>;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[repr(C)]
pub struct Value {
    pub value_key: Vec<String>,
    pub setting_code: Box<str>,
    pub key: Arc<str>,
    pub value: sonic_rs::Value,
}

pub fn from_slice_and_build_key(
    raw: &[u8],
    value_key: &[String],
    setting_code: &str,
) -> Result<Option<Value>> {
    Ok(
        parse_and_build_key(raw, value_key)?.map(|(value, key)| Value {
            value_key: value_key.to_vec(),
            setting_code: setting_code.into(),
            key,
            value,
        }),
    )
}

pub fn to_vec(value: &[Value]) -> Result<Vec<u8>> {
    Ok(sonic_rs::to_vec(value)?)
}

pub fn from_slice(raw: &[u8]) -> Result<Vec<Value>> {
    let value: sonic_rs::Value = sonic_rs::from_slice(raw)?;
    Ok(vec![Value {
        value_key: Vec::new(),
        setting_code: Box::from(""),
        key: Arc::from(""),
        value,
    }])
}

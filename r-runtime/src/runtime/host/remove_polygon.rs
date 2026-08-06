use std::sync::Arc;
use async_trait::async_trait;
use r_error::runtime::error::RuntimeError;
use r_tree::value::polygon::Values;
use serde::Deserialize;
use sonic_rs::Value;
use super::HostFn;

pub struct RemovePolygon {
    pub values: Values,
}

#[derive(Deserialize)]
struct Req {
    setting_code: String,
    key: String,
    value: Value,
}

#[async_trait]
impl HostFn for RemovePolygon {
    fn name(&self) -> &'static str {
        "remove_polygon"
    }

    async fn call(&self, input: &[u8]) -> Result<Option<Vec<u8>>, RuntimeError> {
        let req: Req =
            sonic_rs::from_slice(input).map_err(|e| RuntimeError::Decode(e.to_string()))?;
        let key: Arc<str> = Arc::from(req.key.as_str());
        self.values.remove_polygon(&req.setting_code, key, req.value);
        Ok(None)
    }
}

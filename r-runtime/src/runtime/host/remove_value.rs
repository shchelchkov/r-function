use async_trait::async_trait;
use r_error::runtime::error::RuntimeError;
use r_value::value::value::Values;
use serde::Deserialize;

use super::HostFn;

pub struct RemoveValue {
    pub values: Values,
}

#[derive(Deserialize)]
struct Req {
    setting_code: String,
    key: String,
}

#[async_trait]
impl HostFn for RemoveValue {
    fn name(&self) -> &'static str {
        "remove_value"
    }

    async fn call(&self, input: &[u8]) -> Result<Option<Vec<u8>>, RuntimeError> {
        let req: Req =
            sonic_rs::from_slice(input).map_err(|e| RuntimeError::Decode(e.to_string()))?;
        self.values.remove_value(&req.setting_code, &req.key);
        Ok(None)
    }
}

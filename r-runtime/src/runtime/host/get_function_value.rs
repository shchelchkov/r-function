use async_trait::async_trait;
use r_error::runtime::error::RuntimeError;
use r_setting::functions::functions_value::FunctionValue;
use sonic_rs::{JsonValueTrait, Value};
use tracing::info;

use super::HostFn;

pub struct GetFunctionValue {
    pub function_value: FunctionValue,
}

#[async_trait]
impl HostFn for GetFunctionValue {
    fn name(&self) -> &'static str {
        "get_function_value"
    }

    async fn call(&self, input: &[u8]) -> Result<Option<Vec<u8>>, RuntimeError> {
        let req: Value =
            sonic_rs::from_slice(input).map_err(|e| RuntimeError::Decode(e.to_string()))?;
        let setting_code = req
            .get("setting_code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RuntimeError::Decode("get_function_value: missing setting_code".into()))?
            .to_owned();
        let key = req
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RuntimeError::Decode("get_function_value: missing key".into()))?
            .to_owned();

        let function_value = self.function_value.clone();
        let values = tokio::task::spawn_blocking(move || {
            function_value.get_function_value(&setting_code, &key)
        })
        .await
        .map_err(|e| RuntimeError::Internal(e.to_string()))?;

        match values {
            Some(v) => Ok(Some(
                sonic_rs::to_vec(&*v).map_err(|e| RuntimeError::Internal(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }
}

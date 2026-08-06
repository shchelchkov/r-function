use super::HostFn;
use async_trait::async_trait;
use r_error::runtime::error::RuntimeError;
use r_setting::functions::functions_value::FunctionValue;
use r_value::value::value::Values;
use serde::Deserialize;

pub struct GetValue {
    pub values: Values,
    pub function_value: FunctionValue,
}

#[derive(Deserialize)]
struct Req {
    setting_code: String,
    key: String,
}

#[async_trait]
impl HostFn for GetValue {
    fn name(&self) -> &'static str {
        "get_value"
    }

    async fn call(&self, input: &[u8]) -> Result<Option<Vec<u8>>, RuntimeError> {
        let req: Req =
            sonic_rs::from_slice(input).map_err(|e| RuntimeError::Decode(e.to_string()))?;
        match self.values.get_value(&req.setting_code, &req.key) {
            Some(v) => {
                let bytes =
                    sonic_rs::to_vec(&*v).map_err(|e| RuntimeError::Internal(e.to_string()))?;
                Ok(Some(bytes))
            }
            None => Ok(None),
        }
    }
}

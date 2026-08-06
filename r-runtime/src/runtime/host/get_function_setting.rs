use async_trait::async_trait;
use r_error::runtime::error::RuntimeError;
use r_setting::functions::functions::Function;

use super::HostFn;

pub struct GetFunctionSetting {
    pub function: Function,
}

#[async_trait]
impl HostFn for GetFunctionSetting {
    fn name(&self) -> &'static str {
        "get_function_setting"
    }

    async fn call(&self, input: &[u8]) -> Result<Option<Vec<u8>>, RuntimeError> {
        let code = std::str::from_utf8(input)
            .map_err(|e| RuntimeError::Decode(e.to_string()))?
            .to_owned();

        let function = self.function.clone();
        let settings = tokio::task::spawn_blocking(move || function.get_function_setting(&code))
            .await
            .map_err(|e| RuntimeError::Internal(e.to_string()))?;

        match settings {
            Some(v) => {
                let bytes = sonic_rs::to_vec(&*v)
                    .map_err(|e| RuntimeError::Internal(e.to_string()))?;
                Ok(Some(bytes))
            }
            None => Ok(None),
        }
    }
}

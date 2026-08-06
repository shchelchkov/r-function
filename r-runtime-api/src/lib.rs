use async_trait::async_trait;
use r_error::runtime::error::RuntimeError;
use sonic_rs::Value;

#[async_trait]
pub trait Runtime: Send + Sync {
    async fn invoke_raw(&self, module_name: &str, input: Vec<u8>) -> Result<Vec<u8>, RuntimeError>;

    async fn invoke(
        &self,
        module_name: &str,
        value: &Vec<Value>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let payload = sonic_rs::to_vec(value).map_err(|e| RuntimeError::Encode(e.to_string()))?;
        let out = self.invoke_raw(module_name, payload).await?;
        sonic_rs::from_slice(&out).map_err(|e| RuntimeError::Decode(e.to_string()))
    }
}

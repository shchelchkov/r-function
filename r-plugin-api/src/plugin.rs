use async_trait::async_trait;
use r_error::plugin::error::PluginError;
use sonic_rs::Value;

#[async_trait]
pub trait Plugin: Send + Sync {
    async fn run_plugin(&self, plugin_name: &str, input: Vec<u8>) -> Result<Vec<u8>, PluginError>;

    async fn invoke(&self, plugin_name: &str, value: &[Value]) -> Result<Vec<Value>, PluginError> {
        let payload = sonic_rs::to_vec(value).map_err(|e| PluginError::Encode(e.to_string()))?;
        let out = self.run_plugin(plugin_name, payload).await?;
        sonic_rs::from_slice(&out).map_err(|e| PluginError::Decode(e.to_string()))
    }
}

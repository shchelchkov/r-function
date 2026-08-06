use async_trait::async_trait;
use r_producer::kafka::producer::{DlqContext, KafkaSendError, Producer};
use sonic_rs::Value;

#[async_trait]
pub trait MessagePublisher: Send + Sync {
    async fn send_objects(
        &self,
        setting_code: &str,
        key: Option<&[u8]>,
        objects: &[Value],
    ) -> Result<(), KafkaSendError>;

    async fn send_dlq(
        &self,
        payload: &[u8],
        key: Option<&[u8]>,
        ctx: DlqContext<'_>,
    ) -> Result<(), KafkaSendError>;
}

#[async_trait]
impl MessagePublisher for Producer {
    async fn send_objects(
        &self,
        setting_code: &str,
        key: Option<&[u8]>,
        objects: &[Value],
    ) -> Result<(), KafkaSendError> {
        Producer::send_objects(self, setting_code, key, objects).await
    }

    async fn send_dlq(
        &self,
        payload: &[u8],
        key: Option<&[u8]>,
        ctx: DlqContext<'_>,
    ) -> Result<(), KafkaSendError> {
        Producer::send_dlq(self, payload, key, ctx).await
    }
}

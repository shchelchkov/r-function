use async_trait::async_trait;
use r_producer::kafka::producer::{DlqContext, KafkaSendError, Producer};

#[async_trait]
pub trait MessagePublisher: Send + Sync {
    async fn send_objects(
        &self,
        setting_code: &str,
        key: Option<&[u8]>,
        payload: Vec<u8>,
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
        payload: Vec<u8>,
    ) -> Result<(), KafkaSendError> {
        Producer::send_objects(self, setting_code, key, payload).await
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

use crate::runtime::error::RuntimeError;
use rdkafka::error::KafkaError as RdKafkaError;

#[derive(Debug, thiserror::Error)]
pub enum KafkaError {
    #[error("kafka client: {0}")]
    Client(#[from] RdKafkaError),

    #[error("send failed: {0}")]
    Send(String),

    #[error("commit failed: {0}")]
    Commit(String),

    #[error("subscription failed: {0}")]
    Subscribe(String),

    #[error("payload decode: {0}")]
    Payload(String),

    #[error("runtime: {0}")]
    Runtime(#[from] RuntimeError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use rdkafka::error::KafkaError as RdKafkaError;
    use rdkafka::types::RDKafkaErrorCode;

    #[test]
    fn maps_rdkafka_error_via_from() {
        let rd = RdKafkaError::MessageConsumption(RDKafkaErrorCode::BrokerNotAvailable);
        let err: KafkaError = rd.into();
        let rendered = err.to_string();
        assert!(rendered.starts_with("kafka client"), "got: {rendered}");
    }

    #[test]
    fn variants_render_distinct_prefixes() {
        assert!(KafkaError::Send("x".into()).to_string().starts_with("send"));
        assert!(
            KafkaError::Commit("x".into())
                .to_string()
                .starts_with("commit")
        );
        assert!(
            KafkaError::Subscribe("x".into())
                .to_string()
                .starts_with("subscription")
        );
        assert!(
            KafkaError::Payload("x".into())
                .to_string()
                .starts_with("payload")
        );
    }
}

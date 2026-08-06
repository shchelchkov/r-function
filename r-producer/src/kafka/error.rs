use std::error::Error;
use std::fmt;

use r_error::kafka::KafkaError;

#[derive(Debug)]
pub struct KafkaSendError {
    pub(crate) details: String,
}

impl fmt::Display for KafkaSendError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "KafkaSendError: {}", self.details)
    }
}

impl Error for KafkaSendError {}

impl From<KafkaSendError> for KafkaError {
    fn from(err: KafkaSendError) -> Self {
        KafkaError::Send(err.details)
    }
}

use crate::runtime::error::RuntimeError;

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("NoOutputTopic failed: {0}")]
    NoOutputTopic(String),

    #[error("Payload failed: {0}")]
    Payload(String),

    #[error("Producer failed: {0}")]
    Producer(String),

    #[error("runtime: {0}")]
    Runtime(#[from] RuntimeError),
}

impl ProcessError {
    #[must_use]
    pub fn is_transient(&self) -> bool {
        match self {
            ProcessError::Producer(_) => true,
            ProcessError::Runtime(e) => e.is_transient(),
            ProcessError::NoOutputTopic(_) | ProcessError::Payload(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infra_runtime_failures_are_transient_not_poison() {
        for e in [
            RuntimeError::Trap("pool exhausted".into()),
            RuntimeError::Load("git read".into()),
            RuntimeError::Compile("transient".into()),
            RuntimeError::Internal("join error".into()),
            RuntimeError::Timeout,
        ] {
            assert!(
                ProcessError::Runtime(e).is_transient(),
                "infrastructure failure must hold the offset for redelivery"
            );
        }
    }

    #[test]
    fn guest_rejection_and_bad_payload_are_poison() {
        assert!(!ProcessError::Runtime(RuntimeError::Exit(1)).is_transient());
        assert!(!ProcessError::Runtime(RuntimeError::Decode("bad".into())).is_transient());
        assert!(!ProcessError::Payload("trailing comma".into()).is_transient());
    }

    #[test]
    fn producer_send_is_transient() {
        assert!(ProcessError::Producer("broker down".into()).is_transient());
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("module load: {0}")]
    Git(String),

    #[error("module load: {0}")]
    Load(String),

    #[error("module compile: {0}")]
    Compile(String),

    #[error("guest trap: {0}")]
    Trap(String),

    #[error("guest exit code: {0}")]
    Exit(i32),

    #[error("execution timeout")]
    Timeout,

    #[error("input encode: {0}")]
    Encode(String),

    #[error("output decode: {0}")]
    Decode(String),

    #[error("payload encode: {0}")]
    Payload(String),

    #[error("producer send: {0}")]
    Producer(String),

    #[error("internal: {0}")]
    Internal(String),
}

impl RuntimeError {
    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::Git(_)
                | Self::Load(_)
                | Self::Compile(_)
                | Self::Trap(_)
                | Self::Timeout
                | Self::Producer(_)
                | Self::Internal(_)
        )
    }
}

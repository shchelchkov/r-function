#[derive(Debug, thiserror::Error)]
pub enum PluginError {
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

    #[error("Producer failed: {0}")]
    Producer(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("processing error: {0}")]
    Processing(String),

    #[error("internal error: {0}")]
    Internal(String),

    #[error("encode error: {0}")]
    Encode(String),

    #[error("decode error: {0}")]
    Decode(String),
}


impl PluginError {
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

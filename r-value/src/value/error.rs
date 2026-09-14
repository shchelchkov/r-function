#[derive(Debug, thiserror::Error)]
pub enum ValueError {
    #[error("database error: {0}")]
    Database(#[from] fjall::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] sonic_rs::Error),

    #[error("invalid UTF-8: {0}")]
    InvalidUtf8(#[from] std::str::Utf8Error),

    #[error("value not found: {0}")]
    NotFound(String),

    #[error("internal error: {0}")]
    Internal(String),
}
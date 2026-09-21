#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("database error: {0}")]
    Database(#[from] fjall::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] sonic_rs::Error),

    #[error("invalid UTF-8: {0}")]
    InvalidUtf8(#[from] std::str::Utf8Error),

            #[error("invalid key: {0}")]
    InvalidKey(String),

        #[error("corrupt record: {0}")]
    Corrupt(String),

            #[error("incompatible database format: found {found}, expected {expected}")]
    IncompatibleFormat { found: String, expected: u32 },
}

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("git open: {0}")]
    Open(String),
    #[error("git clone: {0}")]
    Clone(String),
    #[error("git fetch: {0}")]
    Fetch(String),
    #[error("git checkout: {0}")]
    Checkout(String),
    #[error("git rev-parse '{spec}': {cause}")]
    RevParse { spec: String, cause: String },
    #[error("git io: {0}")]
    Io(#[from] std::io::Error),
    #[error("git task: {0}")]
    Task(String),
}

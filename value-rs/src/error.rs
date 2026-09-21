#[derive(Debug, thiserror::Error)]
pub enum ValueRsError {
    #[error("invalid JSON payload: {0}")]
    Json(#[from] sonic_rs::Error),
}

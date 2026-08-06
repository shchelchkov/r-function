use crate::route::error::ApiError;
use crate::route::values::{ApiResponse, ValueEntry};
use axum::{
    Json,
    extract::{Path, State},
};
use r_db::db::db::{DataEntry, Database};

pub async fn get_values(
    State(db): State<Database>,
) -> Result<Json<ApiResponse<Vec<ValueEntry>>>, ApiError> {
    respond(tokio::task::spawn_blocking(move || db.entries()).await)
}

pub async fn get_values_by_setting_code(
    Path(setting_code): Path<String>,
    State(db): State<Database>,
) -> Result<Json<ApiResponse<Vec<ValueEntry>>>, ApiError> {
    respond(tokio::task::spawn_blocking(move || db.entries_by_setting_code(&setting_code)).await)
}

fn respond(
    joined: Result<r_db::db::db::Result<Vec<DataEntry>>, tokio::task::JoinError>,
) -> Result<Json<ApiResponse<Vec<ValueEntry>>>, ApiError> {
    let data = joined
        .map_err(|e| ApiError::Internal(e.to_string()))??
        .into_iter()
        .map(|(key, values)| ValueEntry { key, values })
        .collect();

    Ok(Json(ApiResponse { data }))
}

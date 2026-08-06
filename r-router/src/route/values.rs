use crate::route::error::ApiError;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use r_value::value::value::Values;
use serde::Serialize;
use sonic_rs::Value;
use std::sync::Arc;
use tracing::info;

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

#[derive(Serialize)]
pub struct ValueEntry {
    key: Arc<str>,
    values: Arc<Vec<Value>>,
}

pub async fn get_values(
    Path(setting_code): Path<String>,
    State(values): State<Values>,
) -> Result<Json<ApiResponse<Vec<ValueEntry>>>, ApiError> {
    info!("get_values: {setting_code}");

    let data = values
        .entries()?
        .into_iter()
        .map(|(key, values)| ValueEntry { key, values })
        .collect();

    Ok(Json(ApiResponse { data }))
}

pub async fn get_value(
    Path((setting_code, key)): Path<(String, String)>,
    State(values): State<Values>,
) -> Result<Json<ApiResponse<Arc<Vec<Value>>>>, ApiError> {
    info!("get_value: {setting_code}.{key}");

    let data = values
        .get_value(&setting_code, &key)?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(ApiResponse { data }))
}

pub async fn put_value(
    Path((setting_code, key)): Path<(String, String)>,
    State(values): State<Values>,
    Json(payload): Json<Value>,
) -> Result<StatusCode, ApiError> {
    info!("put_value: {setting_code}.{key}");

    values.put_value(
        &setting_code,
        Arc::from(key.as_str()),
        payload,
    )?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_value(
    Path((setting_code, key)): Path<(String, String)>,
    State(values): State<Values>,
) -> Result<StatusCode, ApiError> {
    info!("delete_value: {setting_code}.{key}");

    if values.remove_value(&setting_code, &key)? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Ok(StatusCode::NOT_FOUND)
    }
}

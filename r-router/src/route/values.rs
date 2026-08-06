use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
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
) -> Json<ApiResponse<Vec<ValueEntry>>> {
    info!("get_values: {setting_code}");
    let data = values
        .entries()
        .into_iter()
        .map(|(key, values)| ValueEntry { key, values })
        .collect();
    Json(ApiResponse { data })
}

pub async fn get_value(
    Path((setting_code, key)): Path<(String, String)>,
    State(values): State<Values>,
) -> Result<Json<ApiResponse<Arc<Vec<Value>>>>, StatusCode> {
    info!("get_value: {setting_code}.{key}");
    values
        .get_value(&setting_code, &key)
        .map(|data| Json(ApiResponse { data }))
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn put_value(
    Path((setting_code, key)): Path<(String, String)>,
    State(values): State<Values>,
    Json(payload): Json<Value>,
) -> StatusCode {
    info!("put_value: {setting_code}");
    values.put_value(&setting_code, Arc::from(key.as_str()), payload);
    StatusCode::NO_CONTENT
}

pub async fn delete_value(
    Path((setting_code, key)): Path<(String, String)>,
    State(values): State<Values>,
) -> StatusCode {
    info!("delete_value: {setting_code}");
    if values.remove_value(&setting_code, &key) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

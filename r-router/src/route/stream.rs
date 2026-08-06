use crate::route::values::ApiResponse;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use r_setting::streams::stream::Stream;
use r_setting::streams::stream_setting::StreamSetting;
use serde::Serialize;
use std::sync::Arc;
use r_setting::git::SettingEntry;

pub async fn get_stream_setting(
    Path(setting_code): Path<String>,
    State(stream): State<Stream>,
) -> Result<Json<ApiResponse<Arc<Vec<StreamSetting>>>>, StatusCode> {
    let settings = tokio::task::spawn_blocking(move || stream.get_stream_setting(&setting_code))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(ApiResponse { data: settings }))
}

pub async fn get_values(
    State(values): State<Stream>,
) -> Json<ApiResponse<Vec<SettingEntry<StreamSetting>>>> {
    let data = values
        .entries()
        .into_iter()
        .map(|(key, values)| SettingEntry {
            key: key,
            values: values,
        })
        .collect();
    Json(ApiResponse { data })
}

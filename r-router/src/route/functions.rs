use crate::route::types::FluxByMapQuery;
use crate::route::values::ApiResponse;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use r_setting::functions::function_setting::FunctionSetting;
use r_setting::functions::functions::Function;
use r_setting::functions::functions_value::FunctionValue;
use sonic_rs::Value;
use std::sync::Arc;
use tracing::info;

pub async fn get_function_setting(
    Path(setting_code): Path<String>,
    State(function): State<Function>,
) -> Result<Json<ApiResponse<Arc<Vec<FunctionSetting>>>>, StatusCode> {
    let settings =
        tokio::task::spawn_blocking(move || function.get_function_setting(&setting_code))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(ApiResponse { data: settings }))
}

pub async fn get_function_value(
    Path((setting_code, key)): Path<(String, String)>,
    State(function_value): State<FunctionValue>,
) -> Result<Json<Arc<Vec<Value>>>, StatusCode> {
    info!("GET::get_function_value: {setting_code}.{key}");
    let data =
        tokio::task::spawn_blocking(move || function_value.get_function_value(&setting_code, &key))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(data))
}

pub async fn get_function_request(
    Query(FluxByMapQuery { setting_code }): Query<FluxByMapQuery>,
    State(consumer): State<Function>,
) -> Result<Json<Arc<Vec<FunctionSetting>>>, StatusCode> {
    let settings =
        tokio::task::spawn_blocking(move || consumer.get_function_setting(&setting_code))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(settings))
}

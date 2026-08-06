use crate::route::types::FluxByMapQuery;
use crate::route::values::ApiResponse;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use r_setting::consumers::consumer::Consumer;
use r_setting::consumers::consumer_setting::ConsumerSetting;
use std::sync::Arc;

pub async fn get_consumer_setting(
    Path(setting_code): Path<String>,
    State(consumer): State<Consumer>,
) -> Result<Json<ApiResponse<Arc<Vec<ConsumerSetting>>>>, StatusCode> {
    let settings =
        tokio::task::spawn_blocking(move || consumer.get_consumer_setting(&setting_code))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(ApiResponse { data: settings }))
}

pub async fn get_consumer_request(
    Query(FluxByMapQuery { setting_code }): Query<FluxByMapQuery>,
    State(consumer): State<Consumer>,
) -> Result<Json<Arc<Vec<ConsumerSetting>>>, StatusCode> {
    let settings =
        tokio::task::spawn_blocking(move || consumer.get_consumer_setting(&setting_code))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(settings))
}

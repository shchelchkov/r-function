use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use r_setting::catalogs::catalog::Catalog;
use r_setting::catalogs::catalog_setting::CatalogSetting;
use std::sync::Arc;

use crate::route::types::FluxByMapQuery;
use crate::route::values::ApiResponse;

pub async fn get_catalog_setting(
    Path(setting_code): Path<String>,
    State(catalog): State<Catalog>,
) -> Result<Json<ApiResponse<Arc<Vec<CatalogSetting>>>>, StatusCode> {
    let settings = tokio::task::spawn_blocking(move || catalog.get_catalog_setting(&setting_code))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(ApiResponse { data: settings }))
}

pub async fn get_catalog_request(
    Query(FluxByMapQuery { setting_code }): Query<FluxByMapQuery>,
    State(catalog): State<Catalog>,
) -> Result<Json<Arc<Vec<CatalogSetting>>>, StatusCode> {
    let settings = tokio::task::spawn_blocking(move || catalog.get_catalog_setting(&setting_code))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(settings))
}

use crate::route::error::ApiError;
use crate::route::values::ApiResponse;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use r_db::db::db::{DataEntry, Database};
use r_db::db::history::{HistoryEntry, HistoryQuery};
use r_db::db::registry::{KeyEntry, KeyMeta};
use serde::{Deserialize, Serialize};
use sonic_rs::Value;
use std::sync::Arc;

#[derive(Serialize)]
pub struct DbEntry {
    pub setting_code: Arc<str>,
    pub key: Arc<str>,
    pub values: Arc<Vec<Value>>,
}

impl From<DataEntry> for DbEntry {
    fn from(entry: DataEntry) -> Self {
        Self {
            setting_code: entry.setting_code,
            key: entry.key,
            values: entry.values,
        }
    }
}

#[derive(Serialize)]
pub struct RegistryEntry {
    pub setting_code: Arc<str>,
    pub key: Arc<str>,
    #[serde(flatten)]
    pub meta: KeyMeta,
}

impl From<KeyEntry> for RegistryEntry {
    fn from(entry: KeyEntry) -> Self {
        Self {
            setting_code: entry.setting_code,
            key: entry.key,
            meta: entry.meta,
        }
    }
}

#[derive(Deserialize)]
pub struct HistoryParams {
    pub from: Option<u64>,
    pub to: Option<u64>,
    pub limit: Option<usize>,
    #[serde(default)]
    pub order: Order,
}

#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Order {
    #[default]
    Asc,
    Desc,
}

pub async fn get_registry(
    State(db): State<Database>,
) -> Result<Json<ApiResponse<Vec<RegistryEntry>>>, ApiError> {
    let data = blocking(move || db.registry()).await?;
    Ok(Json(ApiResponse {
        data: data.into_iter().map(RegistryEntry::from).collect(),
    }))
}

pub async fn get_registry_by_setting_code(
    Path(setting_code): Path<String>,
    State(db): State<Database>,
) -> Result<Json<ApiResponse<Vec<RegistryEntry>>>, ApiError> {
    let data = blocking(move || db.keys(&setting_code)).await?;
    Ok(Json(ApiResponse {
        data: data.into_iter().map(RegistryEntry::from).collect(),
    }))
}

pub async fn get_values(
    State(db): State<Database>,
) -> Result<Json<ApiResponse<Vec<DbEntry>>>, ApiError> {
    let data = blocking(move || db.entries()).await?;
    Ok(Json(ApiResponse {
        data: data.into_iter().map(DbEntry::from).collect(),
    }))
}

pub async fn get_values_by_setting_code(
    Path(setting_code): Path<String>,
    State(db): State<Database>,
) -> Result<Json<ApiResponse<Vec<DbEntry>>>, ApiError> {
    let data = blocking(move || db.entries_by_setting_code(&setting_code)).await?;
    Ok(Json(ApiResponse {
        data: data.into_iter().map(DbEntry::from).collect(),
    }))
}

pub async fn get_value(
    Path((setting_code, key)): Path<(String, String)>,
    State(db): State<Database>,
) -> Result<Json<ApiResponse<Arc<Vec<Value>>>>, ApiError> {
    let data = blocking(move || db.get_value(&setting_code, &key))
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(ApiResponse { data }))
}

pub async fn delete_value(
    Path((setting_code, key)): Path<(String, String)>,
    State(db): State<Database>,
) -> Result<StatusCode, ApiError> {
    if blocking(move || db.remove_value(&setting_code, &key)).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}

pub async fn get_history(
    Path((setting_code, key)): Path<(String, String)>,
    Query(params): Query<HistoryParams>,
    State(db): State<Database>,
) -> Result<Json<ApiResponse<Vec<HistoryEntry>>>, ApiError> {
    let query = HistoryQuery {
        from: params.from,
        to: params.to,
        limit: params.limit,
        newest_first: matches!(params.order, Order::Desc),
    };
    let data = blocking(move || db.get_history(&setting_code, &key, query)).await?;
    Ok(Json(ApiResponse { data }))
}

async fn blocking<T, F>(f: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce() -> r_db::db::db::Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .map_err(ApiError::from)
}

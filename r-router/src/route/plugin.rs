use crate::route::values::ApiResponse;
use axum::Json;
use axum::extract::{Path, State};
use r_plugin::plugin::plugin_module::PluginModule;
use tracing::info;
use crate::route::state::AppState;

pub async fn get_plugin_cache(
    Path(setting_code): Path<String>,
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<String>>> {
    info!("get_values: {setting_code}");
    let data = state
        .plugin_module
        .entries_plugin_cache();

    Json(ApiResponse { data })
}

pub async fn get_plugin_resolve_cache(
    Path(setting_code): Path<String>,
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<String>>> {
    info!("get_values: {setting_code}");
    let data = state
        .plugin_module
        .entries_resolve_cache();

    Json(ApiResponse { data })
}

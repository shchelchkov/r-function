use crate::route::values::ApiResponse;
use axum::Json;
use axum::extract::{Path, State};
use r_plugin::plugin::plugin_module::PluginModule;
use tracing::info;
use crate::route::state::AppState;

pub async fn get_module_cache(
    Path(setting_code): Path<String>,
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<String>>> {
    info!("get_wasm_list: {setting_code}");
    let data = state
        .wasm_runtime
        .entries_module_cache();

    Json(ApiResponse { data })
}

pub async fn get_module_resolve_cache(
    Path(setting_code): Path<String>,
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<String>>> {
    info!("get_module_resolve_cache: {setting_code}");
    let data = state
        .wasm_runtime
        .entries_resolve_cache();

    Json(ApiResponse { data })
}

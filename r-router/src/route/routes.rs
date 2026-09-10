use crate::route::catalog::{get_catalog_request, get_catalog_setting};
use crate::route::consumer::{get_consumer_request, get_consumer_setting};
use crate::route::functions::{get_function_request, get_function_setting, get_function_value};
use crate::route::git::{get_git_head, post_git_refresh};
use crate::route::plugin::{get_plugin_cache, get_plugin_resolve_cache};
use crate::route::state::AppState;
use crate::route::stream::get_stream_setting;
use crate::route::values::{delete_value, get_value, get_values, put_value};
use crate::route::wasm::{get_module_cache, get_module_resolve_cache};
use crate::route::ws::ws_handler;
use axum::{
    Router,
    routing::{get, post},
};
use r_setting::{catalogs, consumers, streams};
use crate::route::{catalog, consumer, functions, stream};

pub fn build_router(state: AppState, prefix_functions: &str, prefix_directory: &str) -> Router {
    Router::new()
        .nest(prefix_functions, api_functions())
        .nest(prefix_directory, api_directory())
        .route("/ws", get(ws_handler))
        .with_state(state)
}

fn api_functions() -> Router<AppState> {
    Router::new()
        .nest("/setting", setting_routes())
        .route("/value/{setting_code}/{key}", get(get_function_value))
        .route("/values/{setting_code}", get(get_values))
        .route(
            "/values/{setting_code}/{key}",
            get(get_value).put(put_value).delete(delete_value),
        )
        .route("/git/refresh", post(post_git_refresh))
        .route("/git/head", get(get_git_head))
}

fn api_directory() -> Router<AppState> {
    Router::new()
        .route("/setting/catalog/findAll", get(get_catalog_request))
        .route("/setting/catalog/fluxByMap", get(get_catalog_request))
        .route("/setting/consumer/fluxByMap", get(get_consumer_request))
        .route("/setting/function/fluxByMap", get(get_function_request))
}

fn setting_routes() -> Router<AppState> {
    Router::new()
        .route("/functions/{setting_code}", get(get_function_setting))
        .route("/catalogs/{setting_code}", get(get_catalog_setting))
        .route("/consumers/{setting_code}", get(get_consumer_setting))
        .route("/streams/{setting_code}", get(get_stream_setting))
        .route("/catalog/{setting_code}", get(get_catalog_setting))
        .route("/consumer/{setting_code}", get(get_consumer_setting))
        
        .route("/functions", get(functions::get_values))
        .route("/catalogs", get(catalog::get_values))
        .route("/consumers", get(consumer::get_values))
        .route("/streams", get(stream::get_values))

        .route("/plugin/cache/{setting_code}", get(get_plugin_cache))
        .route("/plugin/resolve/{setting_code}", get(get_plugin_resolve_cache))
        .route("/module/cache/{setting_code}", get(get_module_cache))
        .route("/module/resolve/{setting_code}", get(get_module_resolve_cache))
}

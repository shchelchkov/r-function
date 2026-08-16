use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;

use r_config::config::TokioConfig;
use r_consumer::functions::catalogs::catalog::Catalog;
use r_consumer::functions::consumers::consumer::Consumer;
use r_consumer::functions::functions::functions::Function;
use r_consumer::functions::functions::functions_value::FunctionValue;
use r_consumer::functions::git::GitHandle;
use r_consumer::functions::streams::stream::Stream;
use r_feed::FeedHub;
use r_router::route::routes;
use r_router::route::state::AppState;
use r_value::value::value::Values;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tracing::info;
use r_plugin::plugin::plugin_module::PluginModule;
use r_runtime::runtime::wasm::WasmRuntime;
use r_tree::value::polygon::Polygon;

#[allow(clippy::too_many_arguments)]
pub async fn serve(
    cfg: &TokioConfig,
    git: Arc<GitHandle>,
    function: Function,
    function_value: FunctionValue,
    catalog: Catalog,
    consumer: Consumer,
    stream: Stream,
    values: Values,
    polygon: Polygon,
    feed: FeedHub,
    plugin_module: Arc<PluginModule>,
    wasm_runtime: Arc<WasmRuntime>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), Box<dyn Error>> {
    let ws_shutdown = shutdown.clone();
    let app_state = AppState {
        values,
        git,
        function,
        function_value,
        catalog,
        consumer,
        stream,
        polygon,
        feed,
        plugin_module,
        wasm_runtime,
        shutdown: ws_shutdown,
    };
    let app = routes::build_router(app_state, &cfg.api_prefix, &cfg.api_directory);

    info!("Server run on {}", cfg.port);

    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.port));
    let listener = TcpListener::bind(addr).await?;
    info!("Server running on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown.changed().await;
        })
        .await?;
    Ok(())
}

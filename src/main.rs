mod app;
mod bootstrap;
mod http;
mod pipeline;
mod shutdown;
mod telemetry;

use std::error::Error;
use std::thread::available_parallelism;

use r_config::config::AppConfig;
use tracing::info;

fn main() -> Result<(), Box<dyn Error>> {
    telemetry::init();

    let cfg_path = std::env::var("RF_CONFIG").unwrap_or_else(|_| "config.yaml".to_string());
    info!("::::::::::::::::::main::cfg_path : {}", &cfg_path);
    let cfg = AppConfig::from_file(&cfg_path)?;

    let worker_threads = cfg
        .tokio
        .worker_threads
        .unwrap_or_else(|| available_parallelism().map(|n| n.get()).unwrap_or(4));
    info!("main::tokio worker_threads: {}", worker_threads);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .enable_all()
        .build()?;

    rt.block_on(app::run(cfg))
}

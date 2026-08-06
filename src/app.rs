use std::error::Error;
use std::time::Duration;

use r_config::config::AppConfig;
use r_consumer::functions::git::spawn_git_refresher;

use r_watchdog::Watchdog;

use crate::bootstrap::{self, Components};
use crate::http;
use crate::pipeline::Pipeline;
use crate::shutdown::Shutdown;

pub async fn run(cfg: AppConfig) -> Result<(), Box<dyn Error>> {
    let shutdown = Shutdown::install();

    let Components {
        consumer,
        git,
        processor,
        runtime,
        observers,
        function,
        function_value,
        catalog,
        consumer_setting,
        stream,
        values,
        feed,
    } = bootstrap::build(&cfg).await?;

    let refresher = spawn_git_refresher(
        git.clone(),
        Duration::from_secs(cfg.function_config.git_fetch_interval_secs),
        observers,
        shutdown.subscribe(),
    );

    let watchdog_task = cfg
        .watchdog
        .clone()
        .map(|wd| Watchdog::new(wd, runtime.clone()).spawn(shutdown.subscribe()));

    let pipeline = Pipeline::spawn(&cfg.pipeline, processor);

    let consumer_task = tokio::spawn({
        let ingress = pipeline.ingress();
        let max_inflight = cfg.pipeline.ingress_queue;
        let mut sd = shutdown.subscribe();
        async move {
            consumer
                .run(ingress, max_inflight, async move {
                    let _ = sd.changed().await;
                })
                .await
        }
    });

    http::serve(
        &cfg.tokio,
        git,
        function,
        function_value,
        catalog,
        consumer_setting,
        stream,
        values,
        feed,
        shutdown.subscribe(),
    )
    .await?;

    if let Err(e) = consumer_task.await? {
        tracing::error!(error = %e, "consumer task failed");
    }
    pipeline.drain().await;
    let _ = refresher.await;
    if let Some(task) = watchdog_task {
        let _ = task.await;
    }

    Ok(())
}

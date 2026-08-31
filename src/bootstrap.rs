use std::error::Error;
use std::sync::Arc;

use r_config::config::AppConfig;
use r_consumer::error::runtime::error::RuntimeError;
use r_consumer::functions::catalogs::catalog::Catalog;
use r_consumer::functions::consumers::consumer::Consumer as SettingConsumer;
use r_consumer::functions::functions::functions::Function;
use r_consumer::functions::functions::functions_value::FunctionValue;
use r_consumer::functions::git::{GitHandle, HeadObserver, git_sync};
use r_consumer::functions::streams::stream::Stream;
use r_consumer::kafka::Consumer;
use r_consumer::process::process::Processor;
use r_consumer::process::producer::kafka::producer::Producer;
use r_feed::FeedHub;
use r_plugin::plugin::loader::GitPluginLoader;
use r_plugin::plugin::plugin_module::PluginModule;
use r_runtime::runtime::loader::GitModuleLoader;
use r_runtime::runtime::wasm::WasmRuntime;
use r_tree::value::polygon::Polygon;
use r_value::value::value::Values;

pub struct Components {
    pub consumer: Consumer,
    pub git: Arc<GitHandle>,
    pub processor: Arc<Processor>,
    pub runtime: Arc<WasmRuntime>,
    pub plugin_module: Arc<PluginModule>,
    pub observers: Vec<Arc<dyn HeadObserver>>,
    pub function: Function,
    pub function_value: FunctionValue,
    pub catalog: Catalog,
    pub consumer_setting: SettingConsumer,
    pub stream: Stream,
    pub values: Values,
    pub polygon: Polygon,
    pub feed: FeedHub,
}

pub async fn build(cfg: &AppConfig) -> Result<Components, Box<dyn Error>> {
    let consumer = Consumer::new(&cfg.kafka_consumer)?;

    let git: Arc<GitHandle> = tokio::task::spawn_blocking({
        let function_config = cfg.function_config.clone();
        move || git_sync(&function_config)
    })
    .await??;

    let function = Function::new(git.clone(), &cfg.function_config);
    let function_value =
        FunctionValue::new(function.settings_store(), git.clone(), &cfg.function_config);
    let catalog = Catalog::new(git.clone(), &cfg.function_config);
    let consumer_setting = SettingConsumer::new(git.clone(), &cfg.function_config);
    let stream = Stream::new(git.clone(), &cfg.function_config);
    let feed = FeedHub::new();
    let producer = Producer::new(&cfg.kafka_producer, stream.clone())?.with_feed(feed.clone());
    let watchdog_slots = u32::from(cfg.watchdog.is_some());

    let head = **git.head.load();
    let loader = GitModuleLoader::new(git.clone(), &cfg.function_config);
    let max_instances = cfg.pipeline.concurrency as u32 + watchdog_slots + 2;

    let values = Values::new();
    let polygon = Polygon::new();
    let runtime = Arc::new(
        WasmRuntime::new(
            loader,
            head,
            function.clone(),
            function_value.clone(),
            stream.clone(),
            values.clone(),
            polygon.clone(),
            producer.clone(),
            max_instances,
        )
        .map_err(|e| RuntimeError::Internal(e.to_string()))?,
    );

    let loader = GitPluginLoader::new(git.clone(), &cfg.function_config);

    let plugin_module = Arc::new(PluginModule::new(
        loader,
        head,
        function.clone(),
        function_value.clone(),
        stream.clone(),
        values.clone(),
        polygon.clone(),
        producer.clone(),
        max_instances,
    )?);

    let processor = Arc::new(Processor::new(
        Arc::new(producer),
        Arc::new(function.clone()),
        runtime.clone(),
        plugin_module.clone(),
    ));

    let observers = build_observers(
        function.clone(),
        function_value.clone(),
        stream.clone(),
        runtime.clone(),
        catalog.clone(),
        consumer_setting.clone(),
        plugin_module.clone(),
    );

    Ok(Components {
        consumer,
        git,
        processor,
        runtime,
        plugin_module,
        observers,
        function,
        function_value,
        catalog,
        consumer_setting,
        stream,
        values,
        polygon,
        feed,
    })
}

fn build_observers(
    function: Function,
    function_value: FunctionValue,
    stream: Stream,
    runtime: Arc<WasmRuntime>,
    catalog: Catalog,
    consumer: SettingConsumer,
    plugin_module: Arc<PluginModule>,
) -> Vec<Arc<dyn HeadObserver>> {
    vec![
        Arc::new(function) as Arc<dyn HeadObserver>,
        Arc::new(function_value),
        Arc::new(stream),
        runtime,
        Arc::new(catalog),
        Arc::new(consumer),
        plugin_module,
    ]
}

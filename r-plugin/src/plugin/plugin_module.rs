use crate::plugin::{PluginContext, SendValue};
use crate::plugin::executor::PluginExecutor;
use crate::plugin::loader::PluginLoader;
use crate::plugin::plugin_repository::PluginRepository;
use async_trait::async_trait;
use r_plugin_api::{Plugin, PluginError};
use r_producer::kafka::producer::Producer;
use r_setting::functions::functions::Function;
use r_setting::functions::functions_value::FunctionValue;
use r_setting::git::HeadObserver;
use r_setting::streams::stream::Stream;
use r_value::value::value::Values;
use std::sync::Arc;
use r_error::runtime::error::RuntimeError;
use r_producer::host::send_pipeline::SendPipeline;
use r_tree::value::polygon::Polygon;

pub struct PluginModule {
    shared: Arc<Shared>,
}

struct Shared {
    executor: PluginExecutor,
    repo: Arc<PluginRepository>,
    _send: SendPipeline,
}

impl Clone for PluginModule {
    fn clone(&self) -> Self {
        PluginModule {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl PluginModule {
    #[allow(clippy::too_many_arguments)]
    pub fn new<L: PluginLoader + 'static>(
        loader: L,
        head: gix::ObjectId,
        function: Function,
        function_value: FunctionValue,
        stream: Stream,
        values: Values,
        polygon: Polygon,
        producer: Producer,
        max_instances: u32,
    ) -> Result<Self, PluginError> {
        let executor = PluginExecutor::new()?;

        let (send, send_txs) = SendPipeline::new(producer.clone());
        let send_value = SendValue { txs: send_txs };

        let context = Arc::new(PluginContext {
            function: function.clone(),
            function_value: function_value.clone(),
            stream: stream.clone(),
            values: values.clone(),
            polygon: polygon.clone(),
            producer: producer.clone(),
            send_value,
        });

        let repo = Arc::new(PluginRepository::new(
            Arc::new(loader),
            context,
        ));

        Ok(Self {
            shared: Arc::new(Shared {
                executor,
                repo,
                _send: send,
            }),
        })
    }

    pub fn entries_plugin_cache(&self) -> Vec<String> {
        self.shared.repo.entries_plugin_cache()
    }
    pub fn entries_resolve_cache(&self) -> Vec<String> {
        self.shared.repo.entries_resolve_cache()
    }
}

impl HeadObserver for PluginModule {
    fn on_revision_changed(&self, new_head: gix::ObjectId, changed_paths: &[String]) {
        self.shared.repo.on_head_changed(new_head, changed_paths);
    }
}

#[async_trait]
impl Plugin for PluginModule {
    async fn run_plugin(
        &self,
        plugin_name: &str,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, RuntimeError> {
        let pre = self.shared.repo.get(plugin_name).await?;
        let ctx = self.shared.repo.context();
        self.shared.executor.run(pre, payload, ctx).await
    }
}


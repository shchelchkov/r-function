use std::sync::Arc;

use crate::runtime::executor::{StoreCtx, WasmExecutor};
use crate::runtime::host::{
    GetFunctionSetting, GetFunctionValue, GetStreamSetting, GetValue, HostRegistry, HttpRequest,
    PutValue, RemoveValue, SendValue,
};
use crate::runtime::module_repository::WasmModuleRepository;
use crate::runtime::send_pipeline::SendPipeline;
use crate::runtime::loader::ModuleLoader;
use async_trait::async_trait;
use r_runtime_api::Runtime;
use r_error::runtime::error::RuntimeError;
use r_producer::kafka::producer::Producer;
use r_setting::functions::functions::Function;
use r_setting::functions::functions_value::FunctionValue;
use r_setting::git::HeadObserver;
use r_setting::streams::stream::Stream;
use r_value::value::value::Values;
use tracing::debug;
use wasmtime::Linker;

pub struct WasmRuntime {
    shared: Arc<Shared>,
}

struct Shared {
    executor: WasmExecutor,
    repo: Arc<WasmModuleRepository>,
    _send: SendPipeline,
}

impl WasmRuntime {
    #[allow(clippy::too_many_arguments)]
    pub fn new<L: ModuleLoader + 'static>(
        loader: L,
        head: gix::ObjectId,
        function: Function,
        function_value: FunctionValue,
        stream: Stream,
        values: Values,
        producer: Producer,
        max_instances: u32,
    ) -> Result<Self, RuntimeError> {
        let executor = WasmExecutor::new(max_instances)?;

        let mut linker = Linker::<StoreCtx>::new(executor.engine());
        wasmtime_wasi::p1::add_to_linker_async(&mut linker, |c: &mut StoreCtx| &mut c.wasi)
            .map_err(|e| RuntimeError::Load(e.to_string()))?;

        let mut registry = HostRegistry::new();
        registry.register(GetFunctionSetting {
            function: function.clone(),
        });
        registry.register(GetValue {
            values: values.clone(),
            function_value: function_value.clone(),
        });
        registry.register(GetFunctionValue { function_value });
        registry.register(GetStreamSetting { stream });
        registry.register(PutValue {
            values: values.clone(),
        });
        registry.register(RemoveValue { values });
        registry.register(HttpRequest {
            client: reqwest::Client::new(),
        });

        let (send, send_txs) = SendPipeline::new(producer);
        registry.register(SendValue { txs: send_txs });

        registry.install(&mut linker)?;

        let repo = Arc::new(WasmModuleRepository::new(
            executor.engine().clone(),
            Arc::new(linker),
            Arc::new(loader),
            head,
        ));

        Ok(Self {
            shared: Arc::new(Shared {
                executor,
                repo,
                _send: send,
            }),
        })
    }
}

impl HeadObserver for WasmRuntime {
    fn on_revision_changed(&self, new_head: gix::ObjectId, changed_paths: &[String]) {
        self.shared.repo.on_head_changed(new_head, changed_paths);
    }
}

#[async_trait]
impl Runtime for WasmRuntime {
    async fn invoke_raw(
        &self,
        module_name: &str,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, RuntimeError> {
        debug!(module_name, "invoke");
        let pre = self.shared.repo.instance_pre(module_name).await?;
        self.shared.executor.run(&pre, payload).await
    }
}

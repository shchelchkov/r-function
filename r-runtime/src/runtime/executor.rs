use std::time::Duration;

use r_error::runtime::error::RuntimeError;
use tokio::task::AbortHandle;
use wasmtime::{
    Engine, InstanceAllocationStrategy, InstancePre, PoolingAllocationConfig, Store, StoreLimits,
    StoreLimitsBuilder,
};
use wasmtime_wasi::{
    I32Exit, WasiCtxBuilder,
    p1::WasiP1Ctx,
    p2::pipe::{MemoryInputPipe, MemoryOutputPipe},
};

const MEMORY_LIMIT_BYTES: usize = 64 * 1024 * 1024;
const TABLE_ELEMENTS_LIMIT: usize = 10_000;
const INSTANCES_LIMIT: usize = 1;
const MEMORIES_LIMIT: usize = 1;
const TABLES_LIMIT: usize = 1;
const STDOUT_BUF_BYTES: usize = 4 * 1024 * 1024;
const STDERR_BUF_BYTES: usize = 64 * 1024;
const EPOCH_TICK: Duration = Duration::from_millis(10);
const EPOCH_DEADLINE_TICKS: u64 = 500; 
const MIN_CONCURRENT_INSTANCES: u32 = 4;

pub struct StoreCtx {
    pub wasi: WasiP1Ctx,
    pub limits: StoreLimits,
}

pub struct WasmExecutor {
    engine: Engine,
    epoch_abort: AbortHandle,
}

impl WasmExecutor {

    pub fn new(max_instances: u32) -> Result<Self, RuntimeError> {
        let max_instances = max_instances.max(MIN_CONCURRENT_INSTANCES);
        let mut pool = PoolingAllocationConfig::default();
        pool.total_memories(max_instances)
            .total_tables(max_instances)
            .total_core_instances(max_instances)
            .total_stacks(max_instances.saturating_mul(2))
            .max_memory_size(MEMORY_LIMIT_BYTES)
            .table_elements(TABLE_ELEMENTS_LIMIT);

        let mut cfg = wasmtime::Config::new();
        cfg.epoch_interruption(true)
            .allocation_strategy(InstanceAllocationStrategy::Pooling(pool));
        let engine = Engine::new(&cfg).map_err(|e| RuntimeError::Internal(e.to_string()))?;

        let epoch_abort = spawn_epoch_ticker(engine.clone(), EPOCH_TICK);
        Ok(Self {
            engine,
            epoch_abort,
        })
    }

    #[must_use]
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub async fn run(
        &self,
        pre: &InstancePre<StoreCtx>,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, RuntimeError> {
        let stdout = MemoryOutputPipe::new(STDOUT_BUF_BYTES);
        let stderr = MemoryOutputPipe::new(STDERR_BUF_BYTES);
        let wasi = WasiCtxBuilder::new()
            .stdin(MemoryInputPipe::new(payload))
            .stdout(stdout.clone())
            .stderr(stderr.clone())
            .build_p1();

        let limits = StoreLimitsBuilder::new()
            .memory_size(MEMORY_LIMIT_BYTES)
            .table_elements(TABLE_ELEMENTS_LIMIT)
            .instances(INSTANCES_LIMIT)
            .memories(MEMORIES_LIMIT)
            .tables(TABLES_LIMIT)
            .build();

        let mut store = Store::new(&self.engine, StoreCtx { wasi, limits });
        store.set_epoch_deadline(EPOCH_DEADLINE_TICKS);
        store.limiter(|c: &mut StoreCtx| &mut c.limits);

        let inst = pre
            .instantiate_async(&mut store)
            .await
            .map_err(|e| RuntimeError::Trap(e.to_string()))?;
        let f = inst
            .get_typed_func::<(), ()>(&mut store, "_start")
            .map_err(|e| RuntimeError::Load(e.to_string()))?;

        let result = f.call_async(&mut store, ()).await;

        let errs = stderr.contents();
        if !errs.is_empty() {
            tracing::warn!(stderr = %String::from_utf8_lossy(&errs), "wasm guest stderr");
        }

        match result {
            Ok(()) => Ok(stdout.contents().to_vec()),
            Err(e) => match e.downcast_ref::<I32Exit>() {
                Some(I32Exit(0)) => Ok(stdout.contents().to_vec()),
                Some(I32Exit(code)) => Err(RuntimeError::Exit(*code)),
                None => Err(RuntimeError::Trap(format!("{e:#}"))),
            },
        }
    }
}

impl Drop for WasmExecutor {
    fn drop(&mut self) {
        self.epoch_abort.abort();
    }
}

fn spawn_epoch_ticker(engine: Engine, interval: Duration) -> AbortHandle {
    let h = tokio::spawn(async move {
        let mut t = tokio::time::interval(interval);
        loop {
            t.tick().await;
            engine.increment_epoch();
        }
    });
    h.abort_handle()
}

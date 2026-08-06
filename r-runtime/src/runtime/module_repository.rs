use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use gix::ObjectId;
use moka::future::Cache;
use r_error::runtime::error::RuntimeError;
use tokio::task::spawn_blocking;
use tracing::{info, warn};
use wasmtime::{InstancePre, Linker, Module};

use crate::runtime::executor::StoreCtx;
use crate::runtime::loader::ModuleLoader;

pub struct WasmModuleRepository {
    engine: wasmtime::Engine,
    linker: Arc<Linker<StoreCtx>>,
    resolve_cache: Cache<Arc<str>, ObjectId>,
    module_cache: Cache<Arc<str>, InstancePre<StoreCtx>>,
    loader: Arc<dyn ModuleLoader>,
    revision: ArcSwap<ObjectId>,
}

impl WasmModuleRepository {
    pub fn new(
        engine: wasmtime::Engine,
        linker: Arc<Linker<StoreCtx>>,
        loader: Arc<dyn ModuleLoader>,
        head: ObjectId,
    ) -> Self {
        let resolve_cache = Cache::builder()
            .max_capacity(10_000)
            .time_to_idle(Duration::from_secs(300))
            .build();
        let module_cache = Cache::builder()
            .max_capacity(256)
            .time_to_idle(Duration::from_secs(3600))
            .build();
        Self {
            engine,
            linker,
            resolve_cache,
            module_cache,
            loader,
            revision: ArcSwap::from_pointee(head),
        }
    }

        pub async fn instance_pre(
        self: &Arc<Self>,
        module_name: &str,
    ) -> Result<InstancePre<StoreCtx>, RuntimeError> {
        let oid = match self.resolve_cache.get(module_name).await {
            Some(oid) => oid,
            None => self
                .resolve_cache
                .try_get_with(Arc::<str>::from(module_name), {
                    let this = self.clone();
                    let name = module_name.to_owned();
                    async move { this.resolve_oid(name).await }
                })
                .await
                .map_err(|e: Arc<RuntimeError>| RuntimeError::Load(e.to_string()))?,
        };

        match self.module_cache.get(module_name).await {
            Some(pre) => Ok(pre),
            None => self
                .module_cache
                .try_get_with(Arc::<str>::from(module_name), {
                    let this = self.clone();
                    async move { this.compile(oid).await }
                })
                .await
                .map_err(|e: Arc<RuntimeError>| RuntimeError::Compile(e.to_string())),
        }
    }

    pub fn on_head_changed(self: &Arc<Self>, new_head: ObjectId, changed_paths: &[String]) {
        info!("on_head_changed start");
        let wasm_path = match self.loader.git_wasm_path() {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "git_wasm_path failed; skipping wasm cache refresh");
                return;
            }
        };
        let prefix = format!("{}/", wasm_path);
        let mut changed: Vec<String> = Vec::new();
        for path in changed_paths {
            if let Some(rest) = path.strip_prefix(&prefix)
                && rest.ends_with(".wasm")
            {
                changed.push(rest.to_owned());
            }
        }
        if changed.is_empty() {
            return;
        }

        self.revision.store(Arc::new(new_head));

        let this = self.clone();
        tokio::spawn(async move {
            for name in changed {
                let key: Arc<str> = Arc::from(name.as_str());
                this.resolve_cache.invalidate(&key).await;
                this.module_cache.invalidate(&key).await;
                match this.refresh(&name).await {
                    Ok(oid) => info!(module = %name, %oid, "on_head_changed wasm module refreshed"),
                    Err(e) => {
                        warn!(module = %name, error = %e, "on_head_changed wasm module refresh failed")
                    }
                }
            }
        });
    }

    async fn refresh(self: &Arc<Self>, module_name: &str) -> Result<ObjectId, RuntimeError> {
        let key: Arc<str> = Arc::from(module_name);

        let oid: ObjectId = self
            .resolve_cache
            .entry_by_ref(&key)
            .or_try_insert_with({
                let this = self.clone();
                let name = module_name.to_owned();
                async move { this.resolve_oid(name).await }
            })
            .await
            .map_err(|e: Arc<RuntimeError>| RuntimeError::Load(e.to_string()))?
            .into_value();

        info!("Function fetch_wasm key {:?}", &key);

        self.module_cache
            .entry_by_ref(&key)
            .or_try_insert_with({
                let this = self.clone();
                async move { this.compile(oid).await }
            })
            .await
            .map_err(|e: Arc<RuntimeError>| RuntimeError::Compile(e.to_string()))?;

        Ok(oid)
    }

    async fn resolve_oid(self: &Arc<Self>, name: String) -> Result<ObjectId, RuntimeError> {
        let this = self.clone();
        spawn_blocking(move || this.loader.resolve(&name))
            .await
            .map_err(|e| RuntimeError::Internal(e.to_string()))?
    }

    async fn compile(
        self: &Arc<Self>,
        oid: ObjectId,
    ) -> Result<InstancePre<StoreCtx>, RuntimeError> {
        let this = self.clone();
        let bytes = spawn_blocking({
            let this = this.clone();
            move || this.loader.read_blob(oid)
        })
        .await
        .map_err(|e| RuntimeError::Internal(e.to_string()))??;

        spawn_blocking(move || {
            let module = Module::from_binary(&this.engine, &bytes)
                .map_err(|e| RuntimeError::Compile(e.to_string()))?;
            this.linker
                .instantiate_pre(&module)
                .map_err(|e| RuntimeError::Compile(e.to_string()))
        })
        .await
        .map_err(|e| RuntimeError::Internal(e.to_string()))?
    }
}

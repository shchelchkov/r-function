use crate::plugin::PluginContext;
use crate::plugin::loader::PluginLoader;
use crate::plugin::plugin::Plugin;
use arc_swap::ArcSwapOption;
use gix::ObjectId;
use moka::future::Cache;
use r_error::runtime::error::RuntimeError;
use std::sync::Arc;
use tokio::task::spawn_blocking;
use tracing::{info, warn};

pub struct PluginRepository {
    resolve_cache: Cache<Arc<str>, ObjectId>,
    plugin_cache: Cache<Arc<str>, Arc<Plugin>>,
    loader: Arc<dyn PluginLoader>,
    ctx: Arc<PluginContext>,
    revision: ArcSwapOption<ObjectId>,
}

impl PluginRepository {
    pub fn new(
        loader: Arc<dyn PluginLoader>,
        ctx: Arc<PluginContext>,
    ) -> Self {
        Self {
            resolve_cache: Cache::new(1024),
            plugin_cache: Cache::new(1024),
            loader,
            ctx: ctx,
            revision: ArcSwapOption::empty(),
        }
    }

    pub fn loader(
        &self,
    ) -> Arc<dyn PluginLoader> {
        Arc::clone(&self.loader)
    }

    pub fn context(
        &self,
    ) -> Arc<PluginContext> {
        Arc::clone(&self.ctx)
    }

    pub async fn get(
        self: &Arc<Self>,
        module_name: &str,
    ) -> Result<Arc<Plugin>, RuntimeError> {
        if let Some(plugin) = self.plugin_cache.get(module_name).await {
            return Ok(plugin);
        }
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
        match self.plugin_cache.get(module_name).await {
            Some(plug) => Ok(plug),
            None => self
                .plugin_cache
                .try_get_with(Arc::<str>::from(module_name), {
                    let this = self.clone();
                    let path = this.loader.path_module(module_name);
                    async move { this.compile(path).await }
                })
                .await
                .map_err(|e: Arc<RuntimeError>| RuntimeError::Compile(e.to_string())),
        }
    }


    pub fn on_head_changed(
        self: &Arc<Self>,
        new_head: ObjectId,
        changed: &[String],
    ) {
        let plugin_path = match self.loader.git_plugin_path() {
            Ok(path) => path,
            Err(error) => {
                warn!(%error,"failed to resolve plugin path");
                return;
            }
        };

        let prefix = format!("{plugin_path}/");

        let changed: Vec<String> = changed
            .iter()
            .filter_map(|path| {
                Some(path.to_owned())
                // let path = path.strip_prefix(&prefix)?;
                //
                // if path.ends_with(".so") {
                //     Some(path.to_owned())
                // } else {
                //     None
                // }
            })
            .collect();

        if changed.is_empty() {
            return;
        }

        self.revision.store(Some(Arc::new(new_head)));

        let this = Arc::clone(self);

        tokio::spawn(async move {
            for plugin_path in changed {
                if let Err(error) = this.refresh(&plugin_path).await {
                    warn!(module = %plugin_path,%error,"plugin refresh failed");
                } else {
                    info!(
                        module = %plugin_path,
                        "plugin refreshed"
                    );
                }
            }
        });
    }

    async fn refresh(self: &Arc<Self>, plugin_path: &str) -> Result<ObjectId, RuntimeError> {
        let key: Arc<str> = Arc::from(plugin_path);
        self.resolve_cache.invalidate(&key).await;
        self.plugin_cache.invalidate(&key).await;

        let oid = {
            let this = Arc::clone(self);
            let name = plugin_path.to_owned();

            self.resolve_cache
                .try_get_with(key.clone(), async move { this.resolve_oid(name).await }).await
                .map_err(|e| {
                    RuntimeError::Load(
                        e.to_string(),
                    )
                })?
        };

        {
            let this = Arc::clone(self);
            let path = this.loader.path_module(plugin_path);
            self.plugin_cache
                .try_get_with(key, async move { this.compile(path).await }).await
                .map_err(|e| {
                    RuntimeError::Compile(
                        e.to_string(),
                    )
                })?;
        }

        Ok(oid)
    }

    async fn resolve_oid(
        self: &Arc<Self>,
        name: String,
    ) -> Result<ObjectId, RuntimeError> {
        let loader = Arc::clone(&self.loader);
        spawn_blocking(move || { loader.resolve(&name) }).await
            .map_err(|error| {
                RuntimeError::Internal(
                    error.to_string(),
                )
            })?
    }

    async fn compile(
        self: &Arc<Self>,
        plugin_path: String,
    ) -> Result<Arc<Plugin>, RuntimeError> {
        spawn_blocking(move || {
            let plugin = unsafe {
                Plugin::load(&plugin_path)
                    .map_err(|error| {
                        RuntimeError::Compile(error.to_string())
                    })?
            };
            Ok(Arc::new(plugin))
        })
            .await
            .map_err(|error| {
                RuntimeError::Internal(error.to_string())
            })?

    }

    pub fn revision(
        &self,
    ) -> Option<Arc<ObjectId>> {
        self.revision.load_full()
    }

    pub fn entries_plugin_cache(&self) -> Vec<String> {
        self.plugin_cache
            .iter()
            .map(|(key, _plugin)| key.to_string())
            .collect()
    }

    pub fn entries_resolve_cache(&self) -> Vec<String> {
        self.resolve_cache
            .iter()
            .map(|(key, _plugin)| key.to_string())
            .collect()
    }
}
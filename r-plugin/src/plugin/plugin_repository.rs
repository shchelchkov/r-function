use crate::plugin::PluginContext;
use crate::plugin::loader::PluginLoader;
use crate::plugin::plugin::Plugin;
use arc_swap::ArcSwapOption;
use gix::ObjectId;
use moka::future::Cache;
use r_error::plugin::error::PluginError;
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
    pub fn new(loader: Arc<dyn PluginLoader>, ctx: Arc<PluginContext>) -> Self {
        Self {
            resolve_cache: Cache::new(1024),
            plugin_cache: Cache::new(1024),
            loader,
            ctx,
            revision: ArcSwapOption::empty(),
        }
    }

    pub fn loader(&self) -> Arc<dyn PluginLoader> {
        Arc::clone(&self.loader)
    }

    pub fn context(&self) -> Arc<PluginContext> {
        Arc::clone(&self.ctx)
    }

    pub async fn get(self: &Arc<Self>, module_name: &str) -> Result<Arc<Plugin>, PluginError> {
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
                .map_err(|e: Arc<PluginError>| PluginError::Load(e.to_string()))?,
        };
        match self.plugin_cache.get(module_name).await {
            Some(plug) => Ok(plug),
            None => self
                .plugin_cache
                .try_get_with(Arc::<str>::from(module_name), {
                    let this = self.clone();
                    async move { this.compile(oid).await }
                })
                .await
                .map_err(|e: Arc<PluginError>| PluginError::Compile(e.to_string())),
        }
    }

    pub fn on_head_changed(self: &Arc<Self>, new_head: ObjectId, changed: &[String]) {
        let plugin_path = match self.loader.git_plugin_path() {
            Ok(path) => path,
            Err(error) => {
                warn!(%error,"failed to resolve plugin path");
                return;
            }
        };

        let changed = changed_modules(&plugin_path, changed);

        if changed.is_empty() {
            return;
        }

        self.revision.store(Some(Arc::new(new_head)));

        let this = Arc::clone(self);

        tokio::spawn(async move {
            for module_name in changed {
                match this.refresh(&module_name).await {
                    Ok(oid) => info!(module = %module_name, %oid, "plugin refreshed"),
                    Err(error) => warn!(module = %module_name, %error, "plugin refresh failed"),
                }
            }
        });
    }

    async fn refresh(self: &Arc<Self>, module_name: &str) -> Result<ObjectId, PluginError> {
        let key: Arc<str> = Arc::from(module_name);
        self.resolve_cache.invalidate(&key).await;
        self.plugin_cache.invalidate(&key).await;

        let oid = {
            let this = Arc::clone(self);
            let name = module_name.to_owned();

            self.resolve_cache
                .try_get_with(key.clone(), async move { this.resolve_oid(name).await })
                .await
                .map_err(|e| PluginError::Load(e.to_string()))?
        };

        {
            let this = Arc::clone(self);
            self.plugin_cache
                .try_get_with(key, async move { this.compile(oid).await })
                .await
                .map_err(|e| PluginError::Compile(e.to_string()))?;
        }

        Ok(oid)
    }

    async fn resolve_oid(self: &Arc<Self>, name: String) -> Result<ObjectId, PluginError> {
        let loader = Arc::clone(&self.loader);
        spawn_blocking(move || loader.resolve(&name))
            .await
            .map_err(|error| PluginError::Internal(error.to_string()))?
    }

    async fn compile(self: &Arc<Self>, oid: ObjectId) -> Result<Arc<Plugin>, PluginError> {
        let loader = Arc::clone(&self.loader);
        spawn_blocking(move || {
            let path = loader.materialize(oid)?;
            let plugin = unsafe { Plugin::load(&path) }
                .map_err(|error| PluginError::Compile(error.to_string()))?;
            Ok(Arc::new(plugin))
        })
        .await
        .map_err(|error| PluginError::Internal(error.to_string()))?
    }

    pub fn revision(&self) -> Option<Arc<ObjectId>> {
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

fn changed_modules(plugin_path: &str, changed: &[String]) -> Vec<String> {
    let prefix = format!("{}/", plugin_path.trim_end_matches('/'));
    changed
        .iter()
        .filter_map(|path| path.strip_prefix(&prefix))
        .filter(|rest| rest.ends_with(".so"))
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::changed_modules;

    fn paths(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn keeps_only_so_under_plugin_path() {
        let changed = paths(&[
            "plugin/function_settings/libf_test_key.so",
            "plugin/function_settings/README.md",
            "json/message/function_settings/x.json",
            "wasm/function_settings/f.wasm",
            "plugin/other/lib.so",
        ]);

        assert_eq!(
            changed_modules("plugin/function_settings", &changed),
            ["libf_test_key.so"]
        );
    }

    #[test]
    fn nested_module_keeps_relative_path() {
        let changed = paths(&["plugin/function_settings/sub/lib.so"]);
        assert_eq!(
            changed_modules("plugin/function_settings", &changed),
            ["sub/lib.so"]
        );
    }

    #[test]
    fn trailing_slash_in_plugin_path_is_tolerated() {
        let changed = paths(&["plugin/function_settings/lib.so"]);
        assert_eq!(
            changed_modules("plugin/function_settings/", &changed),
            ["lib.so"]
        );
    }

    #[test]
    fn prefix_must_match_a_whole_component() {
        let changed = paths(&["plugin/function_settings_v2/lib.so"]);
        assert!(changed_modules("plugin/function_settings", &changed).is_empty());
    }
}

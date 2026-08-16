use gix::ObjectId;
use r_config::config::FunctionConfig;
use r_error::runtime::error::RuntimeError;
use r_setting::git::GitHandle;
use std::sync::Arc;

pub trait PluginLoader: Send + Sync {
    fn git_plugin_path(&self) -> Result<String, RuntimeError>;
    fn resolve(&self, module_name: &str) -> Result<gix::ObjectId, RuntimeError>;
    fn path(&self, module_name: &str, oid: ObjectId) -> String;
    fn path_module(&self, module_name: &str) -> String;
    fn read_blob(&self, oid: gix::ObjectId) -> Result<Vec<u8>, RuntimeError>;
}

#[derive(Clone)]
pub struct GitPluginLoader {
    shared: Arc<Shared>,
}

struct Shared {
    git: Arc<GitHandle>,
    git_plugin_path: Arc<str>,
    git_workdir: Arc<str>,
}

impl GitPluginLoader {
    pub fn new(git: Arc<GitHandle>, cfg: &FunctionConfig) -> Self {
        let shared = Arc::new(Shared {
            git,
            git_plugin_path: cfg.git_plugin_path.clone().into(),
            git_workdir: cfg.git_workdir.clone().into(),
        });
        Self { shared }
    }

}

impl PluginLoader for GitPluginLoader {
    fn git_plugin_path(&self) -> Result<String, RuntimeError> {
        Ok(self.shared.git_plugin_path.to_string())
    }

    fn path(&self, module_name: &str, oid: ObjectId) -> String {
        let spec = format!("{}:{}/{}", oid, self.shared.git_plugin_path, module_name);
        spec
    }

    fn path_module(&self, module_name: &str) -> String {
        let spec = format!("{}/{}/{}", self.shared.git_workdir, self.shared.git_plugin_path, module_name);
        spec
    }

    fn resolve(&self, module_name: &str) -> Result<gix::ObjectId, RuntimeError> {
        let oid = **self.shared.git.head.load();
        let spec = self.path(module_name, oid);
        let repo = self.shared.git.repo().to_thread_local();
        let id = repo
            .rev_parse_single(spec.as_str())
            .map_err(|e| RuntimeError::Load(e.to_string()))?;
        Ok(id.into())
    }

    fn read_blob(&self, oid: gix::ObjectId) -> Result<Vec<u8>, RuntimeError> {
        let repo = self.shared.git.repo().to_thread_local();
        let blob = repo
            .find_object(oid)
            .map_err(|e| RuntimeError::Load(e.to_string()))?;

        Ok(blob.data.clone())
    }

}

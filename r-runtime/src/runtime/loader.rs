use r_config::config::FunctionConfig;
use r_error::runtime::error::RuntimeError;
use r_setting::git::GitHandle;
use std::sync::Arc;

pub trait ModuleLoader: Send + Sync {
    fn git_wasm_path(&self) -> Result<String, RuntimeError>;
    fn resolve(&self, module_name: &str) -> Result<gix::ObjectId, RuntimeError>;
    fn read_blob(&self, oid: gix::ObjectId) -> Result<Vec<u8>, RuntimeError>;
}

#[derive(Clone)]
pub struct GitModuleLoader {
    shared: Arc<Shared>,
}

struct Shared {
    git: Arc<GitHandle>,
    git_wasm_path: Arc<str>,
}

impl GitModuleLoader {
    pub fn new(git: Arc<GitHandle>, cfg: &FunctionConfig) -> Self {
        let shared = Arc::new(Shared {
            git,
            git_wasm_path: cfg.git_wasm_path.clone().into(),
        });
        Self { shared }
    }
}

impl ModuleLoader for GitModuleLoader {
    fn git_wasm_path(&self) -> Result<String, RuntimeError> {
        Ok(self.shared.git_wasm_path.to_string())
    }

    fn resolve(&self, module_name: &str) -> Result<gix::ObjectId, RuntimeError> {
        let oid = **self.shared.git.head.load();
        let spec = format!("{}:{}/{}", oid, self.shared.git_wasm_path, module_name);
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

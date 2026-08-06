use gix::ObjectId;
use r_config::config::FunctionConfig;
use r_error::plugin::error::PluginError;
use r_setting::git::GitHandle;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub trait PluginLoader: Send + Sync {
    fn git_plugin_path(&self) -> Result<String, PluginError>;

    fn resolve(&self, module_name: &str) -> Result<ObjectId, PluginError>;
    fn materialize(&self, oid: ObjectId) -> Result<PathBuf, PluginError>;
}

#[derive(Clone)]
pub struct GitPluginLoader {
    shared: Arc<Shared>,
}

struct Shared {
    git: Arc<GitHandle>,
    git_plugin_path: Arc<str>,
    cache_dir: PathBuf,
}

impl GitPluginLoader {
    pub fn new(git: Arc<GitHandle>, cfg: &FunctionConfig) -> Self {
        let cache_dir = PathBuf::from(format!("{}.plugins", cfg.git_workdir.trim_end_matches('/')));
        let shared = Arc::new(Shared {
            git,
            git_plugin_path: cfg.git_plugin_path.clone().into(),
            cache_dir,
        });
        Self { shared }
    }

    fn spec(&self, module_name: &str, oid: ObjectId) -> String {
        format!("{}:{}/{}", oid, self.shared.git_plugin_path, module_name)
    }

    fn read_blob(&self, oid: ObjectId) -> Result<Vec<u8>, PluginError> {
        let repo = self.shared.git.repo().to_thread_local();
        let object = repo
            .find_object(oid)
            .map_err(|e| PluginError::Load(e.to_string()))?;

        if object.kind != gix::object::Kind::Blob {
            return Err(PluginError::Load(format!(
                "{oid} is a {}, expected a blob",
                object.kind
            )));
        }

        Ok(object.detach().data)
    }
}

impl PluginLoader for GitPluginLoader {
    fn git_plugin_path(&self) -> Result<String, PluginError> {
        Ok(self.shared.git_plugin_path.to_string())
    }

    fn resolve(&self, module_name: &str) -> Result<ObjectId, PluginError> {
        let oid = **self.shared.git.head.load();
        let spec = self.spec(module_name, oid);
        let repo = self.shared.git.repo().to_thread_local();
        let id = repo
            .rev_parse_single(spec.as_str())
            .map_err(|e| PluginError::Load(e.to_string()))?;
        Ok(id.into())
    }

    fn materialize(&self, oid: ObjectId) -> Result<PathBuf, PluginError> {
        materialize_blob(&self.shared.cache_dir, oid, || self.read_blob(oid))
    }
}

fn materialize_blob(
    dir: &Path,
    oid: ObjectId,
    read: impl FnOnce() -> Result<Vec<u8>, PluginError>,
) -> Result<PathBuf, PluginError> {
    let target = dir.join(format!("{oid}.so"));
    if target.is_file() {
        return Ok(target);
    }

    let io = |e: std::io::Error| PluginError::Load(format!("materialize {oid}: {e}"));

    std::fs::create_dir_all(dir).map_err(io)?;
    let bytes = read()?;

    let mut tmp = tempfile::Builder::new()
        .prefix(&format!(".{oid}."))
        .suffix(".so.tmp")
        .tempfile_in(dir)
        .map_err(io)?;
    tmp.write_all(&bytes).map_err(io)?;
    tmp.as_file().sync_all().map_err(io)?;
    tmp.persist(&target)
        .map_err(|e| PluginError::Load(format!("materialize {oid}: {}", e.error)))?;

    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(byte: u8) -> ObjectId {
        ObjectId::from_bytes_or_panic(&[byte; 20])
    }

    #[test]
    fn writes_blob_under_oid_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = materialize_blob(dir.path(), oid(0xab), || Ok(b"elf".to_vec())).unwrap();

        assert_eq!(path, dir.path().join(format!("{}.so", oid(0xab))));
        assert_eq!(std::fs::read(&path).unwrap(), b"elf");
    }

    #[test]
    fn creates_missing_cache_dir() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a").join("b");
        let path = materialize_blob(&nested, oid(1), || Ok(vec![1])).unwrap();

        assert!(path.starts_with(&nested));
        assert!(path.is_file());
    }

    #[test]
    fn existing_file_short_circuits_read() {
        let dir = tempfile::tempdir().unwrap();
        materialize_blob(dir.path(), oid(2), || Ok(b"v1".to_vec())).unwrap();

        let path = materialize_blob(dir.path(), oid(2), || {
            panic!("blob must not be re-read for an existing oid")
        })
        .unwrap();

        assert_eq!(std::fs::read(path).unwrap(), b"v1");
    }

    #[test]
    fn read_failure_leaves_no_files_behind() {
        let dir = tempfile::tempdir().unwrap();
        let err = materialize_blob(dir.path(), oid(3), || {
            Err(PluginError::Load("git down".into()))
        })
        .unwrap_err();

        assert!(matches!(err, PluginError::Load(_)));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn materialized_real_plugin_loads() {
        let lib = std::env::var("R_PLUGIN_SMOKE_LIB").expect("R_PLUGIN_SMOKE_LIB is not set");
        let bytes = std::fs::read(&lib).expect("read plugin cdylib");

        let dir = tempfile::tempdir().unwrap();
        let path = materialize_blob(dir.path(), oid(0x5e), || Ok(bytes)).unwrap();

        let plugin = unsafe { crate::plugin::plugin::Plugin::load(&path) }
            .unwrap_or_else(|e| panic!("dlopen {}: {e}", path.display()));
        drop(plugin);
    }

    #[test]
    fn no_tmp_files_remain_after_success() {
        let dir = tempfile::tempdir().unwrap();
        materialize_blob(dir.path(), oid(4), || Ok(vec![0; 4096])).unwrap();

        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, [format!("{}.so", oid(4))]);
    }
}

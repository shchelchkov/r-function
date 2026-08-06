use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use gix::ObjectId;
use gix::Repository;
use r_config::config::FunctionConfig;
use r_error::git::error::GitError;
use tracing::{info, warn};

use crate::git::handle::GitHandle;
use crate::git::remote::fetch_default_remote;

pub fn git_sync(cfg: &FunctionConfig) -> Result<Arc<GitHandle>, GitError> {
    let workdir = PathBuf::from(&cfg.git_workdir);

    info!(repo = %cfg.git_repo_url, workdir = %workdir.display(), revision = %cfg.git_revision, "git sync");

    let repo = if workdir.join(".git").exists() {
        match open_and_fetch(&workdir) {
            Ok(repo) => repo,
            Err(e) => {
                warn!(error = %e, workdir = %workdir.display(),
                      "open/fetch failed; wiping workdir and re-cloning");
                wipe_and_reclone(cfg.git_repo_url.as_str(), &workdir)?
            }
        }
    } else {
        clone(cfg.git_repo_url.as_str(), &workdir)?
    };

    let head: ObjectId = repo
        .rev_parse_single(cfg.git_revision.as_str())
        .map_err(|e| GitError::RevParse {
            spec: cfg.git_revision.clone(),
            cause: e.to_string(),
        })?
        .detach();

    Ok(Arc::new(GitHandle::new(
        repo.into_sync(),
        head,
        cfg.git_revision.clone().into(),
        cfg.git_repo_url.clone().into(),
        Arc::from(workdir),
    )))
}

fn open_and_fetch(workdir: &Path) -> Result<Repository, GitError> {
    let repo = gix::open_opts(
        workdir,
        gix::open::Options::default().config_overrides(["ssh.variant=ssh"]),
    )
    .map_err(|e| GitError::Open(e.to_string()))?;
    fetch_default_remote(&repo)?;
    Ok(repo)
}

pub(crate) fn wipe_and_reclone(url: &str, workdir: &Path) -> Result<Repository, GitError> {
    match std::fs::remove_dir_all(workdir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(GitError::Clone(format!("remove workdir: {e}"))),
    }
    clone(url, workdir)
}

fn clone(url: &str, workdir: &Path) -> Result<Repository, GitError> {
    let mut prepare = gix::prepare_clone(url, workdir)
        .map_err(|e| GitError::Clone(e.to_string()))?
        .with_in_memory_config_overrides(["ssh.variant=ssh"]);
    let (mut prepare_checkout, _) = prepare
        .fetch_then_checkout(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
        .map_err(|e| GitError::Clone(e.to_string()))?;
    let (repo, _) = prepare_checkout
        .main_worktree(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
        .map_err(|e| GitError::Checkout(e.to_string()))?;
    Ok(repo)
}

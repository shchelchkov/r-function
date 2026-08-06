use std::path::Path;
use std::sync::Arc;

use arc_swap::ArcSwap;
use gix::ObjectId;
use gix::Repository;
use gix::ThreadSafeRepository;
use r_error::git::error::GitError;
use tokio::sync::Mutex;
use tokio::task::spawn_blocking;
use tracing::{info, warn};

use crate::git::remote::{diff_paths, fetch_default_remote};
use crate::git::sync::wipe_and_reclone;

pub struct GitHandle {
    repo: ArcSwap<ThreadSafeRepository>,
    pub head: Arc<ArcSwap<ObjectId>>,
    pub revision: Arc<str>,
    url: Arc<str>,
    workdir: Arc<Path>,
    fetch_lock: Mutex<()>,
}

pub struct RefreshOutcome {
    pub previous: ObjectId,
    pub current: ObjectId,
    pub changed: bool,
    pub changed_paths: Vec<String>,
}

impl GitHandle {
    pub(crate) fn new(
        repo: ThreadSafeRepository,
        head: ObjectId,
        revision: Arc<str>,
        url: Arc<str>,
        workdir: Arc<Path>,
    ) -> Self {
        Self {
            repo: ArcSwap::from_pointee(repo),
            head: Arc::new(ArcSwap::from_pointee(head)),
            revision,
            url,
            workdir,
            fetch_lock: Mutex::new(()),
        }
    }

    pub fn repo(&self) -> Arc<ThreadSafeRepository> {
        self.repo.load_full()
    }

    pub async fn refresh(&self) -> Result<RefreshOutcome, GitError> {
        let _guard = self.fetch_lock.lock().await;

        let repo = self.repo.load_full();
        let rev = self.revision.clone();
        let url = self.url.clone();
        let workdir = self.workdir.clone();
        let prev = **self.head.load();

        let (fresh_repo, new_oid, changed_paths) = spawn_blocking(
            move || -> Result<(Option<ThreadSafeRepository>, ObjectId, Vec<String>), GitError> {
                let local = repo.to_thread_local();
                match fetch_default_remote(&local) {
                    Ok(()) => {
                        let id = resolve_revision(&local, &rev)?;
                        Ok((None, id, diff_if_advanced(&local, prev, id)))
                    }
                    Err(e) => {
                        warn!(error = %e, workdir = %workdir.display(),
                              "::::::::::::: ::::::::::::: :::::::::::::  git fetch failed; wiping workdir and re-cloning");
                        let fresh = wipe_and_reclone(&url, &workdir)?;
                        let id = resolve_revision(&fresh, &rev)?;
                        let paths = diff_if_advanced(&fresh, prev, id);
                        Ok((Some(fresh.into_sync()), id, paths))
                    }
                }
            },
        )
        .await
        .map_err(|e| GitError::Task(e.to_string()))??;

        if let Some(fresh) = fresh_repo {
            self.repo.store(Arc::new(fresh));
        }

        let changed = prev != new_oid;
        if changed {
            self.head.store(Arc::new(new_oid));
            info!(
                old = %prev,
                new = %new_oid,
                paths = changed_paths.len(),
                "git head advanced"
            );
        }

        Ok(RefreshOutcome {
            previous: prev,
            current: new_oid,
            changed,
            changed_paths,
        })
    }
}

fn resolve_revision(repo: &Repository, rev: &str) -> Result<ObjectId, GitError> {
    Ok(repo
        .rev_parse_single(rev)
        .map_err(|e| GitError::RevParse {
            spec: rev.to_string(),
            cause: e.to_string(),
        })?
        .detach())
}

fn diff_if_advanced(repo: &Repository, prev: ObjectId, current: ObjectId) -> Vec<String> {
    if current == prev {
        return Vec::new();
    }
    diff_paths(repo, prev, current).unwrap_or_else(|e| {
        warn!(error = %e, "diff_paths failed; observers will receive empty list");
        Vec::new()
    })
}

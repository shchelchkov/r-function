use std::convert::Infallible;
use std::ops::ControlFlow;

use gix::ObjectId;
use gix::Repository;
use gix::object::tree::diff::Change;
use gix::remote::Direction;
use r_error::git::error::GitError;
use tracing::info;

pub(crate) fn fetch_default_remote(repo: &Repository) -> Result<(), GitError> {
    let remote = repo
        .find_fetch_remote(None)
        .map_err(|e| GitError::Fetch(e.to_string()))?;
    let connection = remote
        .connect(Direction::Fetch)
        .map_err(|e| GitError::Fetch(e.to_string()))?;

    let prepare = match connection.prepare_fetch(gix::progress::Discard, Default::default()) {
        Ok(prepare) => {
            info!("prepare_fetch: ok ");
            prepare
        }
        Err(e) => return Err(GitError::Fetch(e.to_string())),
    };

    let _outcome = match prepare.receive(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED) {
        Ok(outcome) => {
            info!("receive: ok ");
            outcome
        }
        Err(e) => return Err(GitError::Fetch(e.to_string())),
    };
    Ok(())
}

pub(crate) fn diff_paths(
    repo: &Repository,
    prev: ObjectId,
    current: ObjectId,
) -> Result<Vec<String>, GitError> {
    let prev_tree = repo
        .find_commit(prev)
        .map_err(|e| GitError::Fetch(format!("find_commit prev: {e}")))?
        .tree()
        .map_err(|e| GitError::Fetch(format!("tree prev: {e}")))?;
    let new_tree = repo
        .find_commit(current)
        .map_err(|e| GitError::Fetch(format!("find_commit current: {e}")))?
        .tree()
        .map_err(|e| GitError::Fetch(format!("tree current: {e}")))?;

    let mut paths: Vec<String> = Vec::new();
    let mut platform = prev_tree
        .changes()
        .map_err(|e| GitError::Fetch(format!("tree changes: {e}")))?;
    platform
        .for_each_to_obtain_tree(&new_tree, |change| {
            match change {
                Change::Addition { location, .. }
                | Change::Deletion { location, .. }
                | Change::Modification { location, .. } => {
                    info!("diff_paths change: {:?}", location);
                    paths.push(location.to_string());
                }
                Change::Rewrite {
                    source_location,
                    location,
                    ..
                } => {
                    info!(
                        "diff_paths rewrite: {:?} -> {:?}",
                        source_location, location
                    );
                    paths.push(source_location.to_string());
                    paths.push(location.to_string());
                }
            }
            Ok::<_, Infallible>(ControlFlow::Continue(()))
        })
        .map_err(|e| GitError::Fetch(format!("diff: {e}")))?;
    Ok(paths)
}

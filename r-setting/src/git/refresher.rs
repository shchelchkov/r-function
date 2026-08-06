use std::sync::Arc;
use std::time::Duration;

use gix::ObjectId;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;
use tracing::warn;

use crate::git::handle::GitHandle;

pub trait HeadObserver: Send + Sync {
    fn on_revision_changed(&self, new_head: ObjectId, changed_paths: &[String]);
}


pub fn spawn_git_refresher(
    handle: Arc<GitHandle>,
    interval: Duration,
    observers: Vec<Arc<dyn HeadObserver>>,
    mut shutdown: watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        ticker.tick().await;

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    match handle.refresh().await {
                        Ok(out) if out.changed => {
                            for obs in &observers {
                                obs.on_revision_changed(out.current, &out.changed_paths);
                            }
                        }
                        Ok(_) => {}
                        Err(e) => warn!(error = %e, "git fetch failed; keeping previous head"),
                    }
                }
                _ = shutdown.changed() => break,
            }
        }
    })
}

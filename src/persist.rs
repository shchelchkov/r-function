use std::time::Duration;

use r_db::db::db::{Database, PersistMode};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::error;

pub fn spawn_flusher(
    db: Database,
    every: Duration,
    mut shutdown: watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(every);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        tick.tick().await;

        loop {
            tokio::select! {
                _ = tick.tick() => persist(&db, PersistMode::SyncData).await,
                _ = shutdown.changed() => break,
            }
        }
    })
}

pub async fn persist_all(db: &Database) {
    persist(db, PersistMode::SyncAll).await
}

async fn persist(db: &Database, mode: PersistMode) {
    let db = db.clone();
    match tokio::task::spawn_blocking(move || db.persist(mode)).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => error!(error = %e, ?mode, "db persist failed"),
        Err(e) => error!(error = %e, ?mode, "db persist task failed"),
    }
}

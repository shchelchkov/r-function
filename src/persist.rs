use std::sync::Arc;
use std::time::{Duration, Instant};

use r_db::db::db::{Database, PersistMode};
use r_value::value::value::{Restore, Values};
use sonic_rs::{JsonContainerTrait, Value};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{error, info};

use r_state_service::state_service::StateService;

pub const SNAPSHOT_CODE: &str = "values_snapshot";

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

pub fn spawn_snapshotter(
    values: Values,
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
                _ = tick.tick() => snapshot_values(&values, &db).await,
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

pub async fn snapshot_values(values: &Values, db: &Database) {
    let (values, db) = (values.clone(), db.clone());
    let started = Instant::now();
    match tokio::task::spawn_blocking(move || write_snapshot(&values, &db)).await {
        Ok(Ok((keys, removed))) => {
            info!(keys, removed, elapsed = ?started.elapsed(), "values snapshot saved")
        }
        Ok(Err(e)) => error!(error = %e, "values snapshot failed"),
        Err(e) => error!(error = %e, "values snapshot task failed"),
    }
}

type SnapshotResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub fn write_snapshot(values: &Values, db: &Database) -> SnapshotResult<(usize, usize)> {
    let entries = values.entries()?;
    for (key, list) in &entries {
        db.put_value(SNAPSHOT_CODE, key, Value::from(list.as_ref().clone()))?;
    }
    let mut removed = 0;
    for key in values.absent_keys()? {
        if db.remove_value(SNAPSHOT_CODE, &key)? {
            removed += 1;
        }
    }
    Ok((entries.len(), removed))
}

pub fn restore_from(db: Database, service: Option<StateService>) -> Restore {
    Arc::new(
        move |key: &str, at: Option<i64>| match db.get_value(SNAPSHOT_CODE, key) {
            Ok(Some(list)) => list
                .first()
                .and_then(|v| v.as_array())
                .map(|a| Arc::new(a.iter().cloned().collect())),
            Ok(None) => service.as_ref()?.restore(&db, key, at),
            Err(e) => {
                error!(error = %e, key, "values snapshot restore failed");
                None
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use r_db::db::db::DatabaseOptions;

    fn open() -> (tempfile::TempDir, Database) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(
            dir.path(),
            DatabaseOptions {
                limit: 3,
                history: 3,
            },
        )
        .unwrap();
        (dir, db)
    }

    fn val(s: &str) -> Value {
        sonic_rs::from_str(s).unwrap()
    }

    fn restored(db: &Database, key: &str) -> Option<String> {
        restore_from(db.clone(), None)(key, None).map(|list| sonic_rs::to_string(&*list).unwrap())
    }

    #[test]
    fn snapshot_restores_values_after_restart() {
        let (_dir, db) = open();
        let before = Values::new().unwrap();
        before
            .put_value(
                "order_book_depth",
                Arc::from("orderbook.200.linear.bybit.BTCUSDT"),
                val(r#"{"u":7}"#),
            )
            .unwrap();
        before
            .put_value(
                "obi_state",
                Arc::from("orderbook.200.linear.bybit.BTCUSDT.depth"),
                val(r#"{"z":1.5}"#),
            )
            .unwrap();
        assert_eq!(write_snapshot(&before, &db).unwrap(), (2, 0));

        let after = Values::with_restore(restore_from(db.clone(), None)).unwrap();
        let book = after
            .get_value("order_book_depth", "orderbook.200.linear.bybit.BTCUSDT")
            .unwrap()
            .expect("из снимка");
        assert_eq!(sonic_rs::to_string(&*book).unwrap(), r#"[{"u":7}]"#);
        let obi = after
            .get_value("obi_state", "orderbook.200.linear.bybit.BTCUSDT.depth")
            .unwrap()
            .expect("из снимка");
        assert_eq!(sonic_rs::to_string(&*obi).unwrap(), r#"[{"z":1.5}]"#);
        assert!(after.get_value("obi_state", "other").unwrap().is_none());
    }

    #[test]
    fn repeated_snapshots_keep_one_record_per_key() {
        let (_dir, db) = open();
        let values = Values::new().unwrap();
        for i in 0..5 {
            values
                .put_value("c", Arc::from("a"), val(&i.to_string()))
                .unwrap();
            write_snapshot(&values, &db).unwrap();
        }
        let list = db
            .get_value(SNAPSHOT_CODE, "c.a")
            .unwrap()
            .expect("в снимке");
        assert_eq!(
            list.len(),
            1,
            "снимок заменяет запись ключа, а не копит их до limit"
        );
        assert_eq!(restored(&db, "c.a").as_deref(), Some("[4]"));
    }

    #[test]
    fn snapshot_keeps_keys_untouched_in_this_run() {
        let (_dir, db) = open();
        let first = Values::new().unwrap();
        first.put_value("c", Arc::from("a"), val("1")).unwrap();
        first.put_value("c", Arc::from("b"), val("2")).unwrap();
        write_snapshot(&first, &db).unwrap();

        let second = Values::with_restore(restore_from(db.clone(), None)).unwrap();
        second.put_value("c", Arc::from("a"), val("3")).unwrap();
        assert_eq!(write_snapshot(&second, &db).unwrap(), (1, 0));

        assert_eq!(restored(&db, "c.a").as_deref(), Some("[3]"));
        assert_eq!(
            restored(&db, "c.b").as_deref(),
            Some("[2]"),
            "b — из прошлого снимка"
        );
    }

    #[test]
    fn snapshot_drops_keys_removed_in_this_run() {
        let (_dir, db) = open();
        let first = Values::new().unwrap();
        first.put_value("c", Arc::from("a"), val("1")).unwrap();
        first.put_value("c", Arc::from("b"), val("2")).unwrap();
        write_snapshot(&first, &db).unwrap();

        let second = Values::with_restore(restore_from(db.clone(), None)).unwrap();
        second.get_value("c", "a").unwrap().expect("из снимка");
        assert!(second.remove_value("c", "a").unwrap());
        assert!(
            !second.remove_value("c", "b").unwrap(),
            "b в память не поднимали"
        );
        assert_eq!(write_snapshot(&second, &db).unwrap(), (0, 2));
        assert!(
            db.entries_by_setting_code(SNAPSHOT_CODE)
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn snapshotter_writes_by_timer_and_stops_on_shutdown() {
        let (_dir, db) = open();
        let values = Values::new().unwrap();
        values.put_value("c", Arc::from("a"), val("1")).unwrap();
        let (stop, shutdown) = watch::channel(false);
        let every = Duration::from_millis(10);
        let task = spawn_snapshotter(values, db.clone(), every, shutdown);

        let deadline = Instant::now() + Duration::from_secs(5);
        while restored(&db, "c.a").is_none() {
            assert!(Instant::now() < deadline, "снимка по таймеру нет");
            tokio::time::sleep(every).await;
        }
        stop.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("выход по shutdown")
            .unwrap();
    }
}

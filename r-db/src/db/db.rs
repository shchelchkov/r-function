use crate::db::error::DatabaseError;
use crate::db::ring_buffer::RingBuffer;
use dashmap::DashMap;
use fjall::{Keyspace, KeyspaceCreateOptions, PersistMode};
use parking_lot::Mutex;
use sonic_rs::Value;
use std::fmt::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub type Result<T> = std::result::Result<T, DatabaseError>;

pub type DataEntry = (Arc<str>, Arc<Vec<Value>>);

#[derive(Clone)]
pub struct Database {
    shared: Arc<Shared>,
}

// #[derive(Debug)]
struct Shared {
    recent: DashMap<Arc<str>, Arc<Mutex<RingBuffer<Value>>>>,

    locks: DashMap<Arc<str>, Arc<Mutex<()>>>,

    limit: usize,
    db: fjall::Database,
    tree: Keyspace,
    history: Keyspace,
    sequence: AtomicU64,
}

impl Database {
    pub fn new<P: AsRef<Path>>(path: P, limit: usize) -> Result<Self> {
        let db = fjall::Database::builder(path).open()?;

        let tree = db.keyspace("values", KeyspaceCreateOptions::default)?;
        let history = db.keyspace("history", KeyspaceCreateOptions::default)?;

        Ok(Self {
            shared: Arc::new(Shared {
                recent: Default::default(),
                locks: DashMap::new(),
                limit,
                db,
                tree,
                history,
                sequence: AtomicU64::new(0),
            }),
        })
    }

    fn make_key(setting_code: &str, key: &str) -> String {
        format!("{setting_code}.{key}")
    }

    fn history_key(key: &str, timestamp: u64, sequence: u64) -> String {
        let mut buf = String::with_capacity(key.len() + 1 + 20 + 1 + 20);

        write!(buf, "{key}.{timestamp:020}.{sequence:020}").unwrap();

        buf
    }

    fn lock_for(&self, key: &str) -> Arc<Mutex<()>> {
        self.shared
            .locks
            .entry(Arc::from(key))
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    pub fn get_value(&self, setting_code: &str, key: &str) -> Result<Option<Arc<Vec<Value>>>> {
        let k = Self::make_key(setting_code, key);

        let lock = self.lock_for(&k);
        let _guard = lock.lock();

        self.load_value(&k)
    }

    fn load_value(&self, k: &str) -> Result<Option<Arc<Vec<Value>>>> {
        let Some(raw) = self.shared.tree.get(k)? else {
            return Ok(None);
        };

        let value: Vec<Value> = sonic_rs::from_slice(&raw)?;
        let value = Arc::new(value);

        Ok(Some(value))
    }

    pub fn insert_value(&self, setting_code: &str, key: Arc<str>, value: Value) -> Result<()> {
        let k = Self::make_key(setting_code, &key);
        let lock = self.lock_for(&k);
        let _guard = lock.lock();

        let values = match self.load_value(&k)? {
            Some(values) => {
                let mut values = (*values).clone();
                values.insert(0, value);
                values.truncate(self.shared.limit);
                Arc::new(values)
            }
            None => Arc::new(vec![value]),
        };

        let bytes = sonic_rs::to_vec(values.as_ref())?;
        self.shared.tree.insert(k, bytes)?;

        Ok(())
    }

    pub fn entries(&self) -> Result<Vec<DataEntry>> {
        Self::collect_entries(self.shared.tree.iter())
    }

    pub fn entries_by_setting_code(&self, setting_code: &str) -> Result<Vec<DataEntry>> {
        let prefix = Self::make_key(setting_code, "");
        Self::collect_entries(self.shared.tree.prefix(prefix))
    }

    fn collect_entries(iter: impl Iterator<Item = fjall::Guard>) -> Result<Vec<DataEntry>> {
        let mut result = Vec::new();

        for guard in iter {
            let (key, raw) = guard.into_inner()?;
            let values: Vec<Value> = sonic_rs::from_slice(&raw)?;

            let key = Arc::<str>::from(std::str::from_utf8(&key)?);

            result.push((key, Arc::new(values)));
        }

        Ok(result)
    }

    pub fn persist(&self) -> Result<()> {
        self.shared.db.persist(PersistMode::SyncAll)?;
        Ok(())
    }

    pub fn append_value(
        &self,
        setting_code: &str,
        key: Arc<str>,
        timestamp: u64,
        value: Value,
    ) -> Result<()> {
        let k = Self::make_key(setting_code, &key);
        let lock = self.lock_for(&k);
        let _guard = lock.lock();

        let buffer = self
            .shared
            .recent
            .entry(Arc::from(k.as_str()))
            .or_insert_with(|| Arc::new(Mutex::new(RingBuffer::new(self.shared.limit))))
            .clone();

        buffer.lock().push(value.clone());

        let sequence = self.shared.sequence.fetch_add(1, Ordering::Relaxed);

        let history_key = Self::history_key(&k, timestamp, sequence);
        let bytes = sonic_rs::to_vec(&value)?;

        self.shared.history.insert(history_key, bytes)?;

        Ok(())
    }

    pub fn get_history(
        &self,
        setting_code: &str,
        key: &str,
        from: Option<u64>,
        to: Option<u64>,
    ) -> Result<Vec<(u64, Value)>> {
        let k = Self::make_key(setting_code, key);

        let from_ts = from.unwrap_or(0);
        let to_ts = to.unwrap_or(u64::MAX);

        let range_start = format!("{k}.{from_ts:020}.00000000000000000000");
        let range_end = format!("{k}.{to_ts:020}.99999999999999999999");

        let mut result = Vec::new();

        for entry in self.shared.history.range(range_start..=range_end) {
            let history_key = entry.key()?;
            // let raw = entry.value()?;
            let raw = self.shared.history.get(&history_key)?.unwrap();
            let history_key = std::str::from_utf8(&history_key)?;

            let mut parts = history_key.rsplitn(3, '.');

            let _sequence = parts.next().unwrap().parse::<u64>().unwrap();

            let timestamp = parts.next().unwrap().parse::<u64>().unwrap();

            let value = sonic_rs::from_slice(&raw)?;

            result.push((timestamp, value));
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonic_rs::JsonValueTrait;

    fn val(s: &str) -> Value {
        sonic_rs::from_str(s).unwrap()
    }

    fn open(limit: usize) -> (tempfile::TempDir, Database) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::new(dir.path(), limit).expect("open database");
        (dir, db)
    }

    #[test]
    fn insert_value_is_concurrent_safe() {
        use std::thread;

        let (_dir, values) = open(1000);

        let workers = 10;
        let inserts_per_worker = 100;

        let mut handles = Vec::new();

        for worker in 0..workers {
            let values = values.clone();

            handles.push(thread::spawn(move || {
                for i in 0..inserts_per_worker {
                    values
                        .insert_value(
                            "test",
                            Arc::from("key"),
                            val(&format!("{}", worker * inserts_per_worker + i)),
                        )
                        .unwrap();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let result = values
            .get_value("test", "key")
            .unwrap()
            .expect("value exists");

        assert_eq!(result.len(), workers * inserts_per_worker);
    }

    #[test]
    fn insert_value_does_not_lose_concurrent_updates() {
        use std::collections::HashSet;
        use std::thread;

        let (_dir, values) = open(1000);

        let workers = 10;
        let inserts_per_worker = 100;

        let mut handles = Vec::new();

        for worker in 0..workers {
            let values = values.clone();

            handles.push(thread::spawn(move || {
                for i in 0..inserts_per_worker {
                    let n = worker * inserts_per_worker + i;

                    values
                        .insert_value("test", Arc::from("key"), val(&n.to_string()))
                        .unwrap();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let result = values
            .get_value("test", "key")
            .unwrap()
            .expect("value exists");

        assert_eq!(result.len(), workers * inserts_per_worker);

        let actual: HashSet<i64> = result.iter().map(|value| value.as_i64().unwrap()).collect();

        let expected: HashSet<i64> = (0..workers * inserts_per_worker)
            .map(|n| n as i64)
            .collect();

        assert_eq!(actual, expected);
    }

    #[test]
    fn insert_value_respects_limit_under_concurrency() {
        use std::thread;

        let limit = 100;
        let (_dir, values) = open(limit);

        let workers = 10;
        let inserts_per_worker = 100;

        let mut handles = Vec::new();

        for worker in 0..workers {
            let values = values.clone();

            handles.push(thread::spawn(move || {
                for i in 0..inserts_per_worker {
                    values
                        .insert_value(
                            "test",
                            Arc::from("key"),
                            val(&format!("{}", worker * inserts_per_worker + i)),
                        )
                        .unwrap();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let result = values
            .get_value("test", "key")
            .unwrap()
            .expect("value exists");

        assert_eq!(result.len(), limit);
    }

    #[test]
    fn entries_match_get_value_shape() {
        let (_dir, db) = open(10);
        db.insert_value("sc", Arc::from("k"), val("1")).unwrap();
        db.insert_value("sc", Arc::from("k"), val("2")).unwrap();

        let entries = db.entries().unwrap();
        assert_eq!(entries.len(), 1);

        let (key, values) = &entries[0];
        assert_eq!(&**key, "sc.k");
        assert_eq!(sonic_rs::to_string(&**values).unwrap(), "[2,1]");
        assert_eq!(
            sonic_rs::to_string(&*db.get_value("sc", "k").unwrap().unwrap()).unwrap(),
            "[2,1]"
        );
    }

    #[test]
    fn entries_by_setting_code_scans_only_its_prefix() {
        let (_dir, db) = open(10);
        db.insert_value("a", Arc::from("x"), val("1")).unwrap();
        db.insert_value("a", Arc::from("y"), val("2")).unwrap();
        db.insert_value("ab", Arc::from("x"), val("3")).unwrap();
        db.insert_value("b", Arc::from("x"), val("4")).unwrap();

        let mut keys: Vec<String> = db
            .entries_by_setting_code("a")
            .unwrap()
            .into_iter()
            .map(|(k, _)| k.to_string())
            .collect();
        keys.sort();

        assert_eq!(keys, ["a.x", "a.y"]);
        assert!(db.entries_by_setting_code("zzz").unwrap().is_empty());
        assert_eq!(db.entries().unwrap().len(), 4);
    }
}

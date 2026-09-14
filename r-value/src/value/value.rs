use dashmap::DashMap;
use fjall::{Database, Keyspace, KeyspaceCreateOptions, PersistMode};
use sonic_rs::Value;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
pub use crate::value::error::ValueError;
use crate::value::ring_buffer::RingBuffer;
use parking_lot::Mutex;
use std::fmt::Write;

pub type Result<T> = std::result::Result<T, ValueError>;

#[derive(Clone)]
pub struct Values {
    shared: Arc<Shared>,
}

// #[derive(Debug)]
struct Shared {
    values: DashMap<Arc<str>, Arc<Vec<Value>>>,
    recent: DashMap<Arc<str>, Arc<Mutex<RingBuffer<Value>>>>,
    locks: DashMap<Arc<str>, Arc<Mutex<()>>>,

    limit: usize,
    db: Database,
    tree: Keyspace,
    history: Keyspace,
    sequence: AtomicU64,
}

impl Values {
    pub fn new<P: AsRef<Path>>(path: P, limit: usize) -> Result<Self> {
        let db = Database::builder(path).open()?;

        let tree = db.keyspace("values", KeyspaceCreateOptions::default)?;
        let history = db.keyspace("history", KeyspaceCreateOptions::default)?;


        Ok(Self {
            shared: Arc::new(Shared {
                values: DashMap::new(),
                recent: DashMap::new(),
                limit,
                db,
                tree,
                history,
                sequence: AtomicU64::new(0),
                locks: DashMap::new(),
            }),
        })
    }

    fn make_key(setting_code: &str, key: &str) -> String {
        format!("{setting_code}.{key}")
    }


    fn history_key(key: &str, timestamp: u64, sequence: u64) -> String {
        let mut buf = String::with_capacity(
            key.len() + 1 + 20 + 1 + 20
        );

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

    pub fn get_value(
        &self,
        setting_code: &str,
        key: &str,
    ) -> Result<Option<Arc<Vec<Value>>>> {
        let k = Self::make_key(setting_code, key);

        let lock = self.lock_for(&k);
        let _guard = lock.lock();

        self.load_value(&k)
    }

    fn load_value(
        &self,
        k: &String
    ) -> Result<Option<Arc<Vec<Value>>>> {

        if let Some(value) = self.shared.values.get(k.as_str()) {
            return Ok(Some(Arc::clone(value.value())));
        }

        let Some(raw) = self.shared.tree.get(k.as_str())? else {
            return Ok(None);
        };

        let value: Vec<Value> = sonic_rs::from_slice(&raw)?;
        let value = Arc::new(value);

        self.shared
            .values
            .insert(Arc::from(k.as_str()), Arc::clone(&value));

        Ok(Some(value))
    }

    pub fn put_value(
        &self,
        setting_code: &str,
        key: Arc<str>,
        value: Value,
    ) -> Result<()> {
        let k = Self::make_key(setting_code, &key);
        let lock = self.lock_for(&k);
        let _guard = lock.lock();

        let value = Arc::new(vec![value]);

        self.shared
            .values
            .insert(Arc::from(k.as_str()), Arc::clone(&value));

        Ok(())
    }

    pub fn insert_value(
        &self,
        setting_code: &str,
        key: Arc<str>,
        value: Value,
    ) -> Result<()> {
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

        self.shared.values.insert(
            Arc::from(k.as_str()),
            Arc::clone(&values),
        );

        self.shared.tree.insert(k, bytes)?;

        Ok(())
    }

    pub fn remove_value(
        &self,
        setting_code: &str,
        key: &str,
    ) -> Result<bool> {
        let k = Self::make_key(setting_code, key);
        let lock = self.lock_for(&k);
        let _guard = lock.lock();

        let removed_from_cache =
            self.shared.values.remove(k.as_str()).is_some();

        let exists_in_db =
            self.shared.tree.get(k.as_str())?.is_some();

        if exists_in_db {
            self.shared.tree.remove(k)?;
        }

        Ok(removed_from_cache || exists_in_db)
    }

    pub fn entries(&self) -> Result<Vec<(Arc<str>, Arc<Vec<Value>>)>> {
        let mut result = Vec::new();

        for entry in self.shared.tree.iter() {
            // let (key, raw) = entry?;
            let key = entry.key()?;
            let value = self.shared.tree.get(&key)?.unwrap();
            let value: Value = sonic_rs::from_slice(&value)?;

            let key = Arc::<str>::from(
                std::str::from_utf8(&key)?
            );
            let value = Arc::new(vec![value]);

            self.shared.values.insert(
                Arc::clone(&key),
                Arc::clone(&value),
            );

            result.push((key, value));
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

        let buffer = self.shared.recent
            .entry(Arc::from(k.as_str()))
            .or_insert_with(|| {
                Arc::new(Mutex::new(
                    RingBuffer::new(self.shared.limit)
                ))
            })
            .clone();

        buffer.lock().push(value.clone());

        let sequence = self
            .shared
            .sequence
            .fetch_add(1, Ordering::Relaxed);

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

            let _sequence = parts
                .next()
                .unwrap()
                .parse::<u64>()
                .unwrap();

            let timestamp = parts
                .next()
                .unwrap()
                .parse::<u64>()
                .unwrap();

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

    #[test]
    fn put_get_roundtrip_scoped_by_setting_code() {
        let values = Values::new("values", 100).unwrap();
        values.put_value("code", Arc::from("k"), val(r#"{"instant":1}"#)).unwrap();
        let got = values.get_value("code", "k").unwrap().expect("present");
        assert_eq!(sonic_rs::to_string(&*got).unwrap(), r#"[{"instant":1}]"#);
        assert!(
            values
                .get_value("other_code", "k")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn put_overwrites_previous() {
        let values = Values::new("values", 100).unwrap();
        values.put_value("c", Arc::from("k"), val("1")).unwrap();
        values.put_value("c", Arc::from("k"), val("2")).unwrap();
        let got = values.get_value("c", "k").unwrap().expect("present");
        assert_eq!(sonic_rs::to_string(&*got).unwrap(), "[2]");
    }

    #[test]
    fn remove_reports_presence() {
        let values = Values::new("values", 100).unwrap();
        values.put_value("c", Arc::from("k"), val("1")).unwrap();
        assert!(values.remove_value("c", "k").unwrap());
        assert!(!values.remove_value("c", "k").unwrap());
        assert!(values.get_value("c", "k").unwrap().is_none());
    }

    #[test]
    fn entries_snapshot_lists_all() {
        let values = Values::new("values", 100).unwrap();
        values.put_value("c", Arc::from("a"), val("1")).unwrap();
        values.put_value("c", Arc::from("b"), val("2")).unwrap();
        let mut keys: Vec<_> = values.entries().unwrap().into_iter().map(|(k, _)| k).collect();
        keys.sort();
        assert_eq!(keys, [Arc::<str>::from("c.a"), Arc::from("c.b")]);
    }


    #[test]
    fn insert_value_is_concurrent_safe() {
        use std::thread;

        let values = Values::new("values-test-lock", 1000).unwrap();

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

        assert_eq!(
            result.len(),
            workers * inserts_per_worker
        );
    }


    #[test]
    fn insert_value_does_not_lose_concurrent_updates() {
        use std::collections::HashSet;
        use std::thread;

        let values = Values::new("values-test-lock", 1000).unwrap();

        let workers = 10;
        let inserts_per_worker = 100;

        let mut handles = Vec::new();

        for worker in 0..workers {
            let values = values.clone();

            handles.push(thread::spawn(move || {
                for i in 0..inserts_per_worker {
                    let n = worker * inserts_per_worker + i;

                    values
                        .insert_value(
                            "test",
                            Arc::from("key"),
                            val(&n.to_string()),
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

        let actual: HashSet<i64> = result
            .iter()
            .map(|value| value.as_i64().unwrap())
            .collect();

        let expected: HashSet<i64> = (0..workers * inserts_per_worker)
            .map(|n| n as i64)
            .collect();

        assert_eq!(actual, expected);
    }

    #[test]
    fn insert_value_respects_limit_under_concurrency() {
        use std::thread;

        let limit = 100;
        let values = Values::new("values-test-limit", limit).unwrap();

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
}

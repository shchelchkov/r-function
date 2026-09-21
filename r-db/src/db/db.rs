use crate::db::error::DatabaseError;
use crate::db::key;
use crate::db::registry::Tier;
use dashmap::DashMap;
use fjall::{Keyspace, KeyspaceCreateOptions};
use parking_lot::Mutex;
use sonic_rs::Value;
use std::hash::{BuildHasher, RandomState};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use fjall::PersistMode;

pub type Result<T> = std::result::Result<T, DatabaseError>;

pub const FORMAT: u32 = 3;

const META_FORMAT: &[u8] = b"format";
pub(crate) const META_HISTORY_SEQ: &[u8] = b"history_seq";

const STRIPES: usize = 64;

#[derive(Debug, Clone, Copy)]
pub struct DatabaseOptions {
        pub limit: usize,
        pub history: usize,
}

#[derive(Debug, Clone)]
pub struct DataEntry {
    pub setting_code: Arc<str>,
    pub key: Arc<str>,
    pub values: Arc<Vec<Value>>,
}

#[derive(Clone)]
pub struct Database {
    pub(crate) shared: Arc<Shared>,
}

pub(crate) struct Shared {
    path: PathBuf,
    pub(crate) options: DatabaseOptions,
    db: fjall::Database,
    pub(crate) values: Keyspace,
    pub(crate) history: Keyspace,
    pub(crate) meta: Keyspace,
        pub(crate) registry: Keyspace,
    locks: Box<[Mutex<()>]>,
    hasher: RandomState,
        pub(crate) sequence: Mutex<crate::db::history::Sequence>,
        pub(crate) counters: DashMap<Vec<u8>, u32>,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database")
            .field("path", &self.shared.path)
            .field("options", &self.shared.options)
            .finish_non_exhaustive()
    }
}

impl Shared {
                pub(crate) fn stripe(&self, raw_key: &[u8]) -> &Mutex<()> {
        let index = self.hasher.hash_one(raw_key) as usize % self.locks.len();
        &self.locks[index]
    }
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P, options: DatabaseOptions) -> Result<Self> {
        let options = DatabaseOptions {
            limit: options.limit.max(1),
            history: options.history.max(1),
        };

        let path = path.as_ref().to_path_buf();
        let db = fjall::Database::builder(&path).open()?;

        let values = db.keyspace("values", KeyspaceCreateOptions::default)?;
        let history = db.keyspace("history", KeyspaceCreateOptions::default)?;
        let meta = db.keyspace("meta", KeyspaceCreateOptions::default)?;
        let registry = db.keyspace("registry", KeyspaceCreateOptions::default)?;

        Self::check_format(&meta, &[&values, &history, &registry])?;
        let sequence = crate::db::history::Sequence::open(&meta)?;

        let locks = (0..STRIPES)
            .map(|_| Mutex::new(()))
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Ok(Self {
            shared: Arc::new(Shared {
                path,
                options,
                db,
                values,
                history,
                meta,
                registry,
                locks,
                hasher: RandomState::new(),
                sequence: Mutex::new(sequence),
                counters: DashMap::new(),
            }),
        })
    }

            fn check_format(meta: &Keyspace, data: &[&Keyspace]) -> Result<()> {
        match meta.get(META_FORMAT)? {
            Some(raw) => {
                let found = String::from_utf8_lossy(&raw).into_owned();
                if found.parse::<u32>().ok() == Some(FORMAT) {
                    Ok(())
                } else {
                    Err(DatabaseError::IncompatibleFormat {
                        found,
                        expected: FORMAT,
                    })
                }
            }
            None if Self::all_empty(data)? => {
                meta.insert(META_FORMAT, FORMAT.to_string())?;
                Ok(())
            }
            None => Err(DatabaseError::IncompatibleFormat {
                found: "unmarked (pre-format) data".into(),
                expected: FORMAT,
            }),
        }
    }

    fn all_empty(keyspaces: &[&Keyspace]) -> Result<bool> {
        for ks in keyspaces {
            if !ks.is_empty()? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn options(&self) -> DatabaseOptions {
        self.shared.options
    }

    pub fn get_value(&self, setting_code: &str, key: &str) -> Result<Option<Arc<Vec<Value>>>> {
        key::validate(setting_code, key)?;
        Ok(self.load(&key::value_key(setting_code, key))?.map(Arc::new))
    }

    fn load(&self, raw_key: &[u8]) -> Result<Option<Vec<Value>>> {
        let Some(raw) = self.shared.values.get(raw_key)? else {
            return Ok(None);
        };
        Ok(Some(sonic_rs::from_slice(&raw)?))
    }

        pub fn insert_value(&self, setting_code: &str, key: &str, value: Value) -> Result<()> {
        key::validate(setting_code, key)?;
        let raw_key = key::value_key(setting_code, key);

        let _guard = self.shared.stripe(&raw_key).lock();

        let mut values = self.load(&raw_key)?.unwrap_or_default();
        values.insert(0, value);
        values.truncate(self.shared.options.limit);

        let bytes = sonic_rs::to_vec(&values)?;
        self.shared.values.insert(&raw_key, bytes)?;
        self.touch_locked(&raw_key, Tier::Values)?;

        Ok(())
    }

        pub fn remove_value(&self, setting_code: &str, key: &str) -> Result<bool> {
        key::validate(setting_code, key)?;
        let raw_key = key::value_key(setting_code, key);

        let _guard = self.shared.stripe(&raw_key).lock();

        if !self.shared.values.contains_key(&raw_key)? {
            return Ok(false);
        }
        self.shared.values.remove(&raw_key)?;
        self.untouch_locked(&raw_key, Tier::Values)?;
        Ok(true)
    }

        pub fn entries(&self) -> Result<Vec<DataEntry>> {
        Self::collect_entries(self.shared.values.iter())
    }

        pub fn entries_by_setting_code(&self, setting_code: &str) -> Result<Vec<DataEntry>> {
        key::validate(setting_code, "")?;
        Self::collect_entries(self.shared.values.prefix(key::value_prefix(setting_code)))
    }

    fn collect_entries(iter: impl Iterator<Item = fjall::Guard>) -> Result<Vec<DataEntry>> {
        let mut result = Vec::new();

        for guard in iter {
            let (raw_key, raw) = guard.into_inner()?;
            let (setting_code, key) = key::decode_value_key(&raw_key)?;
            let values: Vec<Value> = sonic_rs::from_slice(&raw)?;

            result.push(DataEntry {
                setting_code: Arc::from(setting_code),
                key: Arc::from(key),
                values: Arc::new(values),
            });
        }

        Ok(result)
    }

            pub fn persist(&self, mode: PersistMode) -> Result<()> {
        self.shared.db.persist(mode)?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use sonic_rs::JsonValueTrait;

    pub(crate) fn val(s: &str) -> Value {
        sonic_rs::from_str(s).unwrap()
    }

            pub(crate) fn open(limit: usize) -> (tempfile::TempDir, Database) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::open(
            dir.path(),
            DatabaseOptions {
                limit,
                history: 1000,
            },
        )
        .expect("open database");
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
                            "key",
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
                        .insert_value("test", "key", val(&n.to_string()))
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
                            "key",
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
    fn insert_value_keeps_newest_first() {
        let (_dir, db) = open(10);
        db.insert_value("sc", "k", val("1")).unwrap();
        db.insert_value("sc", "k", val("2")).unwrap();

        assert_eq!(
            sonic_rs::to_string(&*db.get_value("sc", "k").unwrap().unwrap()).unwrap(),
            "[2,1]"
        );
        assert!(db.get_value("sc", "other").unwrap().is_none());
    }

    #[test]
    fn entries_decode_setting_code_and_key() {
        let (_dir, db) = open(10);
        db.insert_value("a.b", "c", val("1")).unwrap();
        db.insert_value("a", "b.c", val("2")).unwrap();

        let mut entries = db.entries().unwrap();
        entries.sort_by(|x, y| (&x.setting_code, &x.key).cmp(&(&y.setting_code, &y.key)));

        let pairs: Vec<(&str, &str, String)> = entries
            .iter()
            .map(|e| {
                (
                    &*e.setting_code,
                    &*e.key,
                    sonic_rs::to_string(&*e.values).unwrap(),
                )
            })
            .collect();
        assert_eq!(
            pairs,
            [
                ("a", "b.c", "[2]".to_string()),
                ("a.b", "c", "[1]".to_string())
            ]
        );
    }

    #[test]
    fn entries_by_setting_code_scans_only_its_prefix() {
        let (_dir, db) = open(10);
        db.insert_value("a", "x", val("1")).unwrap();
        db.insert_value("a", "y", val("2")).unwrap();
        db.insert_value("ab", "x", val("3")).unwrap();
        db.insert_value("b", "x", val("4")).unwrap();

        let mut keys: Vec<String> = db
            .entries_by_setting_code("a")
            .unwrap()
            .into_iter()
            .map(|e| e.key.to_string())
            .collect();
        keys.sort();

        assert_eq!(keys, ["x", "y"]);
        assert!(db.entries_by_setting_code("zzz").unwrap().is_empty());
        assert_eq!(db.entries().unwrap().len(), 4);
    }

    #[test]
    fn remove_value_reports_presence() {
        let (_dir, db) = open(10);
        db.insert_value("sc", "k", val("1")).unwrap();

        assert!(db.remove_value("sc", "k").unwrap());
        assert!(!db.remove_value("sc", "k").unwrap());
        assert!(db.get_value("sc", "k").unwrap().is_none());
    }

    #[test]
    fn invalid_keys_are_rejected_not_stored() {
        let (_dir, db) = open(10);

        assert!(matches!(
            db.insert_value("", "k", val("1")),
            Err(DatabaseError::InvalidKey(_))
        ));
        assert!(matches!(
            db.insert_value("a\0b", "k", val("1")),
            Err(DatabaseError::InvalidKey(_))
        ));
        assert!(matches!(
            db.get_value("a", "k\0"),
            Err(DatabaseError::InvalidKey(_))
        ));
        assert!(db.entries().unwrap().is_empty());
    }

    #[test]
    fn values_survive_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let options = DatabaseOptions {
            limit: 10,
            history: 10,
        };

        {
            let db = Database::open(dir.path(), options).unwrap();
            db.insert_value("sc", "k", val("1")).unwrap();
            db.persist(PersistMode::SyncAll).unwrap();
        }

        let db = Database::open(dir.path(), options).unwrap();
        assert_eq!(
            sonic_rs::to_string(&*db.get_value("sc", "k").unwrap().unwrap()).unwrap(),
            "[1]"
        );
    }

    #[test]
    fn foreign_format_marker_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let options = DatabaseOptions {
            limit: 10,
            history: 10,
        };

        {
            let db = fjall::Database::builder(dir.path()).open().unwrap();
            let meta = db.keyspace("meta", KeyspaceCreateOptions::default).unwrap();
            meta.insert(META_FORMAT, "1").unwrap();
            db.persist(PersistMode::SyncAll).unwrap();
        }

        let err = Database::open(dir.path(), options).unwrap_err();
        assert!(
            matches!(&err, DatabaseError::IncompatibleFormat { found, expected } if found == "1" && *expected == FORMAT),
            "{err}"
        );
    }

    #[test]
    fn unmarked_data_is_rejected_but_empty_directory_is_marked() {
        let dir = tempfile::tempdir().unwrap();
        let options = DatabaseOptions {
            limit: 10,
            history: 10,
        };

        {
            let db = fjall::Database::builder(dir.path()).open().unwrap();
            let values = db
                .keyspace("values", KeyspaceCreateOptions::default)
                .unwrap();
            values.insert("legacy.key", "[]").unwrap();
            db.persist(PersistMode::SyncAll).unwrap();
        }
        assert!(matches!(
            Database::open(dir.path(), options),
            Err(DatabaseError::IncompatibleFormat { .. })
        ));

        let fresh = tempfile::tempdir().unwrap();
        let db = Database::open(fresh.path(), options).unwrap();
        assert_eq!(
            &*db.shared.meta.get(META_FORMAT).unwrap().unwrap(),
            FORMAT.to_string().as_bytes()
        );
    }
}

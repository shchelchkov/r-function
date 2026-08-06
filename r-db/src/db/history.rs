use crate::db::db::{Database, META_HISTORY_SEQ, Result};
use crate::db::error::DatabaseError;
use crate::db::key;
use crate::db::registry::Tier;
use fjall::Keyspace;
use serde::Serialize;
use sonic_rs::Value;

const SEQUENCE_BLOCK: u64 = 4096;

pub(crate) struct Sequence {
    next: u64,
    hi: u64,
}

impl Sequence {
    pub(crate) fn open(meta: &Keyspace) -> Result<Self> {
        let start = match meta.get(META_HISTORY_SEQ)? {
            Some(raw) => u64::from_be_bytes(raw[..].try_into().map_err(|_| {
                DatabaseError::Corrupt(format!("history_seq is not u64: {:?}", &raw[..]))
            })?),
            None => 0,
        };
        let mut sequence = Self {
            next: start,
            hi: start,
        };
        sequence.reserve(meta)?;
        Ok(sequence)
    }

    pub(crate) fn next(&mut self, meta: &Keyspace) -> Result<u64> {
        if self.next == self.hi {
            self.reserve(meta)?;
        }
        let seq = self.next;
        self.next += 1;
        Ok(seq)
    }

    fn reserve(&mut self, meta: &Keyspace) -> Result<()> {
        let hi = self
            .hi
            .checked_add(SEQUENCE_BLOCK)
            .ok_or_else(|| DatabaseError::Corrupt("history_seq overflow".into()))?;
        meta.insert(META_HISTORY_SEQ, hi.to_be_bytes())?;
        self.hi = hi;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryEntry {
    pub timestamp: u64,
    pub value: Value,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HistoryQuery {
    pub from: Option<u64>,
    pub to: Option<u64>,
    pub limit: Option<usize>,
    pub newest_first: bool,
}

impl Database {
            pub fn trim_every(&self) -> u32 {
        (self.shared.options.history / 10).max(16) as u32
    }

    pub fn append_value(
        &self,
        setting_code: &str,
        key: &str,
        timestamp: u64,
        value: Value,
    ) -> Result<()> {
        key::validate(setting_code, key)?;
        let raw_key = key::value_key(setting_code, key);
        let prefix = key::history_prefix(setting_code, key);
        let bytes = sonic_rs::to_vec(&value)?;

        let _guard = self.shared.stripe(&raw_key).lock();

        let sequence = self.shared.sequence.lock().next(&self.shared.meta)?;
        self.shared
            .history
            .insert(key::history_key(&prefix, timestamp, sequence), bytes)?;

        self.touch_locked(&raw_key, Tier::History)?;

        if self.count_append(&prefix) {
            self.trim_locked(&prefix)?;
        }

        Ok(())
    }

            fn count_append(&self, prefix: &[u8]) -> bool {
        let trim_every = self.trim_every();
        match self.shared.counters.get_mut(prefix) {
            Some(mut count) => {
                *count += 1;
                if *count >= trim_every {
                    *count = 0;
                    true
                } else {
                    false
                }
            }
            None => {
                self.shared.counters.insert(prefix.to_vec(), 1);
                false
            }
        }
    }

    pub fn get_history(
        &self,
        setting_code: &str,
        key: &str,
        query: HistoryQuery,
    ) -> Result<Vec<HistoryEntry>> {
        key::validate(setting_code, key)?;
        let prefix = key::history_prefix(setting_code, key);
        let range = key::history_range(
            &prefix,
            query.from.unwrap_or(u64::MIN),
            query.to.unwrap_or(u64::MAX),
        );
        let limit = query.limit.unwrap_or(usize::MAX);

        let iter = self.shared.history.range(range);
        let mut result = Vec::new();

        if query.newest_first {
            for guard in iter.rev().take(limit) {
                result.push(Self::decode_history(guard)?);
            }
        } else {
            for guard in iter.take(limit) {
                result.push(Self::decode_history(guard)?);
            }
        }

        Ok(result)
    }

    fn decode_history(guard: fjall::Guard) -> Result<HistoryEntry> {
        let (raw_key, raw) = guard.into_inner()?;
        let (timestamp, _sequence) = key::decode_history_key(&raw_key)?;
        Ok(HistoryEntry {
            timestamp,
            value: sonic_rs::from_slice(&raw)?,
        })
    }

        pub fn trim_history(&self, setting_code: &str, key: &str) -> Result<usize> {
        key::validate(setting_code, key)?;
        let raw_key = key::value_key(setting_code, key);
        let prefix = key::history_prefix(setting_code, key);

        let _guard = self.shared.stripe(&raw_key).lock();
        self.trim_locked(&prefix)
    }

            fn trim_locked(&self, prefix: &[u8]) -> Result<usize> {
        let keep = self.shared.options.history;

        let mut stale = Vec::new();
        for guard in self.shared.history.prefix(prefix).rev().skip(keep) {
            stale.push(guard.key()?);
        }

        for raw_key in &stale {
            self.shared.history.remove(raw_key.clone())?;
        }

        Ok(stale.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::db::tests::val;
    use crate::db::db::{DatabaseOptions, PersistMode};

    fn open(history: usize) -> (tempfile::TempDir, Database) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::open(dir.path(), DatabaseOptions { limit: 10, history }).expect("open");
        (dir, db)
    }

    fn timestamps(entries: &[HistoryEntry]) -> Vec<u64> {
        entries.iter().map(|e| e.timestamp).collect()
    }

    fn values(entries: &[HistoryEntry]) -> Vec<String> {
        entries
            .iter()
            .map(|e| sonic_rs::to_string(&e.value).unwrap())
            .collect()
    }

    #[test]
    fn history_is_ordered_and_bounds_are_inclusive() {
        let (_dir, db) = open(100);
        for ts in [5u64, 1, 3, 4, 2] {
            db.append_value("sc", "k", ts, val(&ts.to_string()))
                .unwrap();
        }

        let all = db.get_history("sc", "k", HistoryQuery::default()).unwrap();
        assert_eq!(timestamps(&all), [1, 2, 3, 4, 5]);

        let mid = db
            .get_history(
                "sc",
                "k",
                HistoryQuery {
                    from: Some(2),
                    to: Some(4),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(timestamps(&mid), [2, 3, 4]);
        assert_eq!(values(&mid), ["2", "3", "4"]);
    }

    #[test]
    fn history_limit_and_newest_first() {
        let (_dir, db) = open(100);
        for ts in 1..=5u64 {
            db.append_value("sc", "k", ts, val("null")).unwrap();
        }

        let newest = db
            .get_history(
                "sc",
                "k",
                HistoryQuery {
                    limit: Some(2),
                    newest_first: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(timestamps(&newest), [5, 4]);

        let oldest = db
            .get_history(
                "sc",
                "k",
                HistoryQuery {
                    limit: Some(2),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(timestamps(&oldest), [1, 2]);
    }

    #[test]
    fn same_timestamp_keeps_insertion_order() {
        let (_dir, db) = open(100);
        for v in ["\"a\"", "\"b\"", "\"c\""] {
            db.append_value("sc", "k", 7, val(v)).unwrap();
        }

        let entries = db.get_history("sc", "k", HistoryQuery::default()).unwrap();
        assert_eq!(values(&entries), ["\"a\"", "\"b\"", "\"c\""]);
    }

    #[test]
    fn history_is_scoped_per_key() {
        let (_dir, db) = open(100);
        db.append_value("a.b", "c", 1, val("1")).unwrap();
        db.append_value("a", "b.c", 1, val("2")).unwrap();
        db.append_value("a", "b", 1, val("3")).unwrap();

        let get =
            |sc: &str, k: &str| values(&db.get_history(sc, k, HistoryQuery::default()).unwrap());
        assert_eq!(get("a.b", "c"), ["1"]);
        assert_eq!(get("a", "b.c"), ["2"]);
        assert_eq!(get("a", "b"), ["3"]);
        assert!(get("a", "zzz").is_empty());
    }

    #[test]
    fn sequence_survives_reopen_so_equal_timestamps_do_not_collide() {
        let dir = tempfile::tempdir().unwrap();
        let options = DatabaseOptions {
            limit: 10,
            history: 100,
        };

        {
            let db = Database::open(dir.path(), options).unwrap();
            db.append_value("sc", "k", 5, val("\"before\"")).unwrap();
            db.persist(PersistMode::SyncAll).unwrap();
        }

        let db = Database::open(dir.path(), options).unwrap();
        db.append_value("sc", "k", 5, val("\"after\"")).unwrap();

        let entries = db.get_history("sc", "k", HistoryQuery::default()).unwrap();
        assert_eq!(values(&entries), ["\"before\"", "\"after\""]);
    }

    #[test]
    fn retention_trims_to_history() {
        let (_dir, db) = open(50);
        let trim_every = db.trim_every() as usize;
        assert_eq!(trim_every, 16);

        for ts in 0..500u64 {
            db.append_value("sc", "k", ts, val(&ts.to_string()))
                .unwrap();
        }

        let between = db.get_history("sc", "k", HistoryQuery::default()).unwrap();
        assert!(
            between.len() <= 50 + trim_every,
            "between trims: {}",
            between.len()
        );

        let removed = db.trim_history("sc", "k").unwrap();
        let after = db.get_history("sc", "k", HistoryQuery::default()).unwrap();
        assert_eq!(removed, between.len() - 50);
        assert_eq!(after.len(), 50);
        assert_eq!(timestamps(&after), (450..500u64).collect::<Vec<_>>());
    }

    #[test]
    fn invalid_history_keys_are_rejected() {
        let (_dir, db) = open(10);
        assert!(matches!(
            db.append_value("", "k", 1, val("1")),
            Err(DatabaseError::InvalidKey(_))
        ));
        assert!(matches!(
            db.get_history("a\0", "k", HistoryQuery::default()),
            Err(DatabaseError::InvalidKey(_))
        ));
    }
}

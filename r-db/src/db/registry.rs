use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::db::db::{Database, Result};
use crate::db::key;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct KeyMeta {
    pub has_values: bool,
    pub has_history: bool,
            pub updated_at: u64,
}

#[derive(Debug, Clone)]
pub struct KeyEntry {
    pub setting_code: Arc<str>,
    pub key: Arc<str>,
    pub meta: KeyMeta,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Tier {
    Values,
    History,
}

impl KeyMeta {
    fn set(&mut self, tier: Tier, present: bool) {
        match tier {
            Tier::Values => self.has_values = present,
            Tier::History => self.has_history = present,
        }
    }

    fn is_empty(&self) -> bool {
        !self.has_values && !self.has_history
    }
}

impl Database {
                pub(crate) fn touch_locked(&self, raw_key: &[u8], tier: Tier) -> Result<()> {
        let mut meta = self.key_meta_raw(raw_key)?.unwrap_or_default();
        meta.set(tier, true);
        meta.updated_at = now_ms();
        self.shared
            .registry
            .insert(raw_key, sonic_rs::to_vec(&meta)?)?;
        Ok(())
    }

        pub(crate) fn untouch_locked(&self, raw_key: &[u8], tier: Tier) -> Result<()> {
        let Some(mut meta) = self.key_meta_raw(raw_key)? else {
            return Ok(());
        };
        meta.set(tier, false);
        if meta.is_empty() {
            self.shared.registry.remove(raw_key)?;
        } else {
            self.shared
                .registry
                .insert(raw_key, sonic_rs::to_vec(&meta)?)?;
        }
        Ok(())
    }

    fn key_meta_raw(&self, raw_key: &[u8]) -> Result<Option<KeyMeta>> {
        let Some(raw) = self.shared.registry.get(raw_key)? else {
            return Ok(None);
        };
        Ok(Some(sonic_rs::from_slice(&raw)?))
    }

    pub fn key_meta(&self, setting_code: &str, key: &str) -> Result<Option<KeyMeta>> {
        key::validate(setting_code, key)?;
        self.key_meta_raw(&key::value_key(setting_code, key))
    }

        pub fn registry(&self) -> Result<Vec<KeyEntry>> {
        Self::collect_keys(self.shared.registry.iter())
    }

            pub fn keys(&self, setting_code: &str) -> Result<Vec<KeyEntry>> {
        key::validate(setting_code, "")?;
        Self::collect_keys(self.shared.registry.prefix(key::value_prefix(setting_code)))
    }

    fn collect_keys(iter: impl Iterator<Item = fjall::Guard>) -> Result<Vec<KeyEntry>> {
        let mut result = Vec::new();

        for guard in iter {
            let (raw_key, raw) = guard.into_inner()?;
            let (setting_code, key) = key::decode_value_key(&raw_key)?;
            result.push(KeyEntry {
                setting_code: Arc::from(setting_code),
                key: Arc::from(key),
                meta: sonic_rs::from_slice(&raw)?,
            });
        }

        Ok(result)
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::db::tests::{open, val};
    use crate::db::db::{Database, DatabaseOptions, PersistMode};
    use crate::db::error::DatabaseError;

    fn names(entries: &[KeyEntry]) -> Vec<String> {
        entries
            .iter()
            .map(|e| format!("{}/{}", e.setting_code, e.key))
            .collect()
    }

    #[test]
    fn insert_and_append_set_their_tiers() {
        let (_dir, db) = open(10);

        db.insert_value("sc", "k", val("1")).unwrap();
        let meta = db.key_meta("sc", "k").unwrap().unwrap();
        assert!(meta.has_values && !meta.has_history);
        assert!(meta.updated_at > 0);

        db.append_value("sc", "k", 7, val("2")).unwrap();
        let meta = db.key_meta("sc", "k").unwrap().unwrap();
        assert!(meta.has_values && meta.has_history);

        db.append_value("sc", "h", 7, val("3")).unwrap();
        let meta = db.key_meta("sc", "h").unwrap().unwrap();
        assert!(!meta.has_values && meta.has_history);

        assert_eq!(names(&db.keys("sc").unwrap()), ["sc/h", "sc/k"]);
    }

    #[test]
    fn remove_value_clears_tier_or_entry() {
        let (_dir, db) = open(10);

        db.insert_value("sc", "both", val("1")).unwrap();
        db.append_value("sc", "both", 1, val("1")).unwrap();
        assert!(db.remove_value("sc", "both").unwrap());
        let meta = db.key_meta("sc", "both").unwrap().unwrap();
        assert!(!meta.has_values && meta.has_history);

        db.insert_value("sc", "only", val("1")).unwrap();
        assert!(db.remove_value("sc", "only").unwrap());
        assert!(db.key_meta("sc", "only").unwrap().is_none());

        assert!(!db.remove_value("sc", "none").unwrap());
        assert!(db.key_meta("sc", "none").unwrap().is_none());

        assert_eq!(names(&db.keys("sc").unwrap()), ["sc/both"]);
    }

    #[test]
    fn keys_scan_only_their_prefix() {
        let (_dir, db) = open(10);
        db.insert_value("a", "x", val("1")).unwrap();
        db.insert_value("ab", "x", val("1")).unwrap();
        db.append_value("a.b", "c", 1, val("1")).unwrap();

        assert_eq!(names(&db.keys("a").unwrap()), ["a/x"]);
        assert_eq!(names(&db.keys("ab").unwrap()), ["ab/x"]);
        assert_eq!(names(&db.keys("a.b").unwrap()), ["a.b/c"]);
        assert!(db.keys("none").unwrap().is_empty());
        assert_eq!(names(&db.registry().unwrap()), ["a/x", "a.b/c", "ab/x"]);
    }

    #[test]
    fn registry_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let options = DatabaseOptions {
            limit: 10,
            history: 10,
        };

        {
            let db = Database::open(dir.path(), options).unwrap();
            db.insert_value("sc", "k", val("1")).unwrap();
            db.append_value("sc", "h", 1, val("1")).unwrap();
            db.persist(PersistMode::SyncAll).unwrap();
        }

        let db = Database::open(dir.path(), options).unwrap();
        assert_eq!(names(&db.keys("sc").unwrap()), ["sc/h", "sc/k"]);
    }

    #[test]
    fn invalid_keys_are_rejected() {
        let (_dir, db) = open(10);
        assert!(matches!(db.keys(""), Err(DatabaseError::InvalidKey(_))));
        assert!(matches!(
            db.key_meta("sc", "a\0b"),
            Err(DatabaseError::InvalidKey(_))
        ));
    }
}

use crate::value::error::ValueError;
use dashmap::{DashMap, DashSet};
use sonic_rs::Value;
use std::sync::Arc;

pub type Result<T> = std::result::Result<T, ValueError>;

pub type Entry = (Arc<str>, Arc<Vec<Value>>);

pub type Restore = Arc<dyn Fn(&str, Option<i64>) -> Option<Arc<Vec<Value>>> + Send + Sync>;

#[derive(Debug, Clone, Default)]
pub struct Values {
    shared: Arc<Shared>,
}

#[derive(Default)]
struct Shared {
    values: DashMap<Arc<str>, Arc<Vec<Value>>>,
    restore: Option<Restore>,
    checked: DashSet<Arc<str>>,
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared")
            .field("values", &self.values.len())
            .field("restore", &self.restore.is_some())
            .finish_non_exhaustive()
    }
}

impl Values {
    pub fn new() -> Result<Self> {
        Ok(Self::default())
    }

    pub fn with_restore(restore: Restore) -> Result<Self> {
        Ok(Self {
            shared: Arc::new(Shared {
                restore: Some(restore),
                ..Shared::default()
            }),
        })
    }

    pub fn get_value(&self, setting_code: &str, key: &str) -> Result<Option<Arc<Vec<Value>>>> {
        self.lookup(setting_code, key, None)
    }

    pub fn get_value_at(
        &self,
        setting_code: &str,
        key: &str,
        at: i64,
    ) -> Result<Option<Arc<Vec<Value>>>> {
        self.lookup(setting_code, key, (at > 0).then_some(at))
    }

    fn lookup(
        &self,
        setting_code: &str,
        key: &str,
        at: Option<i64>,
    ) -> Result<Option<Arc<Vec<Value>>>> {
        let k = format!("{setting_code}.{key}");
        if let Some(r) = self.shared.values.get(k.as_str()) {
            return Ok(Some(Arc::clone(r.value())));
        }
        Ok(self.restore(k, at))
    }

    fn restore(&self, k: String, at: Option<i64>) -> Option<Arc<Vec<Value>>> {
        let restore = self.shared.restore.as_ref()?;
        if self.shared.checked.contains(k.as_str()) {
            return None;
        }
        let found = restore(&k, at);
        match self.shared.values.entry(Arc::from(k)) {
            dashmap::Entry::Occupied(e) => Some(Arc::clone(e.get())),
            dashmap::Entry::Vacant(e) => {
                if !self.shared.checked.insert(Arc::clone(e.key())) {
                    return None;
                }
                found.map(|v| Arc::clone(e.insert(v).value()))
            }
        }
    }

    fn mark_checked(&self, k: &Arc<str>) {
        if self.shared.restore.is_some() {
            self.shared.checked.insert(Arc::clone(k));
        }
    }

    pub fn put_value(&self, setting_code: &str, key: Arc<str>, v: Value) -> Result<()> {
        let k: Arc<str> = Arc::from(format!("{setting_code}.{key}"));
        self.shared.values.insert(Arc::clone(&k), Arc::new(vec![v]));
        self.mark_checked(&k);

        Ok(())
    }

    pub fn remove_value(&self, setting_code: &str, key: &str) -> Result<bool> {
        let k: Arc<str> = Arc::from(format!("{setting_code}.{key}"));
        self.mark_checked(&k);
        let r = self.shared.values.remove(&k);

        Ok(r.is_some())
    }

    pub fn entries(&self) -> Result<Vec<Entry>> {
        let result = self
            .shared
            .values
            .iter()
            .map(|e| (Arc::clone(e.key()), Arc::clone(e.value())))
            .collect();

        Ok(result)
    }

    pub fn absent_keys(&self) -> Result<Vec<Arc<str>>> {
        let checked: Vec<Arc<str>> = self
            .shared
            .checked
            .iter()
            .map(|k| Arc::clone(k.key()))
            .collect();
        let result = checked
            .into_iter()
            .filter(|k| !self.shared.values.contains_key(k))
            .collect();

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn val(s: &str) -> Value {
        sonic_rs::from_str(s).unwrap()
    }

    #[test]
    fn put_get_roundtrip_scoped_by_setting_code() {
        let values = Values::new().unwrap();
        values
            .put_value("code", Arc::from("k"), val(r#"{"instant":1}"#))
            .unwrap();
        let got = values.get_value("code", "k").unwrap().expect("present");
        assert_eq!(sonic_rs::to_string(&*got).unwrap(), r#"[{"instant":1}]"#);
        assert!(values.get_value("other_code", "k").unwrap().is_none());
    }

    #[test]
    fn put_overwrites_previous() {
        let values = Values::new().unwrap();
        values.put_value("c", Arc::from("k"), val("1")).unwrap();
        values.put_value("c", Arc::from("k"), val("2")).unwrap();
        let got = values.get_value("c", "k").unwrap().expect("present");
        assert_eq!(sonic_rs::to_string(&*got).unwrap(), "[2]");
    }

    #[test]
    fn remove_reports_presence() {
        let values = Values::new().unwrap();
        values.put_value("c", Arc::from("k"), val("1")).unwrap();
        assert!(values.remove_value("c", "k").unwrap());
        assert!(!values.remove_value("c", "k").unwrap());
        assert!(values.get_value("c", "k").unwrap().is_none());
    }

    fn snapshot(pairs: &[(&str, &str)], calls: Arc<std::sync::atomic::AtomicUsize>) -> Restore {
        let map: std::collections::HashMap<String, Arc<Vec<Value>>> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), Arc::new(vec![val(v)])))
            .collect();
        Arc::new(move |k: &str, _at: Option<i64>| {
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            map.get(k).cloned()
        })
    }

    #[test]
    fn miss_is_restored_once_and_kept_in_memory() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let values = Values::with_restore(snapshot(&[("c.k", "7")], calls.clone())).unwrap();
        let got = values.get_value("c", "k").unwrap().expect("из снимка");
        assert_eq!(sonic_rs::to_string(&*got).unwrap(), "[7]");
        assert!(
            values.get_value("c", "k").unwrap().is_some(),
            "уже в памяти"
        );
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "снимок спрошен один раз"
        );
        assert_eq!(values.entries().unwrap().len(), 1);
    }

    #[test]
    fn key_missing_in_snapshot_is_asked_once() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let values = Values::with_restore(snapshot(&[], calls.clone())).unwrap();
        assert!(values.get_value("c", "none").unwrap().is_none());
        assert!(values.get_value("c", "none").unwrap().is_none());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn written_or_removed_key_is_not_restored() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let values =
            Values::with_restore(snapshot(&[("c.a", "1"), ("c.b", "2")], calls.clone())).unwrap();
        values.put_value("c", Arc::from("a"), val("10")).unwrap();
        let got = values.get_value("c", "a").unwrap().expect("present");
        assert_eq!(
            sonic_rs::to_string(&*got).unwrap(),
            "[10]",
            "записанное в этом запуске новее снимка"
        );
        assert!(
            !values.remove_value("c", "b").unwrap(),
            "в памяти ключа не было"
        );
        assert!(
            values.get_value("c", "b").unwrap().is_none(),
            "удалённое из снимка не возвращается"
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn without_restore_keys_are_not_tracked() {
        let values = Values::new().unwrap();
        values.put_value("c", Arc::from("k"), val("1")).unwrap();
        assert!(values.remove_value("c", "k").unwrap());
        assert!(values.get_value("c", "k").unwrap().is_none());
        assert!(values.shared.checked.is_empty());
        assert!(values.absent_keys().unwrap().is_empty());
    }

    fn sorted(mut keys: Vec<Arc<str>>) -> Vec<Arc<str>> {
        keys.sort();
        keys
    }

    #[test]
    fn absent_keys_skip_keys_untouched_in_this_run() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let snap = snapshot(&[("c.a", "1"), ("c.b", "2"), ("c.c", "3")], calls);
        let values = Values::with_restore(snap).unwrap();
        values.get_value("c", "a").unwrap().expect("из снимка");
        values.get_value("c", "b").unwrap().expect("из снимка");
        assert!(values.remove_value("c", "b").unwrap());
        assert!(
            values.get_value("c", "n").unwrap().is_none(),
            "в снимке нет"
        );
        assert!(
            values.get_value("c", "w").unwrap().is_none(),
            "в снимке нет"
        );
        values.put_value("c", Arc::from("w"), val("4")).unwrap();

        assert_eq!(
            sorted(values.absent_keys().unwrap()),
            [Arc::<str>::from("c.b"), Arc::from("c.n")]
        );
        let present = values
            .entries()
            .unwrap()
            .into_iter()
            .map(|(k, _)| k)
            .collect();
        assert_eq!(
            sorted(present),
            [Arc::<str>::from("c.a"), Arc::from("c.w")],
            "c.c не трогали — его нет ни там, ни там"
        );
    }

    fn race_with_restore(during: impl FnOnce(&Values)) -> (Values, Option<Arc<Vec<Value>>>) {
        use std::sync::mpsc;
        let (reading_tx, reading_rx) = mpsc::channel();
        let (go_tx, go_rx) = mpsc::channel::<()>();
        let go_rx = std::sync::Mutex::new(go_rx);
        let values = Values::with_restore(Arc::new(move |_: &str, _: Option<i64>| {
            reading_tx.send(()).unwrap();
            go_rx.lock().unwrap().recv().unwrap();
            Some(Arc::new(vec![val("1")]))
        }))
        .unwrap();
        let got = std::thread::scope(|s| {
            let reader = s.spawn(|| values.get_value("c", "k").unwrap());
            reading_rx.recv().unwrap();
            during(&values);
            go_tx.send(()).unwrap();
            reader.join().unwrap()
        });
        (values, got)
    }

    #[test]
    fn key_written_while_snapshot_is_read_keeps_new_value() {
        let (values, got) =
            race_with_restore(|v| v.put_value("c", Arc::from("k"), val("2")).unwrap());
        assert_eq!(sonic_rs::to_string(&*got.expect("present")).unwrap(), "[2]");
        assert!(values.absent_keys().unwrap().is_empty());
    }

    #[test]
    fn restore_gets_the_date_of_the_request() {
        let asked = Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = Arc::clone(&asked);
        let values = Values::with_restore(Arc::new(move |k: &str, at: Option<i64>| {
            log.lock().unwrap().push((k.to_owned(), at));
            None
        }))
        .unwrap();
        values.get_value_at("c", "a", 1_791_724_500_000).unwrap();
        values.get_value_at("c", "b", 0).unwrap();
        values.get_value("c", "d").unwrap();
        values.get_value_at("c", "a", 1).unwrap();
        assert_eq!(
            *asked.lock().unwrap(),
            [
                ("c.a".to_owned(), Some(1_791_724_500_000)),
                ("c.b".to_owned(), None),
                ("c.d".to_owned(), None)
            ],
            "без даты — None; по решённому ключу restore больше не спрашивается"
        );
    }

    #[test]
    fn key_removed_while_snapshot_is_read_is_not_put_back() {
        let (values, got) = race_with_restore(|v| assert!(!v.remove_value("c", "k").unwrap()));
        assert!(
            got.is_none(),
            "удалённое во время чтения снимка не возвращается"
        );
        assert!(values.get_value("c", "k").unwrap().is_none());
        assert_eq!(values.absent_keys().unwrap(), [Arc::<str>::from("c.k")]);
    }

    #[test]
    fn entries_snapshot_lists_all() {
        let values = Values::new().unwrap();
        values.put_value("c", Arc::from("a"), val("1")).unwrap();
        values.put_value("c", Arc::from("b"), val("2")).unwrap();
        let mut keys: Vec<_> = values
            .entries()
            .unwrap()
            .into_iter()
            .map(|(k, _)| k)
            .collect();
        keys.sort();
        assert_eq!(keys, [Arc::<str>::from("c.a"), Arc::from("c.b")]);
    }
}

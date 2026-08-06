use dashmap::DashMap;
use sonic_rs::Value;
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Clone, Default)]
pub struct Values {
    shared: Arc<Shared>,
}

#[derive(Debug, Default)]
struct Shared {
    values: DashMap<Arc<str>, Arc<Vec<Value>>>,
}

impl Values {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_value(&self, setting_code: &str, key: &str) -> Option<Arc<Vec<Value>>> {
        let k = format!("{setting_code}.{key}");
        self.shared
            .values
            .get(k.as_str())
            .map(|r| Arc::clone(r.value()))
    }

    pub fn put_value(&self, setting_code: &str, key: Arc<str>, v: Value) {
        let k = format!("{setting_code}.{key}");
        self.shared.values.insert(Arc::from(k), Arc::new(vec![v]));
    }

    pub fn remove_value(&self, setting_code: &str, key: &str) -> bool {
        let k = format!("{setting_code}.{key}");
        self.shared.values.remove(k.as_str()).is_some()
    }

    pub fn entries(&self) -> Vec<(Arc<str>, Arc<Vec<Value>>)> {
        self.shared
            .values
            .iter()
            .map(|e| (Arc::clone(e.key()), Arc::clone(e.value())))
            .collect()
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
        let values = Values::new();
        values.put_value("code", Arc::from("k"), val(r#"{"instant":1}"#));
        let got = values.get_value("code", "k").expect("present");
        assert_eq!(sonic_rs::to_string(&*got).unwrap(), r#"[{"instant":1}]"#);
        assert!(values.get_value("other_code", "k").is_none());
    }

    #[test]
    fn put_overwrites_previous() {
        let values = Values::new();
        values.put_value("c", Arc::from("k"), val("1"));
        values.put_value("c", Arc::from("k"), val("2"));
        let got = values.get_value("c", "k").expect("present");
        assert_eq!(sonic_rs::to_string(&*got).unwrap(), "[2]");
    }

    #[test]
    fn remove_reports_presence() {
        let values = Values::new();
        values.put_value("c", Arc::from("k"), val("1"));
        assert!(values.remove_value("c", "k"));
        assert!(!values.remove_value("c", "k"));
        assert!(values.get_value("c", "k").is_none());
    }

    #[test]
    fn entries_snapshot_lists_all() {
        let values = Values::new();
        values.put_value("c", Arc::from("a"), val("1"));
        values.put_value("c", Arc::from("b"), val("2"));
        let mut keys: Vec<_> = values.entries().into_iter().map(|(k, _)| k).collect();
        keys.sort();
        assert_eq!(keys, [Arc::<str>::from("c.a"), Arc::from("c.b")]);
    }
}

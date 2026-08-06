use dashmap::DashMap;
use sonic_rs::Value;
use std::sync::Arc;
use crate::value::error::ValueError;

pub type Result<T> = std::result::Result<T, ValueError>;

#[derive(Debug, Clone, Default)]
pub struct Values {
    shared: Arc<Shared>,
}

#[derive(Debug, Default)]
struct Shared {
    values: DashMap<Arc<str>, Arc<Vec<Value>>>,
}

impl Values {
    pub fn new() -> Result<Self> {
        Ok(Self::default())
    }

    pub fn get_value(&self, setting_code: &str, key: &str) -> Result<Option<Arc<Vec<Value>>>> {
        let k = format!("{setting_code}.{key}");
        Ok(self.shared
            .values
            .get(k.as_str())
            .map(|r| Arc::clone(r.value())))
    }

    pub fn put_value(&self, setting_code: &str, key: Arc<str>, v: Value) -> Result<()> {
        let k = format!("{setting_code}.{key}");
        self.shared.values.insert(Arc::from(k), Arc::new(vec![v]));

        Ok(())
    }

    pub fn remove_value(&self, setting_code: &str, key: &str) -> Result<bool> {
        let k = format!("{setting_code}.{key}");
        let r = self.shared.values.remove(k.as_str());

        Ok(r.is_some())
    }

    pub fn entries(&self) -> Result<Vec<(Arc<str>, Arc<Vec<Value>>)>> {
        let result = self.shared
            .values
            .iter()
            .map(|e| (Arc::clone(e.key()), Arc::clone(e.value()))
            )
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

    #[test]
    fn entries_snapshot_lists_all() {
        let values = Values::new().unwrap();
        values.put_value("c", Arc::from("a"), val("1")).unwrap();
        values.put_value("c", Arc::from("b"), val("2")).unwrap();
        let mut keys: Vec<_> = values.entries().unwrap().into_iter().map(|(k, _)| k).collect();
        keys.sort();
        assert_eq!(keys, [Arc::<str>::from("c.a"), Arc::from("c.b")]);
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use sonic_rs::{json, JsonContainerTrait, Object, JsonValueTrait};
    use f_common::fun::{f_dt_seconds, f_get_key_value, f_target_key, instant};

    #[test]
    fn instant_reads_integer() {
        let v = json!({"instant": 1_700_000_000_123_i64});
        let obj = v.as_object().expect("obj");
        assert_eq!(instant(obj), 1_700_000_000_123);
    }

    #[test]
    fn instant_reads_string() {
        let v = json!({"instant": "1700000000123"});
        let obj = v.as_object().expect("obj");
        assert_eq!(instant(obj), 1_700_000_000_123);
    }

    #[test]
    fn instant_missing_returns_zero() {
        let v = json!({});
        let obj = v.as_object().expect("obj");
        assert_eq!(instant(obj), 0);
    }

    #[test]
    fn dt_seconds_drops_microseconds() {
        assert_eq!(
            f_dt_seconds("2026-06-08 08:52:13.269486"),
            "2026-06-08 08:52:13"
        );
    }

    #[test]
    fn dt_seconds_short_string_returned_as_is() {
        assert_eq!(f_dt_seconds("2026-06-08"), "2026-06-08");
        assert_eq!(f_dt_seconds(""), "");
    }

    #[test]
    fn get_key_value_accepts_non_empty() {
        let setting = json!({"key": "foobar"});
        assert_eq!(f_get_key_value(&setting, "key"), Some("foobar"));
    }

    #[test]
    fn get_key_value_rejects_empty_and_whitespace() {
        let setting = json!({"empty": "", "blank": "   ", "tab": "\t\n"});
        assert_eq!(f_get_key_value(&setting, "empty"), None);
        assert_eq!(f_get_key_value(&setting, "blank"), None);
        assert_eq!(f_get_key_value(&setting, "tab"), None);
    }

    #[test]
    fn target_key_rejects_whitespace_key() {
        let mut obj = Object::with_capacity(1);
        obj.insert("foobar", "FOOBAR");
        let setting = json!({"key": "  ", "targetKey": "out"});
        assert_eq!(f_target_key(&obj, &setting), Err("no_key"));
    }

    #[test]
    fn target_key_happy_path() {
        let mut obj = Object::with_capacity(1);
        obj.insert("foobar", "FOOBAR");
        let setting = json!({"key": "foobar", "targetKey": "out"});
        let (target, value) = f_target_key(&obj, &setting).expect("valid setting");
        assert_eq!(target, "out");
        assert_eq!(value.as_str(), Some("FOOBAR"));
    }
}

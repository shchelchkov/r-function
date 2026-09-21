use crate::db::error::DatabaseError;
use std::ops::RangeInclusive;

pub(crate) const SEP: u8 = 0;

const HISTORY_TAIL: usize = 16;

pub(crate) fn validate(setting_code: &str, key: &str) -> Result<(), DatabaseError> {
    if setting_code.is_empty() {
        return Err(DatabaseError::InvalidKey(
            "setting_code must not be empty".into(),
        ));
    }
    if setting_code.as_bytes().contains(&SEP) {
        return Err(DatabaseError::InvalidKey(format!(
            "setting_code contains NUL: {setting_code:?}"
        )));
    }
    if key.as_bytes().contains(&SEP) {
        return Err(DatabaseError::InvalidKey(format!(
            "key contains NUL: {key:?}"
        )));
    }
    Ok(())
}

pub(crate) fn value_prefix(setting_code: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(setting_code.len() + 1);
    out.extend_from_slice(setting_code.as_bytes());
    out.push(SEP);
    out
}

pub(crate) fn value_key(setting_code: &str, key: &str) -> Vec<u8> {
    let mut out = value_prefix(setting_code);
    out.extend_from_slice(key.as_bytes());
    out
}

pub(crate) fn decode_value_key(raw: &[u8]) -> Result<(&str, &str), DatabaseError> {
    let sep = raw
        .iter()
        .position(|b| *b == SEP)
        .ok_or_else(|| DatabaseError::Corrupt(format!("value key without separator: {raw:?}")))?;
    let setting_code = std::str::from_utf8(&raw[..sep])?;
    let key = std::str::from_utf8(&raw[sep + 1..])?;
    Ok((setting_code, key))
}

pub(crate) fn history_prefix(setting_code: &str, key: &str) -> Vec<u8> {
    let mut out = value_key(setting_code, key);
    out.push(SEP);
    out
}

pub(crate) fn history_key(prefix: &[u8], timestamp: u64, sequence: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(prefix.len() + HISTORY_TAIL);
    out.extend_from_slice(prefix);
    out.extend_from_slice(&timestamp.to_be_bytes());
    out.extend_from_slice(&sequence.to_be_bytes());
    out
}

pub(crate) fn history_range(prefix: &[u8], from: u64, to: u64) -> RangeInclusive<Vec<u8>> {
    history_key(prefix, from, u64::MIN)..=history_key(prefix, to, u64::MAX)
}

pub(crate) fn decode_history_key(raw: &[u8]) -> Result<(u64, u64), DatabaseError> {
    let corrupt = || DatabaseError::Corrupt(format!("history key too short: {raw:?}"));
    let tail = raw
        .len()
        .checked_sub(HISTORY_TAIL)
        .map(|at| &raw[at..])
        .ok_or_else(corrupt)?;
    let (ts, seq) = tail.split_at(8);
    let ts = u64::from_be_bytes(ts.try_into().map_err(|_| corrupt())?);
    let seq = u64::from_be_bytes(seq.try_into().map_err(|_| corrupt())?);
    Ok((ts, seq))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_key_is_unambiguous() {
        assert_ne!(value_key("a.b", "c"), value_key("a", "b.c"));
        assert_eq!(value_key("a", "b"), b"a\0b");
        assert_eq!(decode_value_key(b"a.b\0c").unwrap(), ("a.b", "c"));
        assert_eq!(decode_value_key(b"a\0b.c").unwrap(), ("a", "b.c"));
    }

    #[test]
    fn value_prefix_includes_separator() {
        let prefix = value_prefix("a");
        assert!(value_key("a", "x").starts_with(&prefix));
        assert!(!value_key("ab", "x").starts_with(&prefix));
    }

    #[test]
    fn validate_rejects_nul_and_empty_setting_code() {
        assert!(matches!(
            validate("", "k"),
            Err(DatabaseError::InvalidKey(_))
        ));
        assert!(matches!(
            validate("a\0b", "k"),
            Err(DatabaseError::InvalidKey(_))
        ));
        assert!(matches!(
            validate("a", "k\0"),
            Err(DatabaseError::InvalidKey(_))
        ));
        assert!(validate("a", "").is_ok());
        assert!(validate("a.b", "c.d").is_ok());
    }

    #[test]
    fn history_keys_sort_by_timestamp_then_sequence() {
        let p = history_prefix("s", "k");
        let a = history_key(&p, 1, u64::MAX);
        let b = history_key(&p, 2, 0);
        let c = history_key(&p, 2, 1);
        assert!(a < b && b < c);
        assert_eq!(decode_history_key(&c).unwrap(), (2, 1));
    }

    #[test]
    fn history_range_is_inclusive_on_both_ends() {
        let p = history_prefix("s", "k");
        let range = history_range(&p, 10, 20);
        assert!(range.contains(&history_key(&p, 10, 0)));
        assert!(range.contains(&history_key(&p, 20, u64::MAX)));
        assert!(!range.contains(&history_key(&p, 9, u64::MAX)));
        assert!(!range.contains(&history_key(&p, 21, 0)));
    }

    #[test]
    fn decode_history_key_rejects_short_keys() {
        assert!(matches!(
            decode_history_key(b"s\0k\0short"),
            Err(DatabaseError::Corrupt(_))
        ));
    }
}

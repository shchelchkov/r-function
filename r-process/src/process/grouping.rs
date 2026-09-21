use indexmap::IndexMap;
use r_setting::functions::function_setting::FunctionSetting;
use sonic_rs::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::process::Message;
use crate::process::resolver::setting_code_from_raw;

#[derive(Clone)]
pub(crate) enum Resolved {
    Ready(Arc<Vec<FunctionSetting>>, Arc<Vec<String>>),
    Skip,
    Failed,
}

pub(crate) struct Group {
    pub(crate) settings: Arc<Vec<FunctionSetting>>,
    pub(crate) batch: Vec<Value>,
    pub(crate) idxs: Vec<usize>,
    pub(crate) out_key: Option<Vec<u8>>,
}

pub(crate) struct Grouped {
    pub(crate) results: Vec<Result<(), ()>>,
    pub(crate) groups: Vec<(String, Group)>,
    pub(crate) poison: Vec<(usize, String)>,
}

pub(crate) fn group_messages(
    msgs: &mut [Message],
    resolved: &HashMap<String, Resolved>,
) -> Grouped {
    let mut results: Vec<Result<(), ()>> = vec![Ok(()); msgs.len()];
    let mut by_key: IndexMap<(String, Arc<str>), Group> = IndexMap::new();
    let mut poison: Vec<(usize, String)> = Vec::new();

    for (idx, msg) in msgs.iter_mut().enumerate() {
        let parsed = msg.payload.take().and_then(|mut p| p.pop());
        let raw = msg.raw.as_deref();

        let setting_code = match (&parsed, raw) {
            (Some(v), _) => v.setting_code.to_string(),
            (None, Some(raw)) => match setting_code_from_raw(raw) {
                Some(sc) => sc,
                None => continue,
            },
            (None, None) => continue,
        };

        let (settings, value_key) = match resolved.get(&setting_code) {
            Some(Resolved::Ready(s, k)) => (s.clone(), k.clone()),
            Some(Resolved::Skip) | None => continue,
            Some(Resolved::Failed) => {
                results[idx] = Err(());
                continue;
            }
        };

        let (value, key_value) = match (parsed, raw) {
            (Some(v), _) => (v.value, v.key),
            (None, Some(raw)) => match value_rs::parse_and_build_key(raw, &value_key) {
                Ok(Some(vk)) => vk,
                Ok(None) => continue,
                Err(e) => {
                    poison.push((idx, e.to_string()));
                    continue;
                }
            },
            (None, None) => continue,
        };

        let entry = by_key
            .entry((setting_code, key_value))
            .or_insert_with(|| Group {
                settings,
                batch: Vec::new(),
                idxs: Vec::new(),
                out_key: None,
            });
        entry.batch.push(value);
        entry.idxs.push(idx);
        if entry.out_key.is_none() {
            entry.out_key = msg.key.clone();
        }
    }

    let groups = by_key.into_iter().map(|((sc, _kv), g)| (sc, g)).collect();
    Grouped {
        results,
        groups,
        poison,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn msg(payload: &str, key: Option<&str>) -> Message {
        Message::new(
            "src-topic".into(),
            0,
            0,
            key.map(|k| k.as_bytes().to_vec()),
            Some(payload.as_bytes().to_vec()),
            HashMap::new(),
            None,
        )
    }

    fn ready() -> HashMap<String, Resolved> {
        let mut m = HashMap::new();
        m.insert(
            "sc".to_string(),
            Resolved::Ready(Arc::new(vec![]), Arc::new(vec!["topic".to_string()])),
        );
        m
    }

    #[test]
    fn groups_by_key_value() {
        let mut msgs = vec![
            msg(r#"{"setting_code":"sc","topic":"t1"}"#, Some("t1")),
            msg(r#"{"setting_code":"sc","topic":"t1"}"#, Some("t1")),
            msg(r#"{"setting_code":"sc","topic":"t2"}"#, Some("t2")),
        ];
        let g = group_messages(&mut msgs, &ready());

        assert_eq!(g.groups.len(), 2, "two distinct topics -> two groups");
        let mut sizes: Vec<usize> = g.groups.iter().map(|(_, grp)| grp.batch.len()).collect();
        sizes.sort_unstable();
        assert_eq!(sizes, vec![1, 2], "t1 has 2 messages, t2 has 1");
        assert!(g.results.iter().all(|r| r.is_ok()));
        assert!(g.poison.is_empty());
    }

    #[test]
    fn missing_setting_code_is_dropped() {
        let mut msgs = vec![msg(r#"{"topic":"t1"}"#, None)];
        let g = group_messages(&mut msgs, &ready());
        assert!(g.groups.is_empty());
        assert!(g.poison.is_empty());
        assert_eq!(g.results[0], Ok(()), "dropped -> commit");
    }

    #[test]
    fn failed_resolution_holds_offset() {
        let mut resolved = HashMap::new();
        resolved.insert("sc".to_string(), Resolved::Failed);
        let mut msgs = vec![msg(r#"{"setting_code":"sc","topic":"t1"}"#, None)];
        let g = group_messages(&mut msgs, &resolved);
        assert!(g.groups.is_empty());
        assert_eq!(g.results[0], Err(()), "transient -> hold for redelivery");
    }

    #[test]
    fn unparseable_payload_is_poison() {
        let mut msgs = vec![msg(r#"{"setting_code":"sc","topic":"t1",}"#, None)];
        let g = group_messages(&mut msgs, &ready());
        assert!(g.groups.is_empty(), "poison must not be grouped");
        assert_eq!(g.poison.len(), 1);
        assert_eq!(g.poison[0].0, 0);
    }

    #[test]
    fn parsed_payload_is_grouped_without_reparsing_raw() {
        let mut m = msg("{not json", Some("t1"));
        m.payload = Some(vec![value_rs::Value {
            value_key: vec!["topic".to_string()],
            setting_code: "sc".into(),
            key: Arc::from("t1"),
            value: sonic_rs::from_str(r#"{"setting_code":"sc","topic":"t1"}"#).unwrap(),
        }]);
        let mut msgs = vec![m];
        let g = group_messages(&mut msgs, &ready());

        assert!(
            g.poison.is_empty(),
            "raw must not be reparsed when payload is set"
        );
        assert_eq!(g.groups.len(), 1);
        assert_eq!(g.groups[0].0, "sc");
        assert_eq!(g.groups[0].1.batch.len(), 1);
        assert!(
            msgs[0].payload.is_none(),
            "parsed value is moved into the group"
        );
        assert!(msgs[0].raw.is_some(), "raw stays for DLQ");
    }

    #[test]
    fn out_key_is_taken_from_messages() {
        let mut msgs = vec![msg(r#"{"setting_code":"sc","topic":"t1"}"#, Some("t1"))];
        let g = group_messages(&mut msgs, &ready());
        let (sc, grp) = &g.groups[0];
        assert_eq!(sc, "sc");
        assert_eq!(grp.out_key.as_deref(), Some(&b"t1"[..]));
    }
}

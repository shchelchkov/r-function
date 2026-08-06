use r_setting::functions::function_setting::FunctionSetting;
use sonic_rs::{JsonValueTrait, Value};
use std::collections::HashMap;
use std::sync::Arc;

use crate::process::Message;
use crate::process::key_value_wrapper;

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

pub(crate) fn group_messages(msgs: &[Message], resolved: &HashMap<String, Resolved>) -> Grouped {
    let mut results: Vec<Result<(), ()>> = vec![Ok(()); msgs.len()];
    let mut by_key: HashMap<(String, Arc<str>), Group> = HashMap::new();
    let mut poison: Vec<(usize, String)> = Vec::new();

    for (idx, msg) in msgs.iter().enumerate() {
        let Some(raw) = msg.payload.as_deref() else {
            continue; 
        };
        let Some(setting_code) = sonic_rs::get_from_slice(raw, &["setting_code"])
            .as_str()
            .map(str::to_owned)
        else {
            continue;
        };

        let (settings, value_key) = match resolved.get(&setting_code) {
            Some(Resolved::Ready(s, k)) => (s.clone(), k.clone()),
            Some(Resolved::Skip) | None => continue,
            Some(Resolved::Failed) => {
                results[idx] = Err(()); 
                continue;
            }
        };

        match key_value_wrapper::parse_and_build_key(raw, &value_key) {
            Ok(Some((value, key_value))) => {
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
            Ok(None) => continue, 
            Err(e) => poison.push((idx, e.to_string())),
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
        let msgs = vec![
            msg(r#"{"setting_code":"sc","topic":"t1"}"#, Some("t1")),
            msg(r#"{"setting_code":"sc","topic":"t1"}"#, Some("t1")),
            msg(r#"{"setting_code":"sc","topic":"t2"}"#, Some("t2")),
        ];
        let g = group_messages(&msgs, &ready());

        assert_eq!(g.groups.len(), 2, "two distinct topics -> two groups");
        let mut sizes: Vec<usize> = g.groups.iter().map(|(_, grp)| grp.batch.len()).collect();
        sizes.sort_unstable();
        assert_eq!(sizes, vec![1, 2], "t1 has 2 messages, t2 has 1");
        assert!(g.results.iter().all(|r| r.is_ok()));
        assert!(g.poison.is_empty());
    }

    #[test]
    fn missing_setting_code_is_dropped() {
        let msgs = vec![msg(r#"{"topic":"t1"}"#, None)];
        let g = group_messages(&msgs, &ready());
        assert!(g.groups.is_empty());
        assert!(g.poison.is_empty());
        assert_eq!(g.results[0], Ok(()), "dropped -> commit");
    }

    #[test]
    fn failed_resolution_holds_offset() {
        let mut resolved = HashMap::new();
        resolved.insert("sc".to_string(), Resolved::Failed);
        let msgs = vec![msg(r#"{"setting_code":"sc","topic":"t1"}"#, None)];
        let g = group_messages(&msgs, &resolved);
        assert!(g.groups.is_empty());
        assert_eq!(g.results[0], Err(()), "transient -> hold for redelivery");
    }

    #[test]
    fn unparseable_payload_is_poison() {
        let msgs = vec![msg(r#"{"setting_code":"sc","topic":"t1",}"#, None)];
        let g = group_messages(&msgs, &ready());
        assert!(g.groups.is_empty(), "poison must not be grouped");
        assert_eq!(g.poison.len(), 1);
        assert_eq!(g.poison[0].0, 0);
    }

    #[test]
    fn out_key_is_taken_from_messages() {
        let msgs = vec![msg(r#"{"setting_code":"sc","topic":"t1"}"#, Some("t1"))];
        let g = group_messages(&msgs, &ready());
        let (sc, grp) = &g.groups[0];
        assert_eq!(sc, "sc");
        assert_eq!(grp.out_key.as_deref(), Some(&b"t1"[..]));
    }
}

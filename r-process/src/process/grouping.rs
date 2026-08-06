use indexmap::IndexMap;
use r_setting::functions::function_setting::FunctionSetting;
use sonic_rs::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::process::Message;

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
}

pub(crate) fn group_messages(
    msgs: &mut [Message],
    resolved: &HashMap<String, Resolved>,
) -> Grouped {
    let mut results: Vec<Result<(), ()>> = vec![Ok(()); msgs.len()];
    let mut by_key: IndexMap<(String, Arc<str>), Group> = IndexMap::new();

    for (idx, msg) in msgs.iter_mut().enumerate() {
        let Some(v) = msg.payload.take().and_then(|mut p| p.pop()) else {
            continue;
        };
        let setting_code = v.setting_code.to_string();

        let settings = match resolved.get(&setting_code) {
            Some(Resolved::Ready(s, _)) => s.clone(),
            Some(Resolved::Skip) | None => continue,
            Some(Resolved::Failed) => {
                results[idx] = Err(());
                continue;
            }
        };

        let entry = by_key
            .entry((setting_code, v.key))
            .or_insert_with(|| Group {
                settings,
                batch: Vec::new(),
                idxs: Vec::new(),
                out_key: None,
            });
        entry.batch.push(v.value);
        entry.idxs.push(idx);
        if entry.out_key.is_none() {
            entry.out_key = msg.key.clone();
        }
    }

    let groups = by_key.into_iter().map(|((sc, _kv), g)| (sc, g)).collect();
    Grouped { results, groups }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn message(payload: Option<Vec<value_rs::Value>>, key: Option<&str>) -> Message {
        Message::new(
            "src-topic".into(),
            0,
            0,
            key.map(|k| k.as_bytes().to_vec()),
            payload,
            HashMap::new(),
            None,
        )
    }

    fn msg(json: &str, key: Option<&str>) -> Message {
        let payload =
            value_rs::from_slice_and_build_key(json.as_bytes(), &["topic".to_string()], "sc")
                .unwrap()
                .map(|v| vec![v]);
        message(payload, key)
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
    }

    #[test]
    fn message_without_payload_is_skipped() {
        let mut msgs = vec![message(None, None)];
        let g = group_messages(&mut msgs, &ready());
        assert!(g.groups.is_empty(), "nothing parsed -> nothing to group");
        assert_eq!(g.results[0], Ok(()), "skipped -> commit");
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
    fn parsed_payload_is_moved_into_group() {
        let m = message(
            Some(vec![value_rs::Value {
                value_key: vec!["topic".to_string()],
                setting_code: "sc".into(),
                key: Arc::from("t1"),
                value: sonic_rs::from_str(r#"{"setting_code":"sc","topic":"t1"}"#).unwrap(),
            }]),
            Some("t1"),
        );
        let mut msgs = vec![m];
        let g = group_messages(&mut msgs, &ready());

        assert_eq!(g.groups.len(), 1);
        assert_eq!(g.groups[0].0, "sc");
        assert_eq!(g.groups[0].1.batch.len(), 1);
        assert!(
            msgs[0].payload.is_none(),
            "parsed value is moved into the group"
        );
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

use crate::host::{ctx_setting_code_key, ctx_u64};
use crate::plugin::PluginContext;
use r_db::db::db::Database;
use r_db::db::history::HistoryQuery;
use r_plugin_api::Buffer;
use std::ffi::c_void;

fn into_buffer(mut bytes: Vec<u8>) -> Buffer {
    let buffer = Buffer {
        ptr: bytes.as_mut_ptr(),
        len: bytes.len(),
        capacity: bytes.capacity(),
    };
    std::mem::forget(bytes);
    buffer
}

pub(crate) fn db_get_value(db: &Database, setting_code: &str, key: &str) -> Buffer {
    match db.get_value(setting_code, key) {
        Ok(Some(values)) => match sonic_rs::to_vec(&*values) {
            Ok(json) => into_buffer(json),
            Err(error) => {
                tracing::error!(%error, %setting_code, %key, "plugin db_get_value: encode");
                Buffer::empty()
            }
        },
        Ok(None) => Buffer::empty(),
        Err(error) => {
            tracing::error!(%error, %setting_code, %key, "plugin db_get_value failed");
            Buffer::empty()
        }
    }
}

pub(crate) fn db_get_history(
    db: &Database,
    setting_code: &str,
    key: &str,
    query: HistoryQuery,
) -> Buffer {
    match db
        .get_history(setting_code, key, query)
        .and_then(|entries| Ok(sonic_rs::to_vec(&entries)?))
    {
        Ok(json) => into_buffer(json),
        Err(error) => {
            tracing::error!(%error, %setting_code, %key, "plugin db_get_history failed");
            Buffer::empty()
        }
    }
}

pub(crate) fn db_remove_value(db: &Database, setting_code: &str, key: &str) -> bool {
    match db.remove_value(setting_code, key) {
        Ok(removed) => removed,
        Err(error) => {
            tracing::error!(%error, %setting_code, %key, "plugin db_remove_value failed");
            false
        }
    }
}

pub unsafe extern "C" fn host_db_get_value(
    ctx: *mut c_void,
    setting_code_ptr: *const u8,
    setting_code_len: usize,
    key_ptr: *const u8,
    key_len: usize,
) -> Buffer {
    let Some(ctx) = (unsafe { (ctx as *const PluginContext).as_ref() }) else {
        return Buffer::empty();
    };

    let Some((setting_code, key)) =
        (unsafe { ctx_setting_code_key(setting_code_ptr, setting_code_len, key_ptr, key_len) })
    else {
        return Buffer::empty();
    };

    db_get_value(&ctx.db, &setting_code, &key)
}

#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn host_db_get_history(
    ctx: *mut c_void,
    setting_code_ptr: *const u8,
    setting_code_len: usize,
    key_ptr: *const u8,
    key_len: usize,
    from_ptr: *const u8,
    from_len: usize,
    to_ptr: *const u8,
    to_len: usize,
    limit: usize,
    newest_first: bool,
) -> Buffer {
    let Some(ctx) = (unsafe { (ctx as *const PluginContext).as_ref() }) else {
        return Buffer::empty();
    };

    let Some((setting_code, key)) =
        (unsafe { ctx_setting_code_key(setting_code_ptr, setting_code_len, key_ptr, key_len) })
    else {
        return Buffer::empty();
    };

    let bound = |ptr: *const u8, len: usize| -> Option<Option<u64>> {
        if len == 0 {
            return Some(None);
        }
        unsafe { ctx_u64(ptr, len) }.map(Some)
    };
    let (Some(from), Some(to)) = (bound(from_ptr, from_len), bound(to_ptr, to_len)) else {
        tracing::error!(%setting_code, %key, "plugin db_get_history: bound is not 8 LE bytes");
        return Buffer::empty();
    };

    let query = HistoryQuery {
        from,
        to,
        limit: (limit > 0).then_some(limit),
        newest_first,
    };

    db_get_history(&ctx.db, &setting_code, &key, query)
}

pub unsafe extern "C" fn host_db_remove_value(
    ctx: *mut c_void,
    setting_code_ptr: *const u8,
    setting_code_len: usize,
    key_ptr: *const u8,
    key_len: usize,
) -> bool {
    let Some(ctx) = (unsafe { (ctx as *const PluginContext).as_ref() }) else {
        return false;
    };

    let Some((setting_code, key)) =
        (unsafe { ctx_setting_code_key(setting_code_ptr, setting_code_len, key_ptr, key_len) })
    else {
        return false;
    };

    db_remove_value(&ctx.db, &setting_code, &key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use r_db::db::db::DatabaseOptions;
    use sonic_rs::Value;

    fn open() -> (tempfile::TempDir, Database) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(
            dir.path(),
            DatabaseOptions {
                limit: 4,
                history: 100,
            },
        )
        .unwrap();
        (dir, db)
    }

    fn val(s: &str) -> Value {
        sonic_rs::from_str(s).unwrap()
    }

        fn take(buffer: Buffer) -> Option<String> {
        if buffer.ptr.is_null() {
            return None;
        }
        let bytes = unsafe { Vec::from_raw_parts(buffer.ptr, buffer.len, buffer.capacity) };
        Some(String::from_utf8(bytes).unwrap())
    }

    #[test]
    fn db_get_value_returns_list_or_empty_buffer() {
        let (_dir, db) = open();
        db.insert_value("sc", "k", val("1")).unwrap();
        db.insert_value("sc", "k", val("2")).unwrap();

        assert_eq!(take(db_get_value(&db, "sc", "k")).as_deref(), Some("[2,1]"));
        assert!(take(db_get_value(&db, "sc", "missing")).is_none());
        assert!(take(db_get_value(&db, "", "k")).is_none());
    }

    #[test]
    fn db_get_history_honours_query() {
        let (_dir, db) = open();
        for ts in 1..=5u64 {
            db.append_value("sc", "k", ts, val(&ts.to_string()))
                .unwrap();
        }

        let all = take(db_get_history(&db, "sc", "k", HistoryQuery::default())).unwrap();
        assert_eq!(
            all,
            r#"[{"timestamp":1,"value":1},{"timestamp":2,"value":2},{"timestamp":3,"value":3},{"timestamp":4,"value":4},{"timestamp":5,"value":5}]"#
        );

        let newest = take(db_get_history(
            &db,
            "sc",
            "k",
            HistoryQuery {
                from: Some(2),
                to: Some(4),
                limit: Some(2),
                newest_first: true,
            },
        ))
        .unwrap();
        assert_eq!(
            newest,
            r#"[{"timestamp":4,"value":4},{"timestamp":3,"value":3}]"#
        );

        assert_eq!(
            take(db_get_history(&db, "sc", "none", HistoryQuery::default())).as_deref(),
            Some("[]")
        );
    }

    #[test]
    fn db_remove_value_reports_presence() {
        let (_dir, db) = open();
        db.insert_value("sc", "k", val("1")).unwrap();

        assert!(db_remove_value(&db, "sc", "k"));
        assert!(!db_remove_value(&db, "sc", "k"));
        assert!(!db_remove_value(&db, "", "k"));
    }
}

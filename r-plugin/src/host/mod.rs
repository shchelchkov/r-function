use crate::host::append_value::host_append_value;
use crate::host::contains_point::host_contains_point;
use crate::host::contains_polygon::host_contains_polygon;
use crate::host::free_buffer::host_free_buffer;
use crate::host::get_function_setting::host_get_function_setting;
use crate::host::get_function_value::host_get_function_value;
use crate::host::get_value::host_get_value;
use crate::host::insert_value::host_insert_value;
use crate::host::put_polygon::host_put_polygon;
use crate::host::put_value::host_put_value;
use crate::host::remove_polygon::host_remove_polygon;
use crate::host::send_value::host_send_value;
use crate::plugin::PluginContext;
use r_plugin_api::HostApi;
use sonic_rs::Value;
use std::ffi::c_void;

mod append_value;
mod contains_point;
mod contains_polygon;
mod free_buffer;
mod get_function_setting;
mod get_function_value;
mod get_value;
mod insert_value;
mod put_polygon;
mod put_value;
mod remove_polygon;
mod send_value;

pub fn create_api(context: &PluginContext) -> HostApi {
    HostApi {
        ctx: context as *const PluginContext as *mut c_void,
        put_value: host_put_value,
        insert_value: host_insert_value,
        append_value: host_append_value,
        get_value: host_get_value,
        send_value: host_send_value,
        contains_point: host_contains_point,
        contains_polygon: host_contains_polygon,
        put_polygon: host_put_polygon,
        remove_polygon: host_remove_polygon,
        get_function_value: host_get_function_value,
        get_function_setting: host_get_function_setting,
        free_buffer: host_free_buffer,
    }
}

pub(crate) unsafe fn ctx_setting_code_key(
    setting_code_ptr: *const u8,
    setting_code_len: usize,
    key_ptr: *const u8,
    key_len: usize,
) -> Option<(String, String)> {
    let setting_code = unsafe { slice_from_raw_parts(setting_code_ptr, setting_code_len) }?;
    let key = unsafe { slice_from_raw_parts(key_ptr, key_len) }?;

    Some((
        String::from_utf8_lossy(setting_code).into_owned(),
        String::from_utf8_lossy(key).into_owned(),
    ))
}

pub(crate) unsafe fn ctx_setting_code(
    setting_code_ptr: *const u8,
    setting_code_len: usize,
) -> Option<String> {
    let setting_code = unsafe { slice_from_raw_parts(setting_code_ptr, setting_code_len) }?;

    Some(String::from_utf8_lossy(setting_code).into_owned())
}

pub(crate) unsafe fn ctx_value(value_ptr: *const u8, value_len: usize) -> Option<Value> {
    let value = unsafe { slice_from_raw_parts(value_ptr, value_len) }?;

    sonic_rs::from_slice(value).ok()
}

pub(crate) unsafe fn ctx_u64(value_ptr: *const u8, value_len: usize) -> Option<u64> {
    let bytes = unsafe { slice_from_raw_parts(value_ptr, value_len) }?;
    let bytes: [u8; 8] = bytes.try_into().ok()?;

    Some(u64::from_le_bytes(bytes))
}

pub(crate) unsafe fn slice_from_raw_parts<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    if len == 0 {
        return Some(&[]);
    }

    if ptr.is_null() {
        return None;
    }

    Some(unsafe { std::slice::from_raw_parts(ptr, len) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_rejects_null_with_non_zero_len() {
        assert!(unsafe { slice_from_raw_parts(std::ptr::null(), 8) }.is_none());
    }

    #[test]
    fn slice_accepts_zero_len() {
        assert_eq!(
            unsafe { slice_from_raw_parts(std::ptr::null(), 0) },
            Some(&b""[..])
        );

        let buf = b"abc";
        assert_eq!(
            unsafe { slice_from_raw_parts(buf.as_ptr(), 0) },
            Some(&b""[..])
        );
    }

    #[test]
    fn slice_reads_valid_buffer() {
        let buf = b"abc";

        assert_eq!(
            unsafe { slice_from_raw_parts(buf.as_ptr(), buf.len()) },
            Some(&b"abc"[..])
        );
    }

    #[test]
    fn setting_code_key_reads_both() {
        let code = b"code";
        let key = b"key";

        let got =
            unsafe { ctx_setting_code_key(code.as_ptr(), code.len(), key.as_ptr(), key.len()) };

        assert_eq!(got, Some(("code".to_owned(), "key".to_owned())));
    }

    #[test]
    fn setting_code_key_rejects_null_key() {
        let code = b"code";

        let got = unsafe { ctx_setting_code_key(code.as_ptr(), code.len(), std::ptr::null(), 3) };

        assert!(got.is_none());
    }

    #[test]
    fn setting_code_key_is_lossy_on_invalid_utf8() {
        let code = [0xff_u8, 0xfe];
        let key = b"key";

        let (code, _) =
            unsafe { ctx_setting_code_key(code.as_ptr(), code.len(), key.as_ptr(), key.len()) }
                .expect("lossy conversion must not fail");

        assert_eq!(code, "\u{fffd}\u{fffd}");
    }

    #[test]
    fn u64_requires_exactly_eight_bytes() {
        let eight = 42_u64.to_le_bytes();
        assert_eq!(unsafe { ctx_u64(eight.as_ptr(), eight.len()) }, Some(42));

        let seven = [0_u8; 7];
        assert_eq!(unsafe { ctx_u64(seven.as_ptr(), seven.len()) }, None);

        let as_string = b"1737000000000";
        assert_eq!(
            unsafe { ctx_u64(as_string.as_ptr(), as_string.len()) },
            None
        );

        assert_eq!(unsafe { ctx_u64(std::ptr::null(), 8) }, None);
    }

    #[test]
    fn value_parses_json_and_rejects_the_rest() {
        let ok = br#"{"a":1}"#;
        assert!(unsafe { ctx_value(ok.as_ptr(), ok.len()) }.is_some());

        let malformed = b"{not json";
        assert!(unsafe { ctx_value(malformed.as_ptr(), malformed.len()) }.is_none());

        let empty = b"";
        assert!(unsafe { ctx_value(empty.as_ptr(), 0) }.is_none());

        assert!(unsafe { ctx_value(std::ptr::null(), 4) }.is_none());
    }
}

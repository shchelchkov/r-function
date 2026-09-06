use f_common::fun;
use plugin_common::buffer::{empty_buffer, into_buffer};
use plugin_common::order_book_bybit;
use r_plugin_api::{Buffer, HostApi};
use sonic_rs::{JsonContainerTrait, Value};

pub unsafe fn process(api: *const HostApi, input_ptr: *const u8, input_len: usize) -> Buffer {
    let Some(api) = (unsafe { api.as_ref() }) else {
        return empty_buffer();
    };
    if input_ptr.is_null() && input_len != 0 {
        return empty_buffer();
    }
    let input = unsafe { std::slice::from_raw_parts(input_ptr, input_len) };
    let mut input: Value = fun::from_slice(input);

    let mut v: Vec<&Value> = Vec::new();;
    if let Some(values) = input.as_array() {
        for value in values.iter() {
            if let Some(obj) = value.as_object() {
                if let Ok(processed_value) = order_book_bybit::emit_obi(api, obj) {
                    v.push(value);
                }
            }
        }
    } else if let Some(obj) = input.as_object() {
        if let Ok(processed_value) = order_book_bybit::emit_obi(api, obj) {
            v.push(&input);
        }
    }

    let output = fun::to_vec(&v);
    into_buffer(output)
}




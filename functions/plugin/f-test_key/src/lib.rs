use r_plugin_api::{Buffer, HostApi};

mod process;


#[unsafe(no_mangle)]
pub unsafe extern "C" fn process(
    api: *const HostApi,
    input_ptr: *const u8,
    input_len: usize,
) -> Buffer {
    unsafe {
        process::process(
            api,
            input_ptr,
            input_len,
        )
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn free_buffer(
    buffer: Buffer,
) {
    unsafe {
        plugin_common::buffer::free(buffer);
    }
}
use r_plugin_api::Buffer;

pub unsafe extern "C" fn host_free_buffer(
    buffer: Buffer,
) {
    if buffer.ptr.is_null() {
        return;
    }

    unsafe {
        let _ = Vec::from_raw_parts(
            buffer.ptr,
            buffer.len,
            buffer.capacity,
        );
    }
}
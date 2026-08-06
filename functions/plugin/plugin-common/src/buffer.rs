use r_plugin_api::Buffer;

pub fn empty_buffer() -> Buffer {
    Buffer {
        ptr: std::ptr::null_mut(),
        len: 0,
        capacity: 0,
    }
}

pub fn into_buffer(
    mut bytes: Vec<u8>,
) -> Buffer {
    let buffer = Buffer {
        ptr: bytes.as_mut_ptr(),
        len: bytes.len(),
        capacity: bytes.capacity(),
    };

    std::mem::forget(bytes);

    buffer
}

pub unsafe fn free(
    buffer: Buffer,
) {
    if buffer.ptr.is_null() {
        return;
    }

    unsafe {
        drop(Vec::from_raw_parts(
            buffer.ptr,
            buffer.len,
            buffer.capacity,
        ));
    }
}
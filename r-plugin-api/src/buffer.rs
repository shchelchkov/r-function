#[repr(C)]
pub struct Buffer {
    pub ptr: *mut u8,
    pub len: usize,
    pub capacity: usize,
}

impl Buffer {
    pub fn empty() -> Buffer {
        Buffer {
            ptr: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        }
    }
}

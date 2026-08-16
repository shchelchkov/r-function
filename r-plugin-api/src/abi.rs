use crate::Buffer;
use std::ffi::c_void;
use sonic_rs::Value;

unsafe impl Send for HostApi {}
unsafe impl Sync for HostApi {}

#[repr(C)]
pub struct HostApi {
    pub ctx: *mut c_void,

    pub put_value: unsafe extern "C" fn(
        ctx: *mut c_void,

        setting_code_ptr: *const u8,
        setting_code_len: usize,

        key_ptr: *const u8,
        key_len: usize,

        value_ptr: *const u8,
        value_len: usize,
    ),

    pub get_value: unsafe extern "C" fn(
        ctx: *mut c_void,

        setting_code_ptr: *const u8,
        setting_code_len: usize,

        key_ptr: *const u8,
        key_len: usize,
    ) -> Buffer,

    pub send_value: unsafe extern "C" fn(
        ctx: *mut c_void,

        setting_code_ptr: *const u8,
        setting_code_len: usize,

        key_ptr: *const u8,
        key_len: usize,

        channel_ptr: *const u8,
        channel_len: usize,

        payload_ptr: *const u8,
        payload_len: usize,
    ) -> Buffer,

    pub free_buffer: unsafe extern "C" fn(
        buffer: Buffer,
    ),
}


impl HostApi {

    pub fn put_value(
        &self,
        setting_code: &str,
        key: &str,
        value: &Value,
    ) -> Result<(), sonic_rs::Error> {
        let bytes = sonic_rs::to_vec(value)?;

        unsafe {
            (self.put_value)(
                self.ctx,

                setting_code.as_ptr(),
                setting_code.len(),

                key.as_ptr(),
                key.len(),

                bytes.as_ptr(),
                bytes.len(),
            );
        }

        Ok(())
    }

    pub fn get_value(
        &self,
        setting_code: &str,
        key: &str,
    ) -> Option<Value> {
        let buffer = unsafe {
            (self.get_value)(
                self.ctx,

                setting_code.as_ptr(),
                setting_code.len(),

                key.as_ptr(),
                key.len(),
            )
        };

        if buffer.ptr.is_null() {
            return None;
        }

        let result = unsafe {
            let bytes = std::slice::from_raw_parts(
                buffer.ptr,
                buffer.len,
            );

            sonic_rs::from_slice(bytes).ok()
        };

        unsafe {
            (self.free_buffer)(buffer);
        }

        result
    }

    pub fn send_value(
        &self,
        setting_code: &str,
        key: Option<&[u8]>,
        channel: Option<&[u8]>,
        payload: &[u8],
    ) {
        let key = key.unwrap_or_default();
        let channel = channel.unwrap_or_default();

        unsafe {
            (self.send_value)(
                self.ctx,

                setting_code.as_ptr(),
                setting_code.len(),

                key.as_ptr(),
                key.len(),

                channel.as_ptr(),
                channel.len(),

                payload.as_ptr(),
                payload.len(),
            );
        }
    }
}
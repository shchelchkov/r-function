use crate::{Buffer, PluginError};
use sonic_rs::{JsonContainerTrait, JsonValueTrait, Object, Value};
use std::ffi::c_void;

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

    pub contains_point: unsafe extern "C" fn(
        ctx: *mut c_void,

        setting_code_ptr: *const u8,
        setting_code_len: usize,

        key_ptr: *const u8,
        key_len: usize,

        value_ptr: *const u8,
        value_len: usize,
    ) -> Buffer,

    pub contains_polygon: unsafe extern "C" fn(
        ctx: *mut c_void,

        setting_code_ptr: *const u8,
        setting_code_len: usize,

        key_ptr: *const u8,
        key_len: usize,

        value_ptr: *const u8,
        value_len: usize,
    ) -> Buffer,

    pub put_polygon: unsafe extern "C" fn(
        ctx: *mut c_void,

        setting_code_ptr: *const u8,
        setting_code_len: usize,

        key_ptr: *const u8,
        key_len: usize,

        value_ptr: *const u8,
        value_len: usize,
    ),

    pub remove_polygon: unsafe extern "C" fn(
        ctx: *mut c_void,

        setting_code_ptr: *const u8,
        setting_code_len: usize,

        key_ptr: *const u8,
        key_len: usize,

        value_ptr: *const u8,
        value_len: usize,
    ),


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

    pub get_function_value: unsafe extern "C" fn(
        ctx: *mut c_void,

        setting_code_ptr: *const u8,
        setting_code_len: usize,

        key_ptr: *const u8,
        key_len: usize,

    ) -> Buffer,

    pub get_function_setting: unsafe extern "C" fn(
        ctx: *mut c_void,

        setting_code_ptr: *const u8,
        setting_code_len: usize
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
    ) -> Option<Vec<Value>> {
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
    
    pub fn get_function_value(
        &self,
        setting_code: &str,
        key: &str,
    ) -> Option<Value> {
        let buffer = unsafe {
            (self.get_function_value)(
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
    pub fn get_function_setting(
        &self,
        setting_code: &str,
    ) -> Option<Vec<Value>> {
        let buffer = unsafe {
            (self.get_function_setting)(
                self.ctx,

                setting_code.as_ptr(),
                setting_code.len(),
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
        value: &Object
    ) -> Result<(), PluginError> {

        let setting_code: &str = value.get(&"setting_code").and_then(|v| v.as_str()).unwrap_or_default();
        let key:  &str = value.get(&"key").and_then(|v| v.as_str()).unwrap_or_default();
        let channel:  &str = value.get(&"channel").and_then(|v| v.as_str()).unwrap_or_default();
        let payload = sonic_rs::to_vec(&value)
            .map_err(|e| PluginError::Encode(e.to_string()))?;

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
        Ok(())
    }
}
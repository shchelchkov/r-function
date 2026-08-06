#[link(wasm_import_module = "env")]
unsafe extern "C" {
    #[link_name = "get_function_setting"]
    fn raw_get_function_setting(
        in_ptr: u32,
        in_len: u32,
        out_ptr_ptr: *mut u32,
        out_len_ptr: *mut u32,
    ) -> i32;

    #[link_name = "get_function_value"]
    fn raw_get_function_value(
        in_ptr: u32,
        in_len: u32,
        out_ptr_ptr: *mut u32,
        out_len_ptr: *mut u32,
    ) -> i32;

    #[link_name = "get_stream_setting"]
    fn raw_get_stream_setting(
        in_ptr: u32,
        in_len: u32,
        out_ptr_ptr: *mut u32,
        out_len_ptr: *mut u32,
    ) -> i32;

    #[link_name = "send_value"]
    fn raw_send_value(
        in_ptr: u32,
        in_len: u32,
        out_ptr_ptr: *mut u32,
        out_len_ptr: *mut u32,
    ) -> i32;

    #[link_name = "get_value"]
    fn raw_get_value(
        in_ptr: u32,
        in_len: u32,
        out_ptr_ptr: *mut u32,
        out_len_ptr: *mut u32,
    ) -> i32;

    #[link_name = "put_value"]
    fn raw_put_value(
        in_ptr: u32,
        in_len: u32,
        out_ptr_ptr: *mut u32,
        out_len_ptr: *mut u32,
    ) -> i32;

    #[link_name = "remove_value"]
    fn raw_remove_value(
        in_ptr: u32,
        in_len: u32,
        out_ptr_ptr: *mut u32,
        out_len_ptr: *mut u32,
    ) -> i32;

    #[link_name = "http_request"]
    fn raw_http_request(
        in_ptr: u32,
        in_len: u32,
        out_ptr_ptr: *mut u32,
        out_len_ptr: *mut u32,
    ) -> i32;
}

type HostFn = unsafe extern "C" fn(u32, u32, *mut u32, *mut u32) -> i32;

fn call_bytes(f: HostFn, input: &[u8]) -> Result<Option<Vec<u8>>, i32> {
    let mut out_ptr: u32 = 0;
    let mut out_len: u32 = 0;

    let rc = unsafe {
        f(
            input.as_ptr() as u32,
            input.len() as u32,
            &mut out_ptr,
            &mut out_len,
        )
    };

    match rc {
        0 => Ok(Some(unsafe {
            Vec::from_raw_parts(out_ptr as *mut u8, out_len as usize, out_len as usize)
        })),
        1 => Ok(None),
        err => Err(err),
    }
}

fn call_unit(f: HostFn, input: &[u8]) -> Result<(), i32> {
    let mut out_ptr: u32 = 0;
    let mut out_len: u32 = 0;

    let rc = unsafe {
        f(
            input.as_ptr() as u32,
            input.len() as u32,
            &mut out_ptr,
            &mut out_len,
        )
    };

    match rc {
        0 | 1 => Ok(()),
        err => Err(err),
    }
}

pub fn alloc(len: u32) -> u32 {
    let mut buf = Vec::<u8>::with_capacity(len as usize);
    let ptr = buf.as_mut_ptr() as u32;
    std::mem::forget(buf);
    ptr
}

pub fn get_function_setting(code: &str) -> Result<Option<Vec<u8>>, i32> {
    call_bytes(raw_get_function_setting, code.as_bytes())
}

pub fn get_function_value(req: &[u8]) -> Result<Option<Vec<u8>>, i32> {
    call_bytes(raw_get_function_value, req)
}

pub fn get_stream_setting(code: &str) -> Result<Option<Vec<u8>>, i32> {
    call_bytes(raw_get_stream_setting, code.as_bytes())
}

pub fn get_value(req: &[u8]) -> Result<Option<Vec<u8>>, i32> {
    call_bytes(raw_get_value, req)
}

#[allow(dead_code)]
pub fn send_value(req: &[u8]) -> Result<(), i32> {
    call_unit(raw_send_value, req)
}

pub fn put_value(req: &[u8]) -> Result<(), i32> {
    call_unit(raw_put_value, req)
}

#[allow(dead_code)]
pub fn remove_value(req: &[u8]) -> Result<(), i32> {
    call_unit(raw_remove_value, req)
}

#[allow(dead_code)]
pub fn http_request(req: &[u8]) -> Result<(), i32> {
    call_unit(raw_http_request, req)
}

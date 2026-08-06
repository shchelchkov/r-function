#[link(wasm_import_module = "env")]
extern "C" {
    #[link_name = "get_function_setting"]
    fn raw_get_function_setting(
        in_ptr: u32,
        in_len: u32,
        out_ptr_ptr: *mut u32,
        out_len_ptr: *mut u32,
    ) -> i32;

    #[allow(dead_code)]
    #[link_name = "send_value"]
    fn raw_send_value(
        in_ptr: u32,
        in_len: u32,
        out_ptr_ptr: *mut u32,
        out_len_ptr: *mut u32,
    ) -> i32;
}

#[no_mangle]
pub extern "C" fn alloc(len: u32) -> u32 {
    let mut buf = Vec::<u8>::with_capacity(len as usize);
    let ptr = buf.as_mut_ptr() as u32;
    std::mem::forget(buf);
    ptr
}

pub fn get_function_setting(code: &str) -> Result<Option<Vec<u8>>, i32> {
    let mut out_ptr: u32 = 0;
    let mut out_len: u32 = 0;

    let rc = unsafe {
        raw_get_function_setting(
            code.as_ptr() as u32,
            code.len() as u32,
            &mut out_ptr,
            &mut out_len,
        )
    };

    match rc {
        0 => {
            let bytes = unsafe {
                Vec::from_raw_parts(out_ptr as *mut u8, out_len as usize, out_len as usize)
            };
            Ok(Some(bytes))
        }
        1 => Ok(None),
        err => Err(err),
    }
}

#[allow(dead_code)]
pub fn send_value(req: &[u8]) -> Result<(), i32> {
    let mut out_ptr: u32 = 0;
    let mut out_len: u32 = 0;

    let rc = unsafe {
        raw_send_value(
            req.as_ptr() as u32,
            req.len() as u32,
            &mut out_ptr,
            &mut out_len,
        )
    };

    match rc {
        0 | 1 => Ok(()),
        err => Err(err),
    }
}

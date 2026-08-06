use std::sync::Arc;

use async_trait::async_trait;
use r_error::runtime::error::RuntimeError;
use tracing::warn;
use wasmtime::{Caller, Linker};

pub mod get_function_setting;
pub mod get_function_value;
pub mod get_stream_setting;
pub mod get_value;
pub mod http_request;
pub mod put_value;
pub mod remove_value;
pub mod send_value;
pub mod put_polygon;
pub mod remove_polygon;
pub mod contains_polygon;
pub mod contains_point;

pub use get_function_setting::GetFunctionSetting;
pub use get_function_value::GetFunctionValue;
pub use get_stream_setting::GetStreamSetting;
pub use http_request::HttpRequest;
pub use get_value::GetValue;
pub use put_value::PutValue;
pub use remove_value::RemoveValue;
pub use put_polygon::PutPolygon;
pub use remove_polygon::RemovePolygon;
pub use contains_polygon::ContainsPolygon;
pub use contains_point::ContainsPoint;
pub use send_value::{SendJob, SendValue};

use crate::runtime::executor::StoreCtx;

#[async_trait]
pub trait HostFn: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    async fn call(&self, input: &[u8]) -> Result<Option<Vec<u8>>, RuntimeError>;
}

pub struct HostRegistry {
    fns: Vec<Arc<dyn HostFn>>,
}

impl HostRegistry {
    pub fn new() -> Self {
        Self { fns: Vec::new() }
    }

    pub fn register<H: HostFn>(&mut self, h: H) {
        self.fns.push(Arc::new(h));
    }

    pub fn install(&self, linker: &mut Linker<StoreCtx>) -> Result<(), RuntimeError> {
        for hf in &self.fns {
            install_one(linker, hf.clone())?;
        }
        Ok(())
    }
}

impl Default for HostRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn install_one(linker: &mut Linker<StoreCtx>, hf: Arc<dyn HostFn>) -> Result<(), RuntimeError> {
    let name = hf.name();
    linker
        .func_wrap_async(
            "env",
            name,
            move |caller: Caller<'_, StoreCtx>, params: (u32, u32, u32, u32)| {
                let hf = hf.clone();
                Box::new(trampoline(caller, params, hf))
            },
        )
        .map(|_| ())
        .map_err(|e| RuntimeError::Load(e.to_string()))
}

async fn trampoline<'a>(
    mut caller: Caller<'a, StoreCtx>,
    (in_ptr, in_len, out_ptr_ptr, out_len_ptr): (u32, u32, u32, u32),
    hf: Arc<dyn HostFn>,
) -> i32 {
    let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
        Some(m) => m,
        None => {
            warn!(host_fn = hf.name(), "guest missing `memory` export");
            return -2;
        }
    };

    let alloc_func = match caller.get_export("alloc").and_then(|e| e.into_func()) {
        Some(f) => f,
        None => {
            warn!(host_fn = hf.name(), "guest missing `alloc` export");
            return -3;
        }
    };
    let alloc = match alloc_func.typed::<u32, u32>(&caller) {
        Ok(t) => t,
        Err(e) => {
            warn!(host_fn = hf.name(), error = %e, "`alloc` has wrong signature");
            return -3;
        }
    };

    let mut input = vec![0u8; in_len as usize];
    if let Err(e) = mem.read(&caller, in_ptr as usize, &mut input) {
        warn!(host_fn = hf.name(), error = %e, "read input from guest failed");
        return -2;
    }

    let resp = match hf.call(&input).await {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return 1,
        Err(e) => {
            warn!(host_fn = hf.name(), error = %e, "host fn returned error");
            return -1;
        }
    };

    let ptr = match alloc.call_async(&mut caller, resp.len() as u32).await {
        Ok(p) => p,
        Err(e) => {
            warn!(host_fn = hf.name(), error = %e, "guest alloc failed");
            return -3;
        }
    };

    if let Err(e) = mem.write(&mut caller, ptr as usize, &resp) {
        warn!(host_fn = hf.name(), error = %e, "write resp body failed");
        return -3;
    }
    if let Err(e) = mem.write(&mut caller, out_ptr_ptr as usize, &ptr.to_le_bytes()) {
        warn!(host_fn = hf.name(), error = %e, "write out_ptr failed");
        return -3;
    }
    if let Err(e) = mem.write(
        &mut caller,
        out_len_ptr as usize,
        &(resp.len() as u32).to_le_bytes(),
    ) {
        warn!(host_fn = hf.name(), error = %e, "write out_len failed");
        return -3;
    }
    0
}

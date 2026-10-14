use libloading::Library;
use r_error::plugin::error::PluginError;
use r_plugin_api::{ABI_VERSION, Buffer, HostApi};

type Process = unsafe extern "C" fn(*const HostApi, *const u8, usize) -> Buffer;

type FreeBuffer = unsafe extern "C" fn(Buffer);

type AbiVersion = unsafe extern "C" fn() -> u32;

pub struct Plugin {
    _library: Library,
    process: Process,
    process_discard: Option<Process>,
    free_buffer: FreeBuffer,
}

impl Plugin {
    pub unsafe fn load(path: &std::path::Path) -> Result<Self, PluginError> {
        let compile =
            |e: libloading::Error| PluginError::Compile(format!("{}: {e}", path.display()));

        let library = unsafe { Library::new(path) }.map_err(compile)?;

        let version = match unsafe { library.get::<AbiVersion>(b"abi_version") } {
            Ok(symbol) => unsafe { symbol() },
            Err(_) => 1,
        };
        if version > ABI_VERSION {
            return Err(PluginError::Compile(format!(
                "{}: plugin ABI {version} is newer than host ABI {ABI_VERSION}",
                path.display()
            )));
        }

        let process: Process = unsafe { *library.get::<Process>(b"process").map_err(compile)? };
        let free_buffer: FreeBuffer =
            unsafe { *library.get::<FreeBuffer>(b"free_buffer").map_err(compile)? };
        let process_discard: Option<Process> =
            unsafe { library.get::<Process>(b"process_discard").ok().map(|s| *s) };

        Ok(Self {
            _library: library,
            process,
            process_discard,
            free_buffer,
        })
    }

    pub unsafe fn process(&self, api: &HostApi, input: &[u8]) -> Buffer {
        unsafe { (self.process)(api, input.as_ptr(), input.len()) }
    }

    pub unsafe fn process_discard(&self, api: &HostApi, input: &[u8]) -> Buffer {
        let f = self.process_discard.unwrap_or(self.process);
        unsafe { f(api, input.as_ptr(), input.len()) }
    }

    pub unsafe fn free_buffer(&self, buffer: Buffer) {
        unsafe { (self.free_buffer)(buffer) }
    }
}

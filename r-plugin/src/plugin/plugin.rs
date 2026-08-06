use libloading::Library;
use r_plugin_api::{Buffer, HostApi};

type Process = unsafe extern "C" fn(*const HostApi, *const u8, usize) -> Buffer;

type FreeBuffer = unsafe extern "C" fn(Buffer);

pub struct Plugin {
    _library: Library,
    process: Process,
    free_buffer: FreeBuffer,
}

impl Plugin {
    pub unsafe fn load(path: &std::path::Path) -> Result<Self, libloading::Error> {
        let library = unsafe { Library::new(path)? };

        let process: Process = unsafe { *library.get::<Process>(b"process")? };
        let free_buffer: FreeBuffer = unsafe { *library.get::<FreeBuffer>(b"free_buffer")? };

        Ok(Self {
            _library: library,
            process,
            free_buffer,
        })
    }

    pub unsafe fn process(&self, api: &HostApi, input: &[u8]) -> Buffer {
        unsafe { (self.process)(api, input.as_ptr(), input.len()) }
    }

    pub unsafe fn free_buffer(&self, buffer: Buffer) {
        unsafe { (self.free_buffer)(buffer) }
    }
}

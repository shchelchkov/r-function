use libloading::{Library, Symbol};
use r_plugin_api::{Buffer, HostApi};

type Process = unsafe extern "C" fn(
    *const HostApi,
    *const u8,
    usize,
) -> Buffer;

type FreeBuffer = unsafe extern "C" fn(Buffer);

pub struct Plugin {
    _library: Library,
    process: Symbol<'static, Process>,
    free_buffer: Symbol<'static, FreeBuffer>,
}

impl Plugin {
    pub unsafe fn load(
        path: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let library = Library::new(path)?;

        let process = unsafe { library.get::<Process>(b"process")? };
        let free_buffer = unsafe { library.get::<FreeBuffer>(b"free_buffer")? };
        let process = unsafe { std::mem::transmute::<Symbol<'_, Process>, Symbol<'static, Process>>(process) };
        let free_buffer = unsafe { std::mem::transmute::<Symbol<'_, FreeBuffer>, Symbol<'static, FreeBuffer>>(free_buffer) };

        Ok(Self {
            _library: library,
            process,
            free_buffer,
        })
    }

    pub unsafe fn process(
        &self,
        api: &HostApi,
        input: &[u8],
    ) -> Buffer {
        (self.process)(
            api,
            input.as_ptr(),
            input.len(),
        )
    }

    pub unsafe fn free_buffer(
        &self,
        buffer: Buffer,
    ) {
        (self.free_buffer)(buffer);
    }
}
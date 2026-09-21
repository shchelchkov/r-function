pub mod abi;
pub mod buffer;
pub mod plugin;

pub use abi::{HistoryEntry, HostApi};
pub use buffer::Buffer;
pub use plugin::Plugin;
pub use r_error::plugin::error::PluginError;

pub const ABI_VERSION: u32 = 2;

#[macro_export]
macro_rules! export_abi_version {
    () => {
        #[unsafe(no_mangle)]
        pub extern "C" fn abi_version() -> u32 {
            $crate::ABI_VERSION
        }
    };
}

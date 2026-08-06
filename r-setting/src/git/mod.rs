mod fetch;
mod handle;
mod refresher;
mod remote;
pub mod setting_store;
mod sync;

pub use fetch::{fetch_setting, fetch_setting_value};
pub use handle::{GitHandle, RefreshOutcome};
pub use refresher::{HeadObserver, spawn_git_refresher};
pub use sync::git_sync;

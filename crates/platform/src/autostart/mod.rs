#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod sys;
#[cfg(windows)]
#[path = "windows.rs"]
mod sys;

pub use sys::{autostart_enabled, set_autostart};

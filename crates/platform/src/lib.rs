mod autostart;
mod desktop;
mod error;
mod external;
#[cfg(windows)]
mod input;
mod locks;
mod memory;
mod paths;
mod process;
mod secret;
#[cfg(windows)]
mod win;
mod window;
#[cfg(target_os = "linux")]
mod xdg;

pub use autostart::{autostart_enabled, set_autostart};
pub use desktop::install_desktop_entry;
pub use error::{PlatformError, Result};
pub use external::open_external;
#[cfg(windows)]
pub use input::{Key, press_key, type_text};
pub use locks::clear_stale_locks;
pub use memory::trim_memory;
pub use paths::Paths;
pub use process::{ProcessInfo, close_game, command_line, processes};
pub use secret::Secrets;
pub use window::set_window_title_for_pid;
#[cfg(windows)]
pub use window::{WindowHandle, focus_window, window_for_pid, window_titles_by_pid};

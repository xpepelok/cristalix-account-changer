pub mod instance;
#[cfg(windows)]
pub mod tray;
#[cfg(windows)]
pub mod tray_menu;
pub mod window;

use std::process::Command;

pub fn open_url(url: &str) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .ok();
    }
    #[cfg(not(windows))]
    {
        Command::new("xdg-open").arg(url).spawn().ok();
    }
}

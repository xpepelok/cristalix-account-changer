use std::process::Command;

use crate::error::Result;

#[cfg(windows)]
pub fn open_external(link: &str) -> Result<()> {
    Command::new("rundll32")
        .arg("url.dll,FileProtocolHandler")
        .arg(link)
        .spawn()?;
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn open_external(link: &str) -> Result<()> {
    let mut child = Command::new("xdg-open").arg(link).spawn()?;
    let _ = std::thread::Builder::new()
        .name("xdg-open".into())
        .spawn(move || child.wait());
    Ok(())
}

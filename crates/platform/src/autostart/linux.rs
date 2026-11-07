use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use crate::error::Result;
use crate::xdg;

fn entry_path() -> PathBuf {
    xdg::config_home()
        .join("autostart")
        .join("accountchanger.desktop")
}

pub fn autostart_enabled() -> bool {
    let Ok(raw) = fs::read_to_string(entry_path()) else {
        return false;
    };
    !raw.lines()
        .any(|line| line.trim().eq_ignore_ascii_case("Hidden=true"))
}

pub fn set_autostart(enabled: bool) -> Result<()> {
    let path = entry_path();
    if !enabled {
        return match fs::remove_file(&path) {
            Err(err) if err.kind() != ErrorKind::NotFound => Err(err.into()),
            _ => Ok(()),
        };
    }
    let exe = env::current_exe()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let entry = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=AccountChanger\n\
         Comment=Менеджер аккаунтов Cristalix\n\
         Exec={}\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n",
        xdg::quote_exec(&exe.to_string_lossy())
    );
    fs::write(&path, entry)?;
    Ok(())
}

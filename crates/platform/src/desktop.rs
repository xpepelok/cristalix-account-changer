use crate::error::Result;

#[cfg(windows)]
pub fn install_desktop_entry(_icon_png: &[u8]) -> Result<()> {
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn install_desktop_entry(icon_png: &[u8]) -> Result<()> {
    use std::{env, fs};

    use crate::xdg;

    let exe = env::current_exe()?;
    let data_home = xdg::data_home();
    if !icon_png.is_empty() {
        let icon_dir = data_home
            .join("icons")
            .join("hicolor")
            .join("256x256")
            .join("apps");
        if fs::create_dir_all(&icon_dir).is_ok() {
            let _ = fs::write(icon_dir.join("accountchanger.png"), icon_png);
        }
    }
    let apps_dir = data_home.join("applications");
    fs::create_dir_all(&apps_dir)?;
    let entry = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=AccountChanger\n\
         Comment=Менеджер аккаунтов Cristalix\n\
         Exec={}\n\
         Icon=accountchanger\n\
         Terminal=false\n\
         Categories=Utility;\n\
         StartupWMClass=Accountchanger\n",
        xdg::quote_exec(&exe.to_string_lossy())
    );
    fs::write(apps_dir.join("accountchanger.desktop"), entry)?;
    Ok(())
}

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const APP_DIR: &str = "AccountChanger";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    pub cristalix: PathBuf,
    pub launcher_cfg: PathBuf,
    pub updates: PathBuf,
    pub data: PathBuf,
    pub launcher_exe: PathBuf,
    pub vault: PathBuf,
    pub web_profile: PathBuf,
    pub profiles: PathBuf,
    pub session: PathBuf,
    pub staff_launcher_exe: PathBuf,
    pub launcher_jar: PathBuf,
    pub config: PathBuf,
    pub logs: PathBuf,
    pub players: PathBuf,
    pub db: PathBuf,
}

impl Paths {
    pub fn resolve() -> Self {
        Self::from_dirs(&home_dir(), data_root().join(APP_DIR))
    }

    pub fn from_dirs(home: &Path, data: PathBuf) -> Self {
        let cristalix = home.join(".cristalix");
        Self {
            launcher_cfg: cristalix.join(".launcher"),
            updates: cristalix.join("updates"),
            launcher_exe: data.join("Cristalix.exe"),
            vault: data.join("vault.dat"),
            web_profile: data.join("window"),
            profiles: data.join("profiles"),
            session: data.join("session.json"),
            staff_launcher_exe: data.join("CristalixLauncher.exe"),
            launcher_jar: data.join("Cristalix.jar"),
            config: data.join("config.json"),
            logs: data.join("logs.json"),
            players: data.join("players.json"),
            db: data.join("accountchanger.db"),
            cristalix,
            data,
        }
    }

    pub fn ensure(&self) -> io::Result<()> {
        fs::create_dir_all(&self.data)
    }
}

#[cfg(windows)]
fn home_dir() -> PathBuf {
    std::env::home_dir().unwrap_or_default()
}

#[cfg(windows)]
fn data_root() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join("AppData").join("Local"))
}

#[cfg(target_os = "linux")]
fn home_dir() -> PathBuf {
    crate::xdg::home()
}

#[cfg(target_os = "linux")]
fn data_root() -> PathBuf {
    crate::xdg::data_home()
}

#[cfg(test)]
mod tests {
    use super::Paths;
    use std::path::{Path, PathBuf};

    #[test]
    fn layout_matches_legacy_names() {
        let paths = Paths::from_dirs(Path::new("home"), PathBuf::from("data"));
        assert_eq!(paths.cristalix, Path::new("home").join(".cristalix"));
        assert_eq!(
            paths.launcher_cfg,
            Path::new("home").join(".cristalix").join(".launcher")
        );
        assert_eq!(paths.vault, Path::new("data").join("vault.dat"));
        assert_eq!(paths.db, Path::new("data").join("accountchanger.db"));
        assert_eq!(paths.players, Path::new("data").join("players.json"));
    }
}

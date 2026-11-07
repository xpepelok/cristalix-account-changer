use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::fsutil::{read_shared, write_atomic};
use crate::host::LaunchOptions;

const AUTO_RAM_THRESHOLD: i64 = 512;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LauncherConfig(Map<String, Value>);

impl LauncherConfig {
    pub fn read(path: &Path) -> io::Result<Self> {
        match read_shared(path) {
            Ok(raw) => Ok(Self(serde_json::from_slice(&raw).unwrap_or_default())),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err),
        }
    }

    pub fn write(&self, path: &Path) -> io::Result<()> {
        let payload = serde_json::to_vec(&self.0).map_err(io::Error::other)?;
        write_atomic(path, &payload)
    }

    pub fn accounts(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .get("accounts")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(name, token)| Some((name.as_str(), token.as_str()?)))
    }

    pub fn string(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(Value::as_str)
    }

    fn set(&mut self, key: &str, value: impl Into<Value>) {
        self.0.insert(key.to_owned(), value.into());
    }
}

pub fn apply_account(
    path: &Path,
    name: &str,
    token: &str,
    client: &str,
    opts: &LaunchOptions,
) -> io::Result<()> {
    let mut cfg = LauncherConfig::read(path)?;
    let mut accounts = Map::new();
    accounts.insert(name.to_owned(), Value::from(token));
    cfg.set("accounts", accounts);
    cfg.set("currentAccount", name);
    if !client.is_empty() {
        cfg.set("lastClient", client);
    }
    cfg.set("autoEnter", opts.auto_enter);
    cfg.set("minimalGraphics", opts.min_graphics);
    cfg.set("fullscreen", opts.fullscreen);
    cfg.set("discordRPC", opts.discord_rpc);
    cfg.set("debugMode", opts.debug_mode);
    let memory = if opts.ram >= AUTO_RAM_THRESHOLD {
        opts.ram
    } else {
        -1
    };
    cfg.set("memoryAmount", memory);
    cfg.write(path)
}

pub fn annul_account(path: &Path) -> io::Result<()> {
    let mut cfg = LauncherConfig::read(path)?;
    cfg.set("accounts", Map::new());
    cfg.set("currentAccount", "");
    cfg.set("autoEnter", false);
    cfg.write(path)
}

pub fn updates_dir(cfg_path: &Path, fallback: &Path) -> PathBuf {
    LauncherConfig::read(cfg_path)
        .ok()
        .and_then(|cfg| cfg.string("updatesDirectory").map(PathBuf::from))
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or_else(|| fallback.to_path_buf())
}

pub fn current_client(cfg_path: &Path) -> String {
    LauncherConfig::read(cfg_path)
        .ok()
        .and_then(|cfg| cfg.string("lastClient").map(str::to_owned))
        .unwrap_or_default()
}

pub fn effective_client(requested: &str, account_client: &str, cfg_path: &Path) -> String {
    [requested, account_client]
        .into_iter()
        .find(|client| !client.is_empty())
        .map_or_else(|| current_client(cfg_path), str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(ram: i64) -> LaunchOptions {
        LaunchOptions {
            ram,
            auto_enter: true,
            fullscreen: true,
            ..LaunchOptions::default()
        }
    }

    #[test]
    fn missing_or_corrupt_file_reads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".launcher");
        assert_eq!(
            LauncherConfig::read(&path).unwrap(),
            LauncherConfig::default()
        );
        std::fs::write(&path, b"{not json").unwrap();
        assert_eq!(
            LauncherConfig::read(&path).unwrap(),
            LauncherConfig::default()
        );
    }

    #[test]
    fn apply_replaces_accounts_and_keeps_foreign_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".launcher");
        std::fs::write(
            &path,
            br#"{"accounts":{"Old":"t0"},"updatesDirectory":"D:\\u","lastClient":"A"}"#,
        )
        .unwrap();
        apply_account(&path, "Steve", "tok", "", &opts(256)).unwrap();
        let cfg = LauncherConfig::read(&path).unwrap();
        assert_eq!(cfg.accounts().collect::<Vec<_>>(), vec![("Steve", "tok")]);
        assert_eq!(cfg.string("currentAccount"), Some("Steve"));
        assert_eq!(cfg.string("lastClient"), Some("A"));
        assert_eq!(cfg.string("updatesDirectory"), Some("D:\\u"));
        assert_eq!(cfg.0["memoryAmount"], Value::from(-1));
        assert_eq!(cfg.0["autoEnter"], Value::from(true));

        apply_account(&path, "Steve", "tok", "B", &opts(4096)).unwrap();
        let cfg = LauncherConfig::read(&path).unwrap();
        assert_eq!(cfg.string("lastClient"), Some("B"));
        assert_eq!(cfg.0["memoryAmount"], Value::from(4096));
        assert_eq!(updates_dir(&path, Path::new("x")), PathBuf::from("D:\\u"));
        assert_eq!(current_client(&path), "B");
    }

    #[test]
    fn annul_clears_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".launcher");
        apply_account(&path, "Steve", "tok", "B", &opts(0)).unwrap();
        annul_account(&path).unwrap();
        let cfg = LauncherConfig::read(&path).unwrap();
        assert_eq!(cfg.accounts().count(), 0);
        assert_eq!(cfg.string("currentAccount"), Some(""));
        assert_eq!(cfg.0["autoEnter"], Value::from(false));
        assert_eq!(cfg.string("lastClient"), Some("B"));
    }

    #[test]
    fn effective_client_prefers_request_then_account_then_launcher() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".launcher");
        std::fs::write(&path, br#"{"lastClient":"L"}"#).unwrap();
        assert_eq!(effective_client("R", "A", &path), "R");
        assert_eq!(effective_client("", "A", &path), "A");
        assert_eq!(effective_client("", "", &path), "L");
        assert_eq!(
            updates_dir(&path, Path::new("fallback")),
            PathBuf::from("fallback")
        );
    }
}

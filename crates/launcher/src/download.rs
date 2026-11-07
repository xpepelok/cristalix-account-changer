use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use reqwest::{Client, StatusCode};
use tokio::io::AsyncWriteExt;

use crate::error::{LauncherError, Result};

pub const LAUNCHER_URL: &str = "https://cristalix.gg/content/launcher/Cristalix.exe";
pub const JAR_LAUNCHER_URL: &str = "https://cristalix.gg/content/launcher/Cristalix.jar";
pub const STAFF_LAUNCHER_URL: &str =
    "https://cristalix.gg/content/launcher/new/CristalixLauncher.exe";

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const MANIFEST: &str = "META-INF/MANIFEST.MF";

pub struct Downloader {
    http: Client,
    locks: parking_lot::Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>,
}

impl Downloader {
    pub fn new() -> Result<Self> {
        Ok(Self {
            http: Client::builder().timeout(DOWNLOAD_TIMEOUT).build()?,
            locks: parking_lot::Mutex::default(),
        })
    }

    pub async fn ensure(&self, dest: &Path, url: &str) -> Result<()> {
        let lock = Arc::clone(self.locks.lock().entry(dest.to_path_buf()).or_default());
        let _guard = lock.lock().await;
        if is_usable(dest).await {
            return Ok(());
        }
        if let Some(dir) = dest.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        let mut response = self.http.get(url).send().await?;
        if response.status() != StatusCode::OK {
            return Err(LauncherError::Status(response.status().as_u16()));
        }
        let mut part = dest.as_os_str().to_owned();
        part.push(".part");
        let part = PathBuf::from(part);
        let mut file = tokio::fs::File::create(&part).await?;
        let written = async {
            while let Some(chunk) = response.chunk().await? {
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
            Ok::<_, LauncherError>(())
        }
        .await;
        drop(file);
        if let Err(err) = written {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(err);
        }
        tokio::fs::rename(&part, dest).await?;
        Ok(())
    }
}

async fn is_usable(dest: &Path) -> bool {
    let Ok(meta) = tokio::fs::metadata(dest).await else {
        return false;
    };
    if meta.len() == 0 {
        return false;
    }
    let is_jar = dest.extension().is_some_and(|ext| ext == "jar");
    if !is_jar {
        return true;
    }
    let path = dest.to_path_buf();
    let valid = tokio::task::spawn_blocking(move || jar_is_valid(&path))
        .await
        .unwrap_or(false);
    if !valid {
        let _ = tokio::fs::remove_file(dest).await;
    }
    valid
}

fn jar_is_valid(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    zip::ZipArchive::new(file)
        .is_ok_and(|archive| archive.file_names().any(|name| name == MANIFEST))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn jar_validation_requires_a_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("good.jar");
        let mut writer = zip::ZipWriter::new(File::create(&good).unwrap());
        writer
            .start_file(
                MANIFEST,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(b"Manifest-Version: 1.0\n").unwrap();
        writer.finish().unwrap();
        assert!(jar_is_valid(&good));

        let bad = dir.path().join("bad.jar");
        std::fs::write(&bad, b"<html>not a jar</html>").unwrap();
        assert!(!jar_is_valid(&bad));
    }

    #[tokio::test]
    async fn broken_jar_is_removed_before_download() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("Cristalix.jar");
        std::fs::write(&bad, b"garbage").unwrap();
        assert!(!is_usable(&bad).await);
        assert!(!bad.exists());
        let exe = dir.path().join("Cristalix.exe");
        std::fs::write(&exe, b"MZ").unwrap();
        assert!(is_usable(&exe).await);
    }
}

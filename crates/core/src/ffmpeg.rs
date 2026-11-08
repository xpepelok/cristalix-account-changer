use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, bail};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

const RELEASE: &str = "https://github.com/eugeneware/ffmpeg-static/releases/download/b6.1.1";

#[cfg(windows)]
const ASSET: Option<(&str, &str)> = Some((
    "ffmpeg-win32-x64.gz",
    "8883a3dffbd0a16cf4ef95206ea05283f78908dbfb118f73c83f4951dcc06d77",
));
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const ASSET: Option<(&str, &str)> = Some((
    "ffmpeg-linux-x64.gz",
    "bfe8a8fc511530457b528c48d77b5737527b504a3797a9bc4866aeca69c2dffa",
));
#[cfg(not(any(windows, all(target_os = "linux", target_arch = "x86_64"))))]
const ASSET: Option<(&str, &str)> = None;

const EXE: &str = if cfg!(windows) {
    "ffmpeg.exe"
} else {
    "ffmpeg"
};

fn lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub(crate) fn installed(data: &Path) -> Option<PathBuf> {
    let path = data.join("ffmpeg").join(EXE);
    path.is_file().then_some(path)
}

async fn system() -> Option<PathBuf> {
    if cfg!(windows) {
        return None;
    }
    let status = tokio::process::Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .ok()?;
    status.success().then(|| PathBuf::from("ffmpeg"))
}

pub(crate) async fn ensure(data: &Path) -> anyhow::Result<PathBuf> {
    let _guard = lock().lock().await;
    if let Some(path) = installed(data) {
        return Ok(path);
    }
    if let Some(path) = system().await {
        return Ok(path);
    }
    let Some((asset, checksum)) = ASSET else {
        bail!("установи ffmpeg через пакетный менеджер");
    };
    let packed = reqwest::get(format!("{RELEASE}/{asset}"))
        .await
        .and_then(reqwest::Response::error_for_status)
        .context("нет связи с GitHub")?
        .bytes()
        .await
        .context("загрузка прервалась")?;
    let digest = Sha256::digest(&packed);
    if !digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
        .eq(checksum)
    {
        bail!("файл повреждён");
    }
    let folder = data.join("ffmpeg");
    let target = folder.join(EXE);
    let partial = folder.join(format!("{EXE}.part"));
    let unpacked = tokio::task::spawn_blocking(move || unpack(&packed))
        .await
        .context("распаковка")??;
    tokio::fs::create_dir_all(&folder).await?;
    tokio::fs::write(&partial, unpacked).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&partial, std::fs::Permissions::from_mode(0o755)).await?;
    }
    tokio::fs::rename(&partial, &target).await?;
    Ok(target)
}

fn unpack(packed: &[u8]) -> anyhow::Result<Vec<u8>> {
    use std::io::Read;
    let mut unpacked = Vec::with_capacity(packed.len() * 3);
    flate2::read::GzDecoder::new(packed)
        .read_to_end(&mut unpacked)
        .context("распаковка")?;
    Ok(unpacked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpacks_gzip() {
        use std::io::Write;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(b"ffmpeg").unwrap();
        let packed = encoder.finish().unwrap();
        assert_eq!(unpack(&packed).unwrap(), b"ffmpeg");
    }

    #[test]
    fn finds_installed_binary() {
        let dir = tempfile::tempdir().unwrap();
        assert!(installed(dir.path()).is_none());
        std::fs::create_dir_all(dir.path().join("ffmpeg")).unwrap();
        std::fs::write(dir.path().join("ffmpeg").join(EXE), b"").unwrap();
        assert!(installed(dir.path()).is_some());
    }
}

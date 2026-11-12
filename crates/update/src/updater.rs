use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use reqwest::header::ACCEPT;
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::runtime::Handle;
use tokio::sync::watch;

use crate::error::UpdateError;
use crate::install::{install_downloaded, restart, sibling};
use crate::release::{Release, UpdateInfo};

const RELEASE_API: &str =
    "https://api.github.com/repos/xpepelok/cristalix-account-changer/releases/latest";
const USER_AGENT: &str = "AccountChanger";
const CHECK_CACHE: Duration = Duration::from_secs(10 * 60);
const CHECK_TIMEOUT: Duration = Duration::from_secs(15);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const RESTART_DELAY: Duration = Duration::from_millis(600);

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateProgress {
    pub active: bool,
    pub done: bool,
    pub downloaded: u64,
    pub total: u64,
    pub error: String,
}

impl UpdateProgress {
    pub fn percent(&self) -> u8 {
        match self.total {
            0 => 0,
            total => (self.downloaded.min(total) * 100 / total) as u8,
        }
    }
}

pub struct Updater {
    http: Client,
    exe: PathBuf,
    release_api: String,
    latest: Mutex<Option<(Instant, UpdateInfo)>>,
    progress: watch::Sender<UpdateProgress>,
}

impl Updater {
    pub fn new(exe: PathBuf) -> Result<Self, UpdateError> {
        Ok(Self {
            http: Client::builder().user_agent(USER_AGENT).build()?,
            exe,
            release_api: RELEASE_API.to_owned(),
            latest: Mutex::new(None),
            progress: watch::Sender::new(UpdateProgress::default()),
        })
    }

    pub fn for_current_exe() -> Result<Self, UpdateError> {
        Self::new(std::env::current_exe()?)
    }

    #[cfg(test)]
    pub fn with_release_api(mut self, url: impl Into<String>) -> Self {
        self.release_api = url.into();
        self
    }

    pub fn exe(&self) -> &Path {
        &self.exe
    }

    pub fn progress(&self) -> UpdateProgress {
        self.progress.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<UpdateProgress> {
        self.progress.subscribe()
    }

    pub async fn check(&self) -> UpdateInfo {
        if let Some((fetched, info)) = &*self.latest.lock()
            && fetched.elapsed() < CHECK_CACHE
        {
            return info.clone();
        }
        match self.fetch_latest().await {
            Some(info) => {
                *self.latest.lock() = Some((Instant::now(), info.clone()));
                info
            }
            None => UpdateInfo::unknown(),
        }
    }

    pub fn start<F>(self: &Arc<Self>, runtime: &Handle, on_done: F) -> bool
    where
        F: FnOnce() + Send + 'static,
    {
        let started = self.progress.send_if_modified(|progress| {
            if progress.active {
                return false;
            }
            *progress = UpdateProgress {
                active: true,
                ..UpdateProgress::default()
            };
            true
        });
        if !started {
            return false;
        }

        let updater = Arc::clone(self);
        runtime.spawn(async move {
            if let Err(err) = updater.apply().await {
                updater.progress.send_modify(|progress| {
                    progress.active = false;
                    progress.error = err.to_string();
                });
                return;
            }
            updater.progress.send_modify(|progress| {
                progress.active = false;
                progress.done = true;
            });
            tokio::time::sleep(RESTART_DELAY).await;
            let _ = restart(&updater.exe);
            on_done();
        });
        true
    }

    async fn fetch_latest(&self) -> Option<UpdateInfo> {
        let response = self
            .http
            .get(&self.release_api)
            .header(ACCEPT, "application/vnd.github+json")
            .timeout(CHECK_TIMEOUT)
            .send()
            .await
            .ok()?;
        if response.status() != StatusCode::OK {
            return None;
        }
        let body = response.bytes().await.ok()?;
        UpdateInfo::from_release(serde_json::from_slice::<Release>(&body).ok()?)
    }

    async fn apply(&self) -> Result<(), UpdateError> {
        let info = self.check().await;
        let url = info
            .asset_url
            .filter(|_| info.available)
            .ok_or(UpdateError::NoUpdate)?;

        let response = self.http.get(url).timeout(DOWNLOAD_TIMEOUT).send().await?;
        if response.status() != StatusCode::OK {
            return Err(UpdateError::Status(response.status().as_u16()));
        }
        let total = response.content_length().unwrap_or(0);
        self.progress.send_modify(|progress| progress.total = total);

        let staged = sibling(&self.exe, ".new");
        if let Err(err) = self.download(response, &staged).await {
            let _ = tokio::fs::remove_file(&staged).await;
            return Err(err);
        }

        let exe = self.exe.clone();
        tokio::task::spawn_blocking(move || install_downloaded(&staged, &exe))
            .await
            .map_err(std::io::Error::other)??;
        Ok(())
    }

    async fn download(&self, mut response: Response, target: &Path) -> Result<(), UpdateError> {
        let mut file = tokio::fs::File::create(target).await?;
        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk).await?;
            let received = chunk.len() as u64;
            self.progress
                .send_modify(|progress| progress.downloaded += received);
        }
        file.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_is_bounded() {
        let mut progress = UpdateProgress {
            downloaded: 50,
            total: 200,
            ..UpdateProgress::default()
        };
        assert_eq!(progress.percent(), 25);
        progress.downloaded = 500;
        assert_eq!(progress.percent(), 100);
        progress.total = 0;
        assert_eq!(progress.percent(), 0);
    }

    #[tokio::test]
    async fn cached_check_skips_network() {
        let updater = Updater::new(PathBuf::from("AccountChanger")).unwrap();
        let cached = UpdateInfo {
            latest: "9.9.9".into(),
            ..UpdateInfo::unknown()
        };
        *updater.latest.lock() = Some((Instant::now(), cached.clone()));
        assert_eq!(updater.check().await, cached);
    }

    #[tokio::test]
    async fn start_without_update_reports_error_and_rearms() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("AccountChanger");
        std::fs::write(&exe, b"current").unwrap();
        let updater = Arc::new(Updater::new(exe.clone()).unwrap());
        *updater.latest.lock() = Some((Instant::now(), UpdateInfo::unknown()));
        let mut changes = updater.subscribe();

        assert!(updater.start(&Handle::current(), || {}));
        assert!(!updater.start(&Handle::current(), || {}));

        let finished = changes
            .wait_for(|progress| !progress.active)
            .await
            .unwrap()
            .clone();
        assert_eq!(finished.error, "нет доступного обновления");
        assert!(!finished.done);
        assert_eq!(std::fs::read(&exe).unwrap(), b"current");
        assert!(updater.start(&Handle::current(), || {}));
    }

    async fn serve(listener: tokio::net::TcpListener, release: String, binary: &'static [u8]) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 1024];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let read = socket.read(&mut buffer).await.unwrap();
                request.extend_from_slice(&buffer[..read]);
            }
            let body: &[u8] = if request.starts_with(b"GET /latest ") {
                release.as_bytes()
            } else {
                binary
            };
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            socket.write_all(head.as_bytes()).await.unwrap();
            socket.write_all(body).await.unwrap();
            socket.shutdown().await.unwrap();
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn downloads_swaps_and_signals_restart() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let release = serde_json::json!({
            "tag_name": "v99.0.0",
            "name": "AccountChanger 99",
            "body": "notes",
            "published_at": "2026-11-01T10:00:00Z",
            "assets": [
                {"name": "checksums.txt", "browser_download_url": format!("{base}/checksums")},
                {"name": crate::ASSET_NAME, "browser_download_url": format!("{base}/binary")},
            ],
        })
        .to_string();
        let binary: &'static [u8] = &[7u8; 300_000];
        tokio::spawn(serve(listener, release, binary));

        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("AccountChanger");
        std::fs::write(&exe, b"old build").unwrap();
        let updater = Arc::new(
            Updater::new(exe.clone())
                .unwrap()
                .with_release_api(format!("{base}/latest")),
        );
        let info = updater.check().await;
        assert!(info.available);
        assert_eq!(info.latest, "99.0.0");

        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        assert!(updater.start(&Handle::current(), move || {
            let _ = done_tx.send(());
        }));
        tokio::time::timeout(Duration::from_secs(20), done_rx)
            .await
            .unwrap()
            .unwrap();

        let progress = updater.progress();
        assert!(progress.done && !progress.active && progress.error.is_empty());
        assert_eq!((progress.downloaded, progress.total), (300_000, 300_000));
        assert_eq!(progress.percent(), 100);
        assert_eq!(std::fs::read(&exe).unwrap(), binary);
        assert_eq!(std::fs::read(sibling(&exe, ".old")).unwrap(), b"old build");
        assert!(!sibling(&exe, ".new").exists());
    }

    #[tokio::test]
    #[ignore = "hits api.github.com"]
    async fn live_release_check() {
        let updater = Updater::new(PathBuf::from("AccountChanger")).unwrap();
        let info = updater.check().await;
        assert!(!info.latest.is_empty());
        assert!(!info.published_at.is_empty());
    }
}

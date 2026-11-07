use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;

use crate::config::LauncherConfig;
use crate::host::Host;
use crate::instance::instance_configs;

const RESCAN_INTERVAL: Duration = Duration::from_secs(3);

pub struct Watcher {
    launcher_cfg: PathBuf,
    data: PathBuf,
}

impl Watcher {
    pub fn new(launcher_cfg: PathBuf, data: PathBuf) -> Self {
        Self { launcher_cfg, data }
    }

    pub async fn capture<H: Host>(&self, host: &H) {
        capture_config(host, &self.launcher_cfg).await;
        for config in instance_configs(&self.data) {
            capture_config(host, &config).await;
        }
    }

    pub async fn run<H: Host>(&self, host: &H) {
        let (events, mut changed) = mpsc::channel::<bool>(1);
        let mut watcher: Option<RecommendedWatcher> = None;
        let mut rescan = tokio::time::interval(RESCAN_INTERVAL);
        rescan.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            if watcher.is_none() {
                watcher = self.watch(events.clone());
            }
            self.capture(host).await;
            tokio::select! {
                _ = rescan.tick() => {}
                Some(healthy) = changed.recv() => {
                    if !healthy {
                        watcher = None;
                    }
                }
            }
        }
    }

    fn watch(&self, events: mpsc::Sender<bool>) -> Option<RecommendedWatcher> {
        let dir = self.launcher_cfg.parent()?;
        let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
            let signal = match event {
                Ok(event) if matches!(event.kind, EventKind::Access(_)) => return,
                Ok(_) => true,
                Err(_) => false,
            };
            let _ = events.try_send(signal);
        })
        .ok()?;
        watcher.watch(dir, RecursiveMode::NonRecursive).ok()?;
        Some(watcher)
    }
}

pub async fn capture_config<H: Host>(host: &H, path: &Path) {
    let Ok(config) = LauncherConfig::read(path) else {
        return;
    };
    for (name, token) in config.accounts() {
        if !token.is_empty() {
            host.capture_token(name, token).await;
        }
    }
}

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use ac_api::{BackendEvent, Caps, LauncherKind, OnlineCounts, OnlineStats};
use ac_launcher::{
    Downloader, GameTracker, Games, JAR_LAUNCHER_URL, LaunchQueue, Presence, Titles, Watcher,
};
use ac_platform::{Paths, Secrets};
use ac_player::{CACHE_MAX_AGE, CacheEntry, FLUSH_INTERVAL, PlayerService, TextureClient};
use ac_stats::StatsService;
use ac_store::{LegacyPaths, Store};
use ac_update::Updater;
use anyhow::Context as _;
use tokio::runtime::{Builder, Handle, Runtime};
use tokio::sync::mpsc;

use crate::convert::{
    self, from_unix, launch_record, launcher_kind, store_launcher_kind, to_unix, unix_now,
};
use crate::events::Events;
use crate::host::{Signal, StoreHost};
use crate::import::ImportJob;

const TRIM_DELAY: Duration = Duration::from_secs(8);
const TRIM_INTERVAL: Duration = Duration::from_secs(60);
const ONLINE_STATS_INTERVAL: Duration = Duration::from_secs(60);
const WEB_CACHE_DIRS: [&str; 4] = ["Cache", "Code Cache", "GPUCache", "Service Worker"];

pub struct Core {
    pub(crate) handle: Handle,
    pub(crate) inner: Arc<Inner>,
    runtime: Option<Runtime>,
}

pub(crate) struct Inner {
    pub handle: Handle,
    pub paths: Paths,
    pub caps: Caps,
    pub store: Arc<Store>,
    pub host: Arc<StoreHost>,
    pub tracker: Arc<GameTracker>,
    pub titles: Arc<Titles>,
    pub watcher: Watcher,
    pub queue: Arc<LaunchQueue<StoreHost>>,
    pub downloader: Arc<Downloader>,
    pub players: Arc<PlayerService>,
    pub textures: TextureClient,
    pub stats: StatsService,
    pub updater: Arc<Updater>,
    pub events: Events,
    pub import: ImportJob,
    pub cabinet_http: reqwest::Client,
}

impl Core {
    pub fn start(paths: Paths) -> anyhow::Result<Self> {
        ac_update::cleanup_old_exe(&std::env::current_exe().context("current exe")?);
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("ac-core")
            .enable_all()
            .build()
            .context("tokio runtime")?;
        let (inner, signals) = runtime.block_on(Inner::open(paths))?;
        let handle = runtime.handle().clone();
        inner.spawn_services(&handle, signals);
        Ok(Self {
            handle,
            inner,
            runtime: Some(runtime),
        })
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

pub(crate) fn launcher_allowed(caps: Caps, kind: LauncherKind) -> bool {
    caps.exe_launchers || !kind.is_exe()
}

fn clear_web_cache(profile: &Path) {
    let base = profile.join("EBWebView").join("Default");
    for dir in WEB_CACHE_DIRS {
        let _ = std::fs::remove_dir_all(base.join(dir));
    }
}

impl Inner {
    pub(crate) async fn open(
        paths: Paths,
    ) -> anyhow::Result<(Arc<Self>, mpsc::UnboundedReceiver<Signal>)> {
        let _ = paths.ensure();
        clear_web_cache(&paths.web_profile);
        let legacy = LegacyPaths {
            vault: paths.vault.clone(),
            config: paths.config.clone(),
            players: paths.players.clone(),
            session: paths.session.clone(),
            logs: paths.logs.clone(),
        };
        let store = Arc::new(
            Store::open(&paths.db, Arc::new(Secrets::new(&paths)), legacy)
                .await
                .context("open store")?,
        );
        let caps = Caps::current();
        if !launcher_allowed(caps, launcher_kind(store.launcher())) {
            store
                .set_launcher(store_launcher_kind(LauncherKind::Jar))
                .await?;
        }

        let players = Arc::new(PlayerService::new()?);
        let _ = store
            .prune_players(unix_now() - CACHE_MAX_AGE.as_secs() as i64)
            .await;
        let cached = store.load_player_cache().await.unwrap_or_default();
        players.import(cached.into_iter().filter_map(|entry| {
            Some(CacheEntry {
                key: entry.name,
                info: serde_json::from_str(&entry.info_json).ok()?,
                fetched_at: from_unix(entry.fetched_at),
            })
        }));

        let (signals, receiver) = mpsc::unbounded_channel();
        let host = Arc::new(StoreHost {
            store: Arc::clone(&store),
            players: Arc::clone(&players),
            signals,
        });

        let games = Arc::new(Games::default());
        let records = store.load_launches().await.unwrap_or_default();
        let tracker = Arc::new(GameTracker::new(
            Arc::clone(&games),
            records.into_iter().map(launch_record).collect(),
        ));
        let accounts = store.list();
        if store.window_title() {
            games.allow_titles(
                accounts
                    .iter()
                    .filter(|account| !account.name.is_empty())
                    .map(|account| account.name.as_str()),
            );
        }
        tracker.seed(
            accounts
                .iter()
                .map(|account| (account.uuid.clone(), account.last_launched)),
        );

        let titles = Arc::new(Titles::default());
        let downloader = Arc::new(Downloader::new()?);
        let queue = LaunchQueue::start(
            paths.clone(),
            Arc::clone(&host),
            Arc::clone(&tracker),
            Arc::clone(&titles),
            Arc::clone(&downloader),
        );
        let inner = Arc::new(Self {
            handle: Handle::current(),
            watcher: Watcher::new(paths.launcher_cfg.clone(), paths.data.clone()),
            textures: TextureClient::new()?,
            stats: StatsService::new()?,
            updater: Arc::new(Updater::for_current_exe()?),
            events: Events::default(),
            import: ImportJob::default(),
            cabinet_http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()?,
            paths,
            caps,
            store,
            host,
            tracker,
            titles,
            queue,
            downloader,
            players,
        });
        Ok((inner, receiver))
    }

    fn spawn_services(self: &Arc<Self>, handle: &Handle, signals: mpsc::UnboundedReceiver<Signal>) {
        handle.spawn(async {
            tokio::time::sleep(TRIM_DELAY).await;
            loop {
                ac_platform::trim_memory();
                tokio::time::sleep(TRIM_INTERVAL).await;
            }
        });
        let inner = Arc::clone(self);
        handle.spawn(async move {
            let store = Arc::clone(&inner.store);
            inner
                .stats
                .ping_loop(&inner.paths.data, move || store.stats())
                .await;
        });
        let inner = Arc::clone(self);
        handle.spawn(async move {
            let _ = inner
                .downloader
                .ensure(&inner.paths.launcher_jar, JAR_LAUNCHER_URL)
                .await;
        });
        let inner = Arc::clone(self);
        handle.spawn(async move { inner.watcher.run(inner.host.as_ref()).await });
        let inner = Arc::clone(self);
        handle.spawn(async move {
            let store = Arc::clone(&inner.store);
            inner
                .players
                .refresh_loop(move || account_names(&store))
                .await;
        });
        let inner = Arc::clone(self);
        handle.spawn(async move {
            inner.titles.run(&inner.tracker, inner.host.as_ref()).await;
        });
        let inner = Arc::clone(self);
        handle.spawn(async move { inner.tracker.persist_loop(inner.host.as_ref()).await });
        let inner = Arc::clone(self);
        handle.spawn(async move {
            loop {
                tokio::time::sleep(FLUSH_INTERVAL).await;
                inner.flush_players().await;
            }
        });
        let inner = Arc::clone(self);
        handle.spawn(async move { inner.online_stats_loop().await });
        let inner = Arc::clone(self);
        handle.spawn(async move { inner.update_progress_loop().await });
        let inner = Arc::clone(self);
        handle.spawn(async move { inner.event_pump(signals).await });
    }

    pub(crate) async fn flush_players(&self) {
        let Some(entries) = self.players.take_dirty() else {
            return;
        };
        for entry in entries {
            if let Ok(json) = serde_json::to_string(&entry.info) {
                let _ = self
                    .store
                    .upsert_player(&entry.key, &json, to_unix(entry.fetched_at))
                    .await;
            }
        }
    }

    pub(crate) async fn online_stats(&self) -> OnlineStats {
        if !self.store.stats() {
            return OnlineStats::Disabled;
        }
        match self.stats.fetch().await {
            Some(counts) => OnlineStats::Live(OnlineCounts {
                online: counts.online,
                active: counts.active,
                total: counts.total,
            }),
            None => OnlineStats::Unavailable,
        }
    }

    async fn online_stats_loop(&self) {
        loop {
            tokio::time::sleep(ONLINE_STATS_INTERVAL).await;
            if self.store.stats() {
                let stats = self.online_stats().await;
                self.events.emit(BackendEvent::OnlineStats(stats));
            }
        }
    }

    async fn update_progress_loop(&self) {
        let mut progress = self.updater.subscribe();
        while progress.changed().await.is_ok() {
            let current = progress.borrow_and_update().clone();
            self.events
                .emit(BackendEvent::Update(convert::update_progress(current)));
        }
    }

    pub(crate) fn account_list(&self, presence: &Presence) -> Vec<ac_api::Account> {
        convert::accounts(self.store.list(), presence)
    }

    pub(crate) fn group_list(&self) -> Vec<ac_api::Group> {
        self.store
            .sorted_groups()
            .into_iter()
            .map(convert::group)
            .collect()
    }

    async fn event_pump(&self, mut signals: mpsc::UnboundedReceiver<Signal>) {
        let mut revision = self.store.subscribe();
        let mut presence = self.tracker.subscribe();
        let mut last_accounts = None;
        let mut last_groups = None;
        let mut last_queue = None;
        loop {
            let (accounts, groups, queue, signal) = tokio::select! {
                changed = revision.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    (true, true, false, None)
                }
                changed = presence.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    (true, false, true, None)
                }
                signal = signals.recv() => {
                    let Some(signal) = signal else {
                        return;
                    };
                    (false, false, signal == Signal::Queue, Some(signal))
                }
            };
            let current = self.tracker.presence();
            if accounts {
                emit_changed(
                    &self.events,
                    &mut last_accounts,
                    self.account_list(&current),
                    BackendEvent::Accounts,
                );
            }
            if groups {
                emit_changed(
                    &self.events,
                    &mut last_groups,
                    self.group_list(),
                    BackendEvent::Groups,
                );
            }
            if queue {
                emit_changed(
                    &self.events,
                    &mut last_queue,
                    self.queue.status(&current.running),
                    BackendEvent::LaunchQueue,
                );
            }
            match signal {
                Some(Signal::GroupLaunch) => self
                    .events
                    .emit(BackendEvent::GroupLaunch(self.queue.group_progress())),
                Some(Signal::Logs) => self.events.emit(BackendEvent::LogsChanged),
                Some(Signal::Queue) | None => {}
            }
        }
    }
}

fn emit_changed<T: Clone + PartialEq>(
    events: &Events,
    last: &mut Option<T>,
    current: T,
    event: impl FnOnce(T) -> BackendEvent,
) {
    if last.as_ref() == Some(&current) {
        return;
    }
    *last = Some(current.clone());
    events.emit(event(current));
}

pub(crate) fn account_names(store: &Store) -> Vec<String> {
    store
        .list()
        .into_iter()
        .map(|account| account.name)
        .filter(|name| !name.is_empty())
        .collect()
}

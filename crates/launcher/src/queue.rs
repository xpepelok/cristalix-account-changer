use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ac_api::{GroupLaunchProgress, GroupSkip, LauncherKind, QueueEntry, QueueState, msg};
use ac_platform::Paths;
use parking_lot::Mutex;
use tokio::process::Child;
use tokio::sync::{Notify, mpsc, oneshot, watch};
use tokio::time::Instant;

use crate::autoplay::{AUTO_PLAY_TIMEOUT, click_play_for_pid};
use crate::clients::list_clients;
use crate::config::{annul_account, apply_account, current_client, updates_dir};
use crate::download::{Downloader, JAR_LAUNCHER_URL, LAUNCHER_URL, STAFF_LAUNCHER_URL};
use crate::fsutil::is_dir;
use crate::host::{Account, Change, Host, short_uuid};
use crate::instance::{instance_config, instance_dir, instances_dir, link_shared_dir, remove_link};
use crate::java::resolve_java;
use crate::logtail::LogTail;
use crate::options::{apply_client_options_all, apply_minimal_options_all};
use crate::profiles::{MINIMAL_PROFILE, apply_profile};
use crate::start::{spawn_launcher, spawn_launcher_jar, start_launcher};
use crate::titles::Titles;
use crate::tracker::GameTracker;
use crate::watcher::capture_config;

const GAME_APPEAR_WINDOW: Duration = Duration::from_secs(15 * 60);
const CLOSE_POLL: Duration = Duration::from_secs(1);
const JAR_CHECK_WINDOW: Duration = Duration::from_secs(6);
const STAGING_CLIENT: &str = "Minigames-staging-java21";
const CUSTOM_NO_LOGS: &str = "Этот лаунчер не поддерживает чтение логов";

#[derive(Default)]
struct Pending {
    pending: HashSet<String>,
    client: HashMap<String, String>,
    order: Vec<String>,
    current: String,
}

#[derive(Clone, Default)]
struct GroupState {
    active: bool,
    paused: bool,
    cancelled: bool,
    done: u32,
    total: u32,
    skips: Vec<GroupSkip>,
}

struct Tracked {
    pid: u32,
    kill: Arc<Notify>,
}

pub struct LaunchQueue<H: Host> {
    paths: Paths,
    host: Arc<H>,
    tracker: Arc<GameTracker>,
    titles: Arc<Titles>,
    downloader: Arc<Downloader>,
    jobs: mpsc::UnboundedSender<String>,
    state: Mutex<Pending>,
    group: watch::Sender<GroupState>,
    procs: Mutex<HashMap<String, Tracked>>,
    log: Mutex<()>,
}

impl<H: Host> LaunchQueue<H> {
    pub fn start(
        paths: Paths,
        host: Arc<H>,
        tracker: Arc<GameTracker>,
        titles: Arc<Titles>,
        downloader: Arc<Downloader>,
    ) -> Arc<Self> {
        let (jobs, receiver) = mpsc::unbounded_channel();
        let queue = Arc::new(Self {
            paths,
            host,
            tracker,
            titles,
            downloader,
            jobs,
            state: Mutex::default(),
            group: watch::Sender::new(GroupState::default()),
            procs: Mutex::default(),
            log: Mutex::new(()),
        });
        tokio::spawn(Arc::clone(&queue).worker(receiver));
        queue
    }

    async fn worker(self: Arc<Self>, mut jobs: mpsc::UnboundedReceiver<String>) {
        self.clean_stale_instances().await;
        while let Some(uuid) = jobs.recv().await {
            let client = {
                let mut state = self.state.lock();
                state.current.clone_from(&uuid);
                state.client.get(&uuid).cloned().unwrap_or_default()
            };
            self.host.notify(Change::Queue);
            Arc::clone(&self).run(&uuid, &client).await;
            {
                let mut state = self.state.lock();
                state.current.clear();
                state.pending.remove(&uuid);
                state.client.remove(&uuid);
                state.order.retain(|queued| *queued != uuid);
            }
            self.host.notify(Change::Queue);
        }
    }

    pub fn enqueue(&self, uuid: &str, client: &str) -> bool {
        {
            let mut state = self.state.lock();
            if !state.pending.insert(uuid.to_owned()) {
                return false;
            }
            state.client.insert(uuid.to_owned(), client.to_owned());
            state.order.push(uuid.to_owned());
        }
        self.tracker.note_launch(uuid);
        let _ = self.jobs.send(uuid.to_owned());
        self.host.notify(Change::Queue);
        true
    }

    pub fn status(&self, running: &HashMap<String, u32>) -> Vec<QueueEntry> {
        let (order, current) = {
            let state = self.state.lock();
            (state.order.clone(), state.current.clone())
        };
        order
            .into_iter()
            .filter(|uuid| running.get(uuid).is_none_or(|pid| *pid == 0))
            .map(|uuid| QueueEntry {
                name: self
                    .host
                    .account(&uuid)
                    .map(|account| account.name)
                    .unwrap_or_default(),
                state: if uuid == current {
                    QueueState::Launching
                } else {
                    QueueState::Pending
                },
                uuid,
            })
            .collect()
    }

    pub fn kill_proc(&self, uuid: &str) {
        let tracked = self.procs.lock().remove(uuid);
        if let Some(tracked) = tracked {
            self.tracker.games().unregister_launcher(tracked.pid);
            tracked.kill.notify_one();
        }
    }

    fn launcher_pid(&self, uuid: &str) -> u32 {
        self.procs.lock().get(uuid).map_or(0, |tracked| tracked.pid)
    }

    fn track(self: &Arc<Self>, uuid: &str, mut child: Child) -> Option<Arc<Notify>> {
        let pid = child.id()?;
        let kill = Arc::new(Notify::new());
        self.procs.lock().insert(
            uuid.to_owned(),
            Tracked {
                pid,
                kill: Arc::clone(&kill),
            },
        );
        self.tracker.games().register_launcher(pid);
        let queue = Arc::clone(self);
        let uuid = uuid.to_owned();
        let stop = Arc::clone(&kill);
        tokio::spawn(async move {
            tokio::select! {
                _ = child.wait() => {}
                () = stop.notified() => {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                }
            }
            let mut procs = queue.procs.lock();
            if procs.get(&uuid).is_some_and(|tracked| tracked.pid == pid) {
                procs.remove(&uuid);
                queue.tracker.games().unregister_launcher(pid);
            }
        });
        Some(kill)
    }

    fn launch_log(&self, line: impl Display) {
        let stamp = chrono::Local::now().format("%H:%M:%S");
        let _guard = self.log.lock();
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.paths.data.join("launch.log"))
        {
            let _ = writeln!(file, "[{stamp}] {line}");
        }
    }

    fn effective_client(&self, requested: &str, account: &Account) -> String {
        [requested, account.client.as_str()]
            .into_iter()
            .find(|client| !client.is_empty())
            .map_or_else(|| current_client(&self.paths.launcher_cfg), str::to_owned)
    }

    fn updates(&self) -> PathBuf {
        updates_dir(&self.paths.launcher_cfg, &self.paths.updates)
    }

    async fn run(self: Arc<Self>, uuid: &str, client: &str) {
        let account = self.host.account(uuid).filter(Account::launchable);
        let Some(account) = account else {
            self.launch_log(format_args!(
                "run {}: пропуск — нет аккаунта или токена (ok={})",
                short_uuid(uuid),
                self.host.account(uuid).is_some()
            ));
            return;
        };
        let config = self.host.config();
        self.launch_log(format_args!(
            "=== запуск '{}' (client={client}, launcher={}, aggressive={}, autoPlay={}) ===",
            account.name,
            launcher_name(config.launcher),
            config.aggressive,
            config.auto_play
        ));
        if self.tracker.resolve().is_running(uuid) {
            self.launch_log(format_args!("'{}': уже запущен, пропуск", account.name));
            return;
        }
        if !client.is_empty() {
            self.host.set_client(uuid, client).await;
        }
        let effective = self.effective_client(client, &account);
        ac_platform::clear_stale_locks(&self.paths.cristalix);
        if config.launcher == LauncherKind::Custom
            && apply_account(
                &self.paths.launcher_cfg,
                &account.name,
                &account.token,
                &effective,
                &account.launch_options(),
            )
            .is_err()
        {
            return;
        }
        if effective.is_empty() {
            self.launch_log(format_args!(
                "'{}': effective client ПУСТ — настройки НЕ применяются",
                account.name
            ));
        } else {
            self.apply_settings(&account, &effective);
        }
        if !self.launch_and_bind(&account, &effective, true).await {
            self.launch_log(format_args!("'{}': лаунчер НЕ запустился", account.name));
        }
    }

    fn apply_settings(&self, account: &Account, effective: &str) {
        let updates = self.updates();
        let client_dir = updates.join(effective);
        self.launch_log(format_args!(
            "'{}': настройки — updates={}, clientDir={} (существует={})",
            account.name,
            updates.display(),
            client_dir.display(),
            is_dir(&client_dir)
        ));
        if !account.profile.is_empty() && account.profile != MINIMAL_PROFILE {
            apply_profile(&self.paths.profiles, &account.profile, &client_dir);
            self.launch_log(format_args!(
                "'{}': профиль '{}' скопирован в {}",
                account.name,
                account.profile,
                client_dir.display()
            ));
            if self.host.is_staff(&account.name) {
                apply_profile(
                    &self.paths.profiles,
                    &account.profile,
                    &updates.join(STAGING_CLIENT),
                );
            }
        }
        if account.wants_minimal(&account.profile) {
            apply_minimal_options_all(&updates);
            self.launch_log(format_args!(
                "'{}': минимальные настройки применены к клиентам: {:?}",
                account.name,
                list_clients(&updates)
            ));
        } else {
            apply_client_options_all(&updates, account);
            self.launch_log(format_args!(
                "'{}': клиентские настройки (chunks={},fps={},anim={},fastRender={}) применены к: {:?}",
                account.name,
                account.render_distance,
                account.max_fps,
                account.animations,
                account.fast_render,
                list_clients(&updates)
            ));
        }
    }

    async fn launch_and_bind(
        self: &Arc<Self>,
        account: &Account,
        effective: &str,
        verbose: bool,
    ) -> bool {
        let uuid = account.uuid.as_str();
        let mut before = self.tracker.games().game_pids();
        if !Arc::clone(self)
            .launch_for(account, effective, &before)
            .await
        {
            return false;
        }
        self.host.mark_launched(uuid).await;
        let launcher_pid = self.launcher_pid(uuid);
        if launcher_pid != 0 {
            before.push(launcher_pid);
        }
        let queue = Arc::clone(self);
        let uuid = uuid.to_owned();
        let name = account.name.clone();
        let bind = async move {
            let mut pid = 0;
            if launcher_pid != 0 {
                pid = queue
                    .tracker
                    .bind_verified_launcher(&uuid, launcher_pid)
                    .await;
            }
            if pid == 0 {
                pid = queue.tracker.bind_game(&uuid, &before).await;
            }
            queue.titles.kick();
            (queue, name, pid)
        };
        if self.host.config().aggressive {
            tokio::spawn(async move {
                let (queue, name, pid) = bind.await;
                queue.auto_play(&name, pid).await;
            });
        } else {
            if verbose {
                self.launch_log(format_args!(
                    "'{}': обычный режим — ждём поднятия игры перед следующим",
                    account.name
                ));
            }
            let (queue, name, pid) = bind.await;
            tokio::spawn(async move { queue.auto_play(&name, pid).await });
            if verbose {
                self.launch_log(format_args!(
                    "'{}': игра поднялась, можно запускать следующий",
                    account.name
                ));
            }
        }
        true
    }

    async fn auto_play(&self, name: &str, pid: u32) {
        if !self.host.config().auto_play {
            return;
        }
        if pid == 0 {
            self.launch_log(format_args!(
                "'{name}': окно клиента не найдено, ИГРАТЬ не жму"
            ));
            return;
        }
        self.launch_log(format_args!("'{name}': жму ИГРАТЬ (автозапуск)"));
        click_play_for_pid(self.tracker.games(), pid, AUTO_PLAY_TIMEOUT).await;
    }

    async fn launch_for(self: Arc<Self>, account: &Account, client: &str, before: &[u32]) -> bool {
        let config = self.host.config();
        match config.launcher {
            LauncherKind::Custom => {
                self.host
                    .log_unsupported(&account.uuid, &account.name, CUSTOM_NO_LOGS)
                    .await;
                start_launcher(&config.custom_launcher).is_ok()
            }
            LauncherKind::Normal => {
                let exe = self.paths.launcher_exe.clone();
                self.launch_exe(account, client, &exe, LAUNCHER_URL, before)
                    .await
            }
            LauncherKind::New => {
                let exe = self.paths.staff_launcher_exe.clone();
                self.launch_exe(account, client, &exe, STAFF_LAUNCHER_URL, before)
                    .await
            }
            LauncherKind::Jar => {
                if self
                    .downloader
                    .ensure(&self.paths.launcher_jar, JAR_LAUNCHER_URL)
                    .await
                    .is_err()
                {
                    let exe = self.paths.staff_launcher_exe.clone();
                    return self
                        .launch_exe(account, client, &exe, STAFF_LAUNCHER_URL, before)
                        .await;
                }
                self.launch_jar(account, client, before).await
            }
        }
    }

    async fn launch_exe(
        self: &Arc<Self>,
        account: &Account,
        client: &str,
        exe: &Path,
        url: &str,
        before: &[u32],
    ) -> bool {
        if let Err(err) = self.downloader.ensure(exe, url).await {
            self.host
                .log_unsupported(
                    &account.uuid,
                    &account.name,
                    &format!("[AccountChanger] Не удалось подготовить лаунчер: {err}"),
                )
                .await;
            return false;
        }
        let home = self.instance_home(&account.uuid, client).await;
        self.launch_log(format_args!(
            "'{}': exe-лаунчер, exe={}, home={}",
            account.name,
            exe.display(),
            display_opt(home.as_deref())
        ));
        let child = match spawn_launcher(exe, home.as_deref()) {
            Ok(child) => child,
            Err(err) => {
                self.launch_log(format_args!(
                    "'{}': запуск exe-лаунчера НЕ УДАЛСЯ: {err}",
                    account.name
                ));
                self.host
                    .log_unsupported(
                        &account.uuid,
                        &account.name,
                        &format!("[AccountChanger] Не удалось запустить лаунчер: {err}"),
                    )
                    .await;
                return false;
            }
        };
        let launcher_pid = child.id().unwrap_or(0);
        self.track(&account.uuid, child);
        self.launch_log(format_args!(
            "'{}': exe-лаунчер запущен, bootstrap pid={launcher_pid}",
            account.name
        ));
        self.tail_game(account, client, before, launcher_pid);
        true
    }

    async fn launch_jar(self: &Arc<Self>, account: &Account, client: &str, before: &[u32]) -> bool {
        let home = self.instance_home(&account.uuid, client).await;
        let java = resolve_java(&self.paths.cristalix).await;
        self.launch_log(format_args!(
            "'{}': jar-лаунчер, java={}, jar={}, home={}",
            account.name,
            java.display(),
            self.paths.launcher_jar.display(),
            display_opt(home.as_deref())
        ));
        let (child, mut broken) =
            match spawn_launcher_jar(&java, &self.paths.launcher_jar, home.as_deref()) {
                Ok(spawned) => spawned,
                Err(err) => {
                    self.launch_log(format_args!(
                        "'{}': запуск jar-лаунчера НЕ УДАЛСЯ: {err}",
                        account.name
                    ));
                    self.host
                        .log_unsupported(
                            &account.uuid,
                            &account.name,
                            &format!("[AccountChanger] Не удалось запустить лаунчер: {err}"),
                        )
                        .await;
                    return false;
                }
            };
        let launcher_pid = child.id().unwrap_or(0);
        if let Some(kill) = self.track(&account.uuid, child) {
            let queue = Arc::clone(self);
            let uuid = account.uuid.clone();
            let name = account.name.clone();
            tokio::spawn(async move {
                let invalid = tokio::time::timeout(JAR_CHECK_WINDOW, broken.recv())
                    .await
                    .is_ok_and(|signal| signal.is_some());
                if invalid {
                    kill.notify_one();
                    queue.jar_fallback(&uuid, &name, home.as_deref()).await;
                }
            });
        }
        self.launch_log(format_args!(
            "'{}': jar-лаунчер запущен, bootstrap pid={launcher_pid}",
            account.name
        ));
        self.tail_game(account, client, before, launcher_pid);
        true
    }

    async fn jar_fallback(self: &Arc<Self>, uuid: &str, name: &str, home: Option<&Path>) {
        self.launch_log(format_args!(
            "'{name}': jar битый/не запустился, фолбэк на staff exe"
        ));
        let exe = &self.paths.staff_launcher_exe;
        if self
            .downloader
            .ensure(exe, STAFF_LAUNCHER_URL)
            .await
            .is_err()
        {
            return;
        }
        if let Ok(child) = spawn_launcher(exe, home) {
            self.track(uuid, child);
        }
    }

    fn tail_game(
        self: &Arc<Self>,
        account: &Account,
        client: &str,
        before: &[u32],
        launcher_pid: u32,
    ) {
        let queue = Arc::clone(self);
        let account = account.clone();
        let client = client.to_owned();
        let before = before.to_vec();
        tokio::spawn(async move {
            let updates = queue.paths.updates.clone();
            let closed = Arc::new(Notify::new());
            let (ready, found) = oneshot::channel();
            let tail = LogTail {
                host: queue.host.as_ref(),
                updates: &updates,
                client: &client,
                uuid: &account.uuid,
                name: &account.name,
                closed: Arc::clone(&closed),
            };
            let watch_game = async {
                if found.await != Ok(true) {
                    queue.launch_log(format_args!(
                        "'{}': игровой лог НЕ найден в отведённое время",
                        account.name
                    ));
                    return;
                }
                queue.launch_log(format_args!(
                    "'{}': игровой лог найден (клиент запускается)",
                    account.name
                ));
                let mut pid = queue
                    .tracker
                    .bind_verified_launcher(&account.uuid, launcher_pid)
                    .await;
                if pid == 0 {
                    pid = queue.tracker.bind_game(&account.uuid, &before).await;
                }
                queue.launch_log(format_args!(
                    "'{}': игра привязана, game pid={pid} (по launcherPID={launcher_pid})",
                    account.name
                ));
                queue.wipe_instance_token(&account.uuid).await;
                if queue.wait_game_closed(&account.uuid).await {
                    closed.notify_one();
                }
                queue.launch_log(format_args!(
                    "'{}': игра закрыта, инстанс очищен",
                    account.name
                ));
                queue.remove_instance_dir(&account.uuid).await;
            };
            tokio::join!(tail.run(ready), watch_game);
        });
    }

    async fn wait_game_closed(&self, uuid: &str) -> bool {
        let mut seen = false;
        let mut misses = 0;
        let give_up = Instant::now() + GAME_APPEAR_WINDOW;
        loop {
            if self.tracker.resolve().is_running(uuid) {
                seen = true;
                misses = 0;
            } else if seen {
                misses += 1;
                if misses >= 2 {
                    return true;
                }
            } else if Instant::now() > give_up {
                return false;
            }
            tokio::time::sleep(CLOSE_POLL).await;
        }
    }

    async fn instance_home(&self, uuid: &str, client: &str) -> Option<PathBuf> {
        let home = instance_dir(&self.paths.data, uuid);
        let cristalix = home.join(".cristalix");
        std::fs::create_dir_all(&cristalix).ok()?;
        link_shared_dir(
            &cristalix.join("runtime"),
            &self.paths.cristalix.join("runtime"),
        )
        .await;
        let config = cristalix.join(".launcher");
        if let Ok(data) = std::fs::read(&self.paths.launcher_cfg) {
            let _ = std::fs::write(&config, data);
        }
        if let Some(account) = self.host.account(uuid) {
            let effective = self.effective_client(client, &account);
            let options = account.launch_options();
            let _ = apply_account(&config, &account.name, &account.token, &effective, &options);
            self.launch_log(format_args!(
                "instance '{}': токен записан в {} (client={effective}, ram={}, minimal={})",
                account.name,
                config.display(),
                options.ram,
                account.minimal
            ));
        }
        Some(home)
    }

    async fn wipe_instance_token(&self, uuid: &str) {
        let config = instance_config(&instance_dir(&self.paths.data, uuid));
        if config.exists() {
            capture_config(self.host.as_ref(), &config).await;
            let _ = annul_account(&config);
        }
    }

    async fn remove_instance_dir(&self, uuid: &str) {
        let dir = instance_dir(&self.paths.data, uuid);
        capture_config(self.host.as_ref(), &instance_config(&dir)).await;
        remove_link(&dir.join(".cristalix").join("runtime"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    async fn clean_stale_instances(&self) {
        let Ok(entries) = std::fs::read_dir(instances_dir(&self.paths.data)) else {
            return;
        };
        let stale: Vec<String> = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();
        for uuid in stale {
            self.remove_instance_dir(&uuid).await;
        }
    }

    fn update_group(&self, mutate: impl FnOnce(&mut GroupState) -> bool) -> bool {
        let changed = self.group.send_if_modified(mutate);
        if changed {
            self.host.notify(Change::GroupLaunch);
        }
        changed
    }

    pub fn pause_group(&self) -> bool {
        self.update_group(|group| {
            if !group.active || group.paused {
                return false;
            }
            group.paused = true;
            true
        })
    }

    pub fn resume_group(&self) -> bool {
        self.update_group(|group| {
            if !group.active || !group.paused {
                return false;
            }
            group.paused = false;
            true
        })
    }

    pub fn cancel_group(&self) -> bool {
        self.update_group(|group| {
            if !group.active {
                return false;
            }
            group.cancelled = true;
            group.paused = false;
            true
        })
    }

    pub fn group_progress(&self) -> GroupLaunchProgress {
        let group = self.group.borrow();
        GroupLaunchProgress {
            active: group.active,
            paused: group.paused,
            done: group.done,
            total: group.total,
            skipped: group.skips.clone(),
        }
    }

    fn skip(&self, name: &str, reason: &str) {
        self.update_group(|group| {
            group.skips.push(GroupSkip {
                name: name.to_owned(),
                reason: reason.to_owned(),
            });
            true
        });
    }

    pub async fn launch_group(self: Arc<Self>, members: Vec<String>, group_profile: String) {
        let started = self.update_group(|group| {
            if group.active {
                return false;
            }
            *group = GroupState {
                active: true,
                total: members.len() as u32,
                ..GroupState::default()
            };
            true
        });
        if !started {
            return;
        }
        self.launch_log(format_args!(
            "=== ГРУППА: запуск {} аккаунтов (profile={group_profile:?}, aggressive={}) ===",
            members.len(),
            self.host.config().aggressive
        ));
        let has_staff = members.iter().any(|uuid| {
            self.host.account(uuid).is_some_and(|account| {
                !account.name.is_empty() && self.host.is_staff(&account.name)
            })
        });
        let mut applied = HashSet::new();
        let mut progress = self.group.subscribe();
        for uuid in &members {
            let _ = progress
                .wait_for(|group| !group.paused || group.cancelled)
                .await;
            if self.group.borrow().cancelled {
                break;
            }
            self.launch_member(uuid, &group_profile, has_staff, &mut applied)
                .await;
            self.update_group(|group| {
                group.done += 1;
                true
            });
        }
        self.update_group(|group| {
            group.active = false;
            group.paused = false;
            group.cancelled = false;
            group.done = 0;
            group.total = 0;
            true
        });
    }

    async fn launch_member(
        self: &Arc<Self>,
        uuid: &str,
        group_profile: &str,
        has_staff: bool,
        applied: &mut HashSet<PathBuf>,
    ) {
        let Some(account) = self.host.account(uuid).filter(Account::launchable) else {
            self.skip(short_uuid(uuid), msg::GROUP_SKIP_NO_TOKEN);
            return;
        };
        if self.tracker.resolve().is_running(uuid) {
            self.skip(&account.name, msg::GROUP_SKIP_RUNNING);
            return;
        }
        let effective = self.effective_client("", &account);
        ac_platform::clear_stale_locks(&self.paths.cristalix);
        if self.host.config().launcher == LauncherKind::Custom
            && apply_account(
                &self.paths.launcher_cfg,
                &account.name,
                &account.token,
                &effective,
                &account.launch_options(),
            )
            .is_err()
        {
            self.skip(&account.name, msg::GROUP_SKIP_APPLY_FAILED);
            return;
        }
        let profile = if group_profile.is_empty() {
            account.profile.as_str()
        } else {
            group_profile
        };
        let updates = self.updates();
        if !effective.is_empty() {
            let client_dir = updates.join(&effective);
            if !profile.is_empty()
                && profile != MINIMAL_PROFILE
                && applied.insert(client_dir.clone())
            {
                apply_profile(&self.paths.profiles, profile, &client_dir);
                if has_staff {
                    apply_profile(&self.paths.profiles, profile, &updates.join(STAGING_CLIENT));
                }
            }
        }
        if account.wants_minimal(profile) {
            apply_minimal_options_all(&updates);
        } else {
            apply_client_options_all(&updates, &account);
        }
        self.tracker.note_launch(uuid);
        self.launch_log(format_args!(
            "группа: запускаю '{}' (client={effective})",
            account.name
        ));
        if !self.launch_and_bind(&account, &effective, false).await {
            self.launch_log(format_args!(
                "группа: '{}' — лаунчер не запустился",
                account.name
            ));
            self.skip(&account.name, msg::GROUP_SKIP_LAUNCHER_FAILED);
        }
    }
}

fn display_opt(path: Option<&Path>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_default()
}

fn launcher_name(kind: LauncherKind) -> &'static str {
    match kind {
        LauncherKind::Normal => "normal",
        LauncherKind::Jar => "jar",
        LauncherKind::New => "new",
        LauncherKind::Custom => "custom",
    }
}

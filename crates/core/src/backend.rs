use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use ac_api::{
    Account, ApiError, ApiFuture, ApiResult, Backend, BackendEvent, CabinetStatus, Caps, Clients,
    Credentials, CustomLauncher, EventStream, Group, GroupLaunchProgress, ImportProgress,
    LaunchSettings, LauncherKind, LogStats, LogSummary, LogView, OnlineStats, PlayerInfo,
    ProfileFiles, Profiles, QueueEntry, SavedProfile, Settings, Status, TextureKind, Theme,
    UpdateInfo, UpdateProgress, VideoBackground, msg, normalize_label,
};
use ac_launcher::{
    JAR_LAUNCHER_URL, LAUNCHER_URL, MINIMAL_PROFILE, STAFF_LAUNCHER_URL, current_client,
    list_clients, minimal_profile_content, read_profile, resolve_java, sanitize_profile_name,
    save_profile_from_client, start_launcher, start_launcher_jar, updates_dir, write_profile,
};

use crate::convert::{
    self, launch_opts, launcher_kind, store_launcher_kind, store_theme, theme, unix_now,
};
use crate::core::{Core, Inner, launcher_allowed};
use crate::import::{Credential, StartResult};

const MIN_RAM: u32 = 1024;
const BACKGROUND_FILE: &str = "background";

impl Core {
    fn run<F, T>(&self, future: F) -> ApiFuture<T>
    where
        F: Future<Output = ApiResult<T>> + Send + 'static,
        T: Send + 'static,
    {
        let task = self.handle.spawn(future);
        Box::pin(async move {
            task.await
                .unwrap_or_else(|_| Err(ApiError::fixed(msg::GENERIC)))
        })
    }

    fn with<F, Fut, T>(&self, body: F) -> ApiFuture<T>
    where
        F: FnOnce(Arc<Inner>) -> Fut + Send + 'static,
        Fut: Future<Output = ApiResult<T>> + Send + 'static,
        T: Send + 'static,
    {
        let inner = Arc::clone(&self.inner);
        self.run(body(inner))
    }
}

impl Inner {
    fn clamp_ram(&self, ram: i64, minimal: bool) -> i64 {
        if (ram != 0 || minimal) && ram < i64::from(MIN_RAM) {
            i64::from(MIN_RAM)
        } else {
            ram
        }
    }

    fn client_dir(&self, client: &str) -> Option<PathBuf> {
        let client = if client.is_empty() {
            current_client(&self.paths.launcher_cfg)
        } else {
            client.to_owned()
        };
        if client.is_empty() {
            return None;
        }
        Some(updates_dir(&self.paths.launcher_cfg, &self.paths.updates).join(client))
    }

    async fn start_chosen_launcher(&self) -> ApiResult<()> {
        let settings = self.store.settings();
        match launcher_kind(settings.launcher) {
            LauncherKind::Custom => {
                start_launcher(&PathBuf::from(settings.custom_launcher)).map_err(launch_error)
            }
            LauncherKind::Jar => {
                self.downloader
                    .ensure(&self.paths.launcher_jar, JAR_LAUNCHER_URL)
                    .await
                    .map_err(prepare_error)?;
                let java = resolve_java(&self.paths.cristalix).await;
                start_launcher_jar(&java, &self.paths.launcher_jar).map_err(launch_error)
            }
            LauncherKind::New => {
                self.downloader
                    .ensure(&self.paths.staff_launcher_exe, STAFF_LAUNCHER_URL)
                    .await
                    .map_err(prepare_error)?;
                start_launcher(&self.paths.staff_launcher_exe).map_err(launch_error)
            }
            LauncherKind::Normal => {
                self.downloader
                    .ensure(&self.paths.launcher_exe, LAUNCHER_URL)
                    .await
                    .map_err(prepare_error)?;
                start_launcher(&self.paths.launcher_exe).map_err(launch_error)
            }
        }
    }

    fn profiles(&self) -> Profiles {
        let mut profiles = vec![MINIMAL_PROFILE.to_owned()];
        profiles.extend(
            ac_launcher::list_profiles(&self.paths.profiles)
                .into_iter()
                .filter(|name| name != MINIMAL_PROFILE),
        );
        Profiles {
            profiles,
            files: ac_api::PROFILE_FILES
                .iter()
                .map(|&f| f.to_owned())
                .collect(),
            builtin: vec![MINIMAL_PROFILE.to_owned()],
        }
    }

    async fn background_bytes(&self) -> Option<Vec<u8>> {
        let file = self.store.background()?;
        tokio::fs::read(self.paths.data.join(file)).await.ok()
    }

    async fn set_background(&self, source: Option<PathBuf>) -> ApiResult<Option<String>> {
        let Some(source) = source else {
            self.store
                .set_background(None)
                .await
                .map_err(|_| ApiError::fixed(msg::GENERIC))?;
            self.store
                .set_background_dim(ac_api::BACKGROUND_DIM_DEFAULT)
                .await
                .ok();
            self.store.set_background_blur(0).await.ok();
            self.sweep_backgrounds(None).await;
            return Ok(None);
        };
        let extension = source
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("png")
            .to_ascii_lowercase();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis());
        let file = format!("{BACKGROUND_FILE}-{stamp}.{extension}");
        tokio::fs::copy(&source, self.paths.data.join(&file))
            .await
            .map_err(|err| ApiError::new(err.to_string()))?;
        self.store
            .set_background(Some(file.clone()))
            .await
            .map_err(|_| ApiError::fixed(msg::GENERIC))?;
        self.sweep_backgrounds(Some(&file)).await;
        Ok(Some(file))
    }

    async fn sweep_backgrounds(&self, keep: Option<&str>) {
        let Ok(mut entries) = tokio::fs::read_dir(&self.paths.data).await else {
            return;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let ours = name
                .strip_prefix(BACKGROUND_FILE)
                .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('-'));
            if ours && Some(name) != keep {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }

    async fn background_video(&self) -> ApiResult<Option<VideoBackground>> {
        let Some(file) = self
            .store
            .background()
            .filter(|file| ac_api::is_video(file))
        else {
            return Ok(None);
        };
        let ffmpeg = crate::ffmpeg::ensure(&self.paths.data)
            .await
            .map_err(|err| ApiError::new(format!("Не удалось скачать FFmpeg: {err:#}")))?;
        Ok(Some(VideoBackground {
            ffmpeg,
            file: self.paths.data.join(file),
        }))
    }

    async fn close_group(&self, id: &str) -> ApiResult<u32> {
        let Some((members, _)) = self.store.group_members(id) else {
            return Err(ApiError::fixed(msg::GROUP_NOT_FOUND));
        };
        self.queue.cancel_group();
        let running = self.tracker.resolve().running;
        let mut closed = 0;
        for uuid in members {
            self.queue.kill_proc(&uuid);
            if let Some(pid) = running.get(&uuid).copied().filter(|pid| *pid != 0)
                && close_game(pid).await
            {
                closed += 1;
            }
            self.tracker.forget(&uuid);
        }
        Ok(closed)
    }
}

async fn close_game(pid: u32) -> bool {
    tokio::task::spawn_blocking(move || ac_platform::close_game(pid))
        .await
        .unwrap_or(false)
}

fn prepare_error(err: ac_launcher::LauncherError) -> ApiError {
    ApiError::new(msg::launcher_prepare_failed(&err.to_string()))
}

fn launch_error(err: ac_launcher::LauncherError) -> ApiError {
    ApiError::new(msg::launcher_start_failed(&err.to_string()))
}

impl Backend for Core {
    fn caps(&self) -> Caps {
        self.inner.caps
    }

    fn subscribe(&self) -> EventStream {
        self.inner.events.subscribe()
    }

    fn status(&self) -> ApiFuture<Status> {
        self.with(|inner| async move {
            Ok(Status {
                launcher_ready: inner.paths.launcher_exe.exists(),
                data_dir: inner.paths.data.display().to_string(),
            })
        })
    }

    fn accounts(&self) -> ApiFuture<Vec<Account>> {
        self.with(|inner| async move {
            let presence = inner.tracker.resolve();
            let _ = inner
                .store
                .touch_online(&presence.running.keys().cloned().collect::<Vec<_>>())
                .await;
            Ok(inner.account_list(&presence))
        })
    }

    fn launch(&self, uuid: String, client: Option<String>) -> ApiFuture<()> {
        self.with(|inner| async move {
            inner.watcher.capture(inner.host.as_ref()).await;
            let account = inner
                .store
                .get(&uuid)
                .filter(|account| !account.name.is_empty() && !account.token.is_empty());
            let Some(account) = account else {
                return Err(ApiError::fixed(msg::NO_TOKEN));
            };
            if inner.tracker.resolve().is_running(&account.uuid) {
                return Ok(());
            }
            inner
                .queue
                .enqueue(&account.uuid, &client.unwrap_or_default());
            Ok(())
        })
    }

    fn launch_guest(&self) -> ApiFuture<()> {
        self.with(|inner| async move {
            inner.watcher.capture(inner.host.as_ref()).await;
            ac_launcher::annul_account(&inner.paths.launcher_cfg)
                .map_err(|err| ApiError::new(err.to_string()))?;
            inner.start_chosen_launcher().await
        })
    }

    fn open_launcher(&self) -> ApiFuture<()> {
        self.with(|inner| async move { inner.start_chosen_launcher().await })
    }

    fn launch_queue(&self) -> ApiFuture<Vec<QueueEntry>> {
        self.with(|inner| async move {
            let presence = inner.tracker.presence();
            Ok(inner.queue.status(&presence.running))
        })
    }

    fn stop(&self, uuid: String) -> ApiFuture<bool> {
        self.with(|inner| async move {
            inner.queue.kill_proc(&uuid);
            let running = inner.tracker.resolve().running;
            let mut closed = false;
            if let Some(pid) = running.get(&uuid).copied().filter(|pid| *pid != 0) {
                closed = close_game(pid).await;
            }
            inner.tracker.forget(&uuid);
            Ok(closed)
        })
    }

    fn forget(&self, uuid: String) -> ApiFuture<()> {
        self.with(|inner| async move {
            let _ = inner.store.forget(&uuid).await;
            Ok(())
        })
    }

    fn set_label(&self, uuid: String, label: String) -> ApiFuture<()> {
        self.with(move |inner| async move {
            let _ = inner.store.set_label(&uuid, &normalize_label(&label)).await;
            Ok(())
        })
    }

    fn set_pinned(&self, uuid: String, pinned: bool) -> ApiFuture<()> {
        self.with(move |inner| async move {
            let _ = inner.store.set_pinned(&uuid, pinned).await;
            Ok(())
        })
    }

    fn set_account_profile(&self, uuid: String, profile: String) -> ApiFuture<()> {
        self.with(move |inner| async move {
            let _ = inner
                .store
                .set_profile(&uuid, &sanitize_profile_name(&profile))
                .await;
            Ok(())
        })
    }

    fn set_ram(&self, uuids: Vec<String>, ram: u32) -> ApiFuture<()> {
        self.with(move |inner| async move {
            let ram = inner.clamp_ram(i64::from(ram), false);
            let _ = inner.store.set_ram(&uuids, ram).await;
            Ok(())
        })
    }

    fn set_launch_settings(&self, uuids: Vec<String>, settings: LaunchSettings) -> ApiFuture<()> {
        self.with(move |inner| async move {
            let ram = inner.clamp_ram(i64::from(settings.ram), settings.minimal);
            let opts = launch_opts(&settings, ram);
            let _ = inner.store.set_launch_settings(&uuids, opts).await;
            Ok(())
        })
    }

    fn clients(&self) -> ApiFuture<Clients> {
        self.with(|inner| async move {
            let updates = updates_dir(&inner.paths.launcher_cfg, &inner.paths.updates);
            Ok(Clients {
                clients: list_clients(&updates),
                current: current_client(&inner.paths.launcher_cfg),
            })
        })
    }

    fn import_start(&self, accounts: Vec<Credentials>) -> ApiFuture<()> {
        let core = Arc::clone(&self.inner);
        self.with(move |inner| async move {
            if !inner.caps.credential_import {
                return Err(ApiError::fixed(msg::NO_UI_AUTOMATION));
            }
            let credentials = accounts
                .into_iter()
                .map(|credential| Credential {
                    login: credential.login,
                    password: credential.password,
                })
                .collect();
            let valid = valid_logins(&inner);
            match inner
                .import
                .start_import(credentials, &move |login| valid.contains(login))
            {
                StartResult::Started(tasks) => {
                    core.spawn_import(tasks);
                    Ok(())
                }
                StartResult::Rejected(reason) => Err(ApiError::new(reason)),
            }
        })
    }

    fn import_progress(&self) -> ApiFuture<ImportProgress> {
        self.with(|inner| async move { Ok(inner.import.progress()) })
    }

    fn import_cancel(&self) -> ApiFuture<()> {
        self.with(|inner| async move {
            inner.import.request_cancel();
            Ok(())
        })
    }

    fn refresh_start(&self) -> ApiFuture<()> {
        let core = Arc::clone(&self.inner);
        self.with(move |inner| async move {
            if !inner.caps.credential_import {
                return Err(ApiError::fixed(msg::NO_UI_AUTOMATION));
            }
            let now = unix_now();
            let threshold = now + 7 * 24 * 60 * 60;
            let expiring: Vec<(String, String)> = inner
                .store
                .list()
                .into_iter()
                .filter(|account| {
                    !account.token.is_empty() && account.expires > 0 && account.expires <= threshold
                })
                .map(|account| (account.uuid, account.name))
                .collect();
            match inner.import.start_refresh(expiring) {
                StartResult::Started(tasks) => {
                    core.spawn_refresh(tasks);
                    Ok(())
                }
                StartResult::Rejected(reason) => Err(ApiError::new(reason)),
            }
        })
    }

    fn profiles(&self) -> ApiFuture<Profiles> {
        self.with(|inner| async move { Ok(inner.profiles()) })
    }

    fn save_profile(&self, name: String, client: Option<String>) -> ApiFuture<SavedProfile> {
        self.with(move |inner| async move {
            let name = sanitize_profile_name(&name);
            if name.is_empty() {
                return Err(ApiError::fixed(msg::PROFILE_NAME_REQUIRED));
            }
            if name == MINIMAL_PROFILE {
                return Err(ApiError::fixed(msg::PROFILE_NAME_RESERVED));
            }
            let Some(client_dir) = inner.client_dir(&client.unwrap_or_default()) else {
                return Err(ApiError::fixed(msg::PROFILE_NO_CLIENT));
            };
            let copied = save_profile_from_client(&inner.paths.profiles, &name, &client_dir)
                .map_err(|err| ApiError::new(err.to_string()))?;
            if copied == 0 {
                return Err(ApiError::fixed(msg::PROFILE_NOTHING_TO_SAVE));
            }
            inner.events.emit(BackendEvent::ProfilesChanged);
            Ok(SavedProfile { name, copied })
        })
    }

    fn profile_content(&self, name: String) -> ApiFuture<ProfileFiles> {
        self.with(move |inner| async move {
            let name = sanitize_profile_name(&name);
            if name.is_empty() {
                return Err(ApiError::fixed(msg::NO_NAME));
            }
            if name == MINIMAL_PROFILE {
                return Ok(minimal_profile_content());
            }
            Ok(read_profile(&inner.paths.profiles, &name))
        })
    }

    fn update_profile(&self, name: String, files: ProfileFiles) -> ApiFuture<()> {
        self.with(move |inner| async move {
            let name = sanitize_profile_name(&name);
            if name.is_empty() {
                return Err(ApiError::fixed(msg::NO_NAME));
            }
            if name == MINIMAL_PROFILE {
                return Err(ApiError::fixed(msg::PROFILE_BUILTIN_READONLY));
            }
            write_profile(&inner.paths.profiles, &name, &files)
                .map_err(|err| ApiError::new(err.to_string()))?;
            inner.events.emit(BackendEvent::ProfilesChanged);
            Ok(())
        })
    }

    fn delete_profile(&self, name: String) -> ApiFuture<()> {
        self.with(move |inner| async move {
            let name = sanitize_profile_name(&name);
            if name == MINIMAL_PROFILE {
                return Err(ApiError::fixed(msg::PROFILE_BUILTIN_UNDELETABLE));
            }
            if !name.is_empty() {
                let _ = ac_launcher::delete_profile(&inner.paths.profiles, &name);
                inner.events.emit(BackendEvent::ProfilesChanged);
            }
            Ok(())
        })
    }

    fn groups(&self) -> ApiFuture<Vec<Group>> {
        self.with(|inner| async move { Ok(inner.group_list()) })
    }

    fn create_group(&self, name: String) -> ApiFuture<Group> {
        self.with(move |inner| async move {
            match inner.store.create_group(&name).await {
                Ok(Some(group)) => Ok(convert::group(group)),
                _ => Err(ApiError::fixed(msg::GROUP_NAME_EMPTY)),
            }
        })
    }

    fn delete_group(&self, id: String) -> ApiFuture<()> {
        self.with(move |inner| async move {
            let _ = inner.store.delete_group(&id).await;
            Ok(())
        })
    }

    fn rename_group(&self, id: String, name: String) -> ApiFuture<()> {
        self.with(move |inner| async move {
            match inner.store.rename_group(&id, &name).await {
                Ok(true) => Ok(()),
                _ => Err(ApiError::fixed(msg::GROUP_NAME_EMPTY)),
            }
        })
    }

    fn set_group_pinned(&self, id: String, pinned: bool) -> ApiFuture<()> {
        self.with(move |inner| async move {
            let _ = inner.store.set_group_pinned(&id, pinned).await;
            Ok(())
        })
    }

    fn reorder_groups(&self, ids: Vec<String>) -> ApiFuture<()> {
        self.with(move |inner| async move {
            let _ = inner.store.reorder_groups(&ids).await;
            Ok(())
        })
    }

    fn add_group_member(&self, id: String, uuid: String) -> ApiFuture<()> {
        self.with(move |inner| async move {
            let _ = inner.store.add_group_member(&id, &uuid).await;
            Ok(())
        })
    }

    fn remove_group_member(&self, id: String, uuid: String) -> ApiFuture<()> {
        self.with(move |inner| async move {
            let _ = inner.store.remove_group_member(&id, &uuid).await;
            Ok(())
        })
    }

    fn set_group_profile(&self, id: String, profile: String) -> ApiFuture<()> {
        self.with(move |inner| async move {
            let _ = inner.store.set_group_profile(&id, &profile).await;
            Ok(())
        })
    }

    fn launch_group(&self, id: String) -> ApiFuture<()> {
        let core = Arc::clone(&self.inner);
        self.with(move |inner| async move {
            let Some((members, profile)) = inner.store.group_members(&id) else {
                return Err(ApiError::fixed(msg::GROUP_NOT_FOUND));
            };
            if members.is_empty() {
                return Err(ApiError::fixed(msg::GROUP_EMPTY));
            }
            inner.watcher.capture(inner.host.as_ref()).await;
            let queue = Arc::clone(&core.queue);
            tokio::spawn(queue.launch_group(members, profile));
            Ok(())
        })
    }

    fn close_group(&self, id: String) -> ApiFuture<u32> {
        self.with(move |inner| async move { inner.close_group(&id).await })
    }

    fn pause_group_launch(&self) -> ApiFuture<bool> {
        self.with(|inner| async move { Ok(inner.queue.pause_group()) })
    }

    fn resume_group_launch(&self) -> ApiFuture<bool> {
        self.with(|inner| async move { Ok(inner.queue.resume_group()) })
    }

    fn group_launch_progress(&self) -> ApiFuture<GroupLaunchProgress> {
        self.with(|inner| async move { Ok(inner.queue.group_progress()) })
    }

    fn settings(&self) -> ApiFuture<Settings> {
        self.with(|inner| async move {
            let settings = inner.store.settings();
            Ok(Settings {
                autostart: ac_platform::autostart_enabled(),
                launcher: launcher_kind(settings.launcher),
                custom_launcher: settings.custom_launcher,
                auto_play: settings.auto_play,
                stats: settings.stats,
                aggressive: settings.aggressive,
                window_title: settings.window_title,
                theme: theme(settings.theme),
                sound: settings.sound,
                background: ac_api::Background {
                    file: settings.background,
                    dim: settings.background_dim,
                    blur: settings.background_blur,
                },
            })
        })
    }

    fn set_autostart(&self, enabled: bool) -> ApiFuture<bool> {
        self.run(async move {
            ac_platform::set_autostart(enabled).map_err(|err| ApiError::new(err.to_string()))?;
            Ok(ac_platform::autostart_enabled())
        })
    }

    fn set_launcher(&self, launcher: LauncherKind) -> ApiFuture<LauncherKind> {
        self.with(move |inner| async move {
            if !launcher_allowed(inner.caps, launcher) {
                return Err(ApiError::fixed(msg::LAUNCHER_WINDOWS_ONLY));
            }
            if !inner
                .store
                .set_launcher(store_launcher_kind(launcher))
                .await
                .unwrap_or(false)
            {
                return Err(ApiError::fixed(msg::LAUNCHER_UNKNOWN));
            }
            Ok(launcher_kind(inner.store.launcher()))
        })
    }

    fn set_custom_launcher(&self, path: PathBuf) -> ApiFuture<CustomLauncher> {
        self.with(move |inner| async move {
            let path = path.to_string_lossy().into_owned();
            if !inner
                .store
                .set_custom_launcher(&path)
                .await
                .unwrap_or(false)
            {
                return Err(ApiError::fixed(msg::CUSTOM_LAUNCHER_REQUIRED));
            }
            let settings = inner.store.settings();
            Ok(CustomLauncher {
                launcher: launcher_kind(settings.launcher),
                custom_launcher: settings.custom_launcher,
            })
        })
    }

    fn set_auto_play(&self, enabled: bool) -> ApiFuture<bool> {
        self.with(move |inner| async move {
            let _ = inner.store.set_auto_play(enabled).await;
            Ok(inner.store.auto_play())
        })
    }

    fn set_aggressive(&self, enabled: bool) -> ApiFuture<bool> {
        self.with(move |inner| async move {
            let _ = inner.store.set_aggressive_launch(enabled).await;
            Ok(inner.store.aggressive_launch())
        })
    }

    fn set_window_title(&self, enabled: bool) -> ApiFuture<bool> {
        self.with(move |inner| async move {
            let _ = inner.store.set_window_title(enabled).await;
            Ok(inner.store.window_title())
        })
    }

    fn set_stats(&self, enabled: bool) -> ApiFuture<bool> {
        self.with(move |inner| async move {
            let _ = inner.store.set_stats(enabled).await;
            let enabled = inner.store.stats();
            inner
                .events
                .emit(BackendEvent::OnlineStats(inner.online_stats().await));
            Ok(enabled)
        })
    }

    fn set_theme(&self, theme: Theme) -> ApiFuture<()> {
        self.with(move |inner| async move {
            let _ = inner.store.set_theme(store_theme(theme)).await;
            Ok(())
        })
    }

    fn set_sound(&self, enabled: bool) -> ApiFuture<()> {
        self.with(move |inner| async move {
            let _ = inner.store.set_sound(enabled).await;
            Ok(())
        })
    }

    fn set_background(&self, source: Option<PathBuf>) -> ApiFuture<Option<String>> {
        self.with(move |inner| async move { inner.set_background(source).await })
    }

    fn set_background_dim(&self, dim: u8) -> ApiFuture<()> {
        self.with(move |inner| async move {
            let _ = inner.store.set_background_dim(dim).await;
            Ok(())
        })
    }

    fn set_background_blur(&self, blur: u8) -> ApiFuture<()> {
        self.with(move |inner| async move {
            let _ = inner.store.set_background_blur(blur).await;
            Ok(())
        })
    }

    fn background_image(&self) -> ApiFuture<Option<Vec<u8>>> {
        self.with(|inner| async move { Ok(inner.background_bytes().await) })
    }

    fn background_video(&self) -> ApiFuture<Option<VideoBackground>> {
        self.with(|inner| async move { inner.background_video().await })
    }

    fn cabinet_status(&self, uuid: String) -> ApiFuture<CabinetStatus> {
        self.with(move |inner| async move {
            let session = inner
                .store
                .cabinet_session(&uuid)
                .await
                .map_err(|_| ApiError::fixed(msg::GENERIC))?
                .ok_or_else(|| ApiError::fixed(msg::CABINET_SESSION))?;
            match crate::cabinet::fetch_status(&inner.cabinet_http, &session).await {
                Ok(status) => Ok(status),
                Err(err) => {
                    if err.message() == msg::CABINET_SESSION {
                        inner.store.set_cabinet_session(&uuid, "").await.ok();
                    }
                    Err(err)
                }
            }
        })
    }

    fn has_cabinet_session(&self, uuid: String) -> ApiFuture<bool> {
        self.with(move |inner| async move {
            Ok(inner
                .store
                .cabinet_session(&uuid)
                .await
                .map(|session| session.is_some())
                .unwrap_or(false))
        })
    }

    fn set_cabinet_session(&self, uuid: String, session: String) -> ApiFuture<()> {
        self.with(move |inner| async move {
            inner
                .store
                .set_cabinet_session(&uuid, &session)
                .await
                .map_err(|_| ApiError::fixed(msg::GENERIC))
        })
    }

    fn clear_cabinet_session(&self, uuid: String) -> ApiFuture<()> {
        self.with(move |inner| async move {
            inner
                .store
                .set_cabinet_session(&uuid, "")
                .await
                .map_err(|_| ApiError::fixed(msg::GENERIC))
        })
    }

    fn account_password(&self, uuid: String) -> ApiFuture<Option<String>> {
        self.with(move |inner| async move { Ok(inner.store.password(&uuid).await.unwrap_or(None)) })
    }

    fn set_account_password(&self, uuid: String, password: String) -> ApiFuture<()> {
        self.with(move |inner| async move {
            inner
                .store
                .set_password(&uuid, &password)
                .await
                .map_err(|_| ApiError::fixed(msg::GENERIC))
        })
    }

    fn cabinet_sessions(&self) -> ApiFuture<Vec<String>> {
        self.with(
            |inner| async move { Ok(inner.store.cabinet_sessions().await.unwrap_or_default()) },
        )
    }

    fn online_stats(&self) -> ApiFuture<OnlineStats> {
        self.with(|inner| async move { Ok(inner.online_stats().await) })
    }

    fn logs(&self) -> ApiFuture<Vec<LogSummary>> {
        self.with(|inner| async move {
            Ok(inner
                .store
                .log_summaries()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(convert::log_summary)
                .collect())
        })
    }

    fn log_stats(&self) -> ApiFuture<LogStats> {
        self.with(|inner| async move {
            Ok(convert::log_stats(
                inner.store.log_stats().await.unwrap_or_default(),
            ))
        })
    }

    fn log(&self, uuid: String, session: Option<String>) -> ApiFuture<LogView> {
        self.with(move |inner| async move {
            let session_id = session.and_then(|id| id.parse::<i64>().ok());
            let Some(lines) = inner
                .store
                .log_lines(&uuid, session_id)
                .await
                .ok()
                .flatten()
            else {
                return Ok(LogView::default());
            };
            let sessions = inner.store.log_sessions(&uuid).await.unwrap_or_default();
            Ok(LogView {
                lines: lines.into_iter().map(convert::log_line).collect(),
                sessions: sessions.into_iter().map(convert::log_session).collect(),
            })
        })
    }

    fn logs_size(&self) -> ApiFuture<u64> {
        self.with(|inner| async move {
            Ok(inner.store.log_size_all().await.unwrap_or_default().max(0) as u64)
        })
    }

    fn clear_logs(&self) -> ApiFuture<()> {
        self.with(|inner| async move {
            let _ = inner.store.clear_all_logs().await;
            inner.events.emit(BackendEvent::LogsChanged);
            Ok(())
        })
    }

    fn delete_log_session(&self, uuid: String, session: String) -> ApiFuture<()> {
        self.with(move |inner| async move {
            if let Ok(id) = session.parse::<i64>() {
                let _ = inner.store.delete_log_session(&uuid, id).await;
                inner.events.emit(BackendEvent::LogsChanged);
            }
            Ok(())
        })
    }

    fn check_update(&self) -> ApiFuture<UpdateInfo> {
        self.with(|inner| async move { Ok(convert::update_info(inner.updater.check().await)) })
    }

    fn apply_update(&self) -> ApiFuture<()> {
        let handle = self.handle.clone();
        self.with(move |inner| async move {
            if !inner.updater.check().await.available {
                return Err(ApiError::fixed(msg::NO_UPDATE));
            }
            let events = inner.events.clone();
            inner.updater.start(&handle, move || {
                events.emit(BackendEvent::QuitRequested);
            });
            Ok(())
        })
    }

    fn update_progress(&self) -> ApiFuture<UpdateProgress> {
        self.with(|inner| async move { Ok(convert::update_progress(inner.updater.progress())) })
    }

    fn player(&self, name: String, fresh: bool) -> ApiFuture<Option<PlayerInfo>> {
        self.with(move |inner| async move {
            if fresh {
                inner.players.invalidate(&name);
            }
            Ok(inner
                .players
                .fetch(&name)
                .await
                .map(|info| convert::player(&info)))
        })
    }

    fn texture(&self, uuid: String, kind: TextureKind) -> ApiFuture<Option<Vec<u8>>> {
        self.with(move |inner| async move {
            Ok(inner
                .textures
                .fetch(convert::texture_kind(kind), &uuid)
                .await
                .ok()
                .map(|bytes| bytes.to_vec()))
        })
    }

    fn focus(&self) -> ApiFuture<()> {
        self.with(|inner| async move {
            inner.events.emit(BackendEvent::FocusRequested);
            Ok(())
        })
    }

    fn quit(&self) -> ApiFuture<()> {
        self.with(|inner| async move {
            inner.events.emit(BackendEvent::QuitRequested);
            Ok(())
        })
    }
}

fn valid_logins(inner: &Inner) -> std::collections::HashSet<String> {
    let now = unix_now();
    inner
        .store
        .list()
        .into_iter()
        .filter(|account| {
            !account.name.is_empty()
                && !account.token.is_empty()
                && !convert::is_expired(account.expires, now)
        })
        .map(|account| account.name.to_lowercase())
        .collect()
}

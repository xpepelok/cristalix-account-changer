use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use ac_api::{
    Account, ApiFuture, Backend, BackendEvent, Caps, Group, GroupLaunchProgress, ImportProgress,
    OnlineStats, PlayerInfo, Presence, Profiles, QueueEntry, Rank, Settings, Status, TextureKind,
    UpdateInfo, UpdateProgress,
};
use futures::StreamExt;
use gpui::{App, AppContext, Context, Entity, EventEmitter, RenderImage, Task};

use crate::format::{now_millis, parse_iso_millis, span};
use crate::skin::{Skins, Texture};

pub const TOAST_LIFETIME: Duration = Duration::from_millis(2600);
const PLAYER_REFRESH: Duration = Duration::from_secs(60);
const UPDATE_RECHECK: Duration = Duration::from_secs(30 * 60);
const LAUNCH_DEBOUNCE: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub id: u64,
    pub text: String,
    pub error: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OnlineStatus {
    pub text: String,
    pub on: bool,
    pub hidden: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreEvent {
    Focus,
    LogsChanged,
    Quit,
}

pub struct Store {
    pub backend: Arc<dyn Backend>,
    pub caps: Caps,
    pub status: Status,
    pub accounts: Vec<Account>,
    pub groups: Vec<Group>,
    pub profiles: Profiles,
    pub settings: Settings,
    pub settings_loaded: bool,
    pub queue: Vec<QueueEntry>,
    pub queue_peak: usize,
    pub group_launch: GroupLaunchProgress,
    pub import: ImportProgress,
    pub update: Option<UpdateInfo>,
    pub update_progress: Option<UpdateProgress>,
    pub online: Option<OnlineStats>,
    pub accounts_loaded: bool,
    players: HashMap<String, Option<PlayerInfo>>,
    pending_players: HashSet<String>,
    launching: HashSet<String>,
    pub skins: Skins,
    pub toast: Option<Toast>,
    toast_seq: u64,
    _tasks: Vec<Task<()>>,
}

impl EventEmitter<StoreEvent> for Store {}

pub fn player_key(name: &str) -> String {
    name.to_lowercase()
}

impl Store {
    pub fn new(backend: Arc<dyn Backend>, cx: &mut Context<Self>) -> Self {
        let caps = backend.caps();
        let mut store = Self {
            backend,
            caps,
            status: Status::default(),
            accounts: Vec::new(),
            groups: Vec::new(),
            profiles: Profiles::default(),
            settings: Settings::default(),
            settings_loaded: false,
            queue: Vec::new(),
            queue_peak: 0,
            group_launch: GroupLaunchProgress::default(),
            import: ImportProgress::default(),
            update: None,
            update_progress: None,
            online: None,
            accounts_loaded: false,
            players: HashMap::new(),
            pending_players: HashSet::new(),
            launching: HashSet::new(),
            skins: Skins::default(),
            toast: None,
            toast_seq: 0,
            _tasks: Vec::new(),
        };
        store._tasks.push(store.listen(cx));
        store._tasks.push(store.refresh_players_periodically(cx));
        store._tasks.push(store.recheck_updates(cx));
        store.load_all(cx);
        store
    }

    fn listen(&self, cx: &mut Context<Self>) -> Task<()> {
        let mut events = self.backend.subscribe();
        cx.spawn(async move |this, cx| {
            while let Some(event) = events.next().await {
                if this.update(cx, |store, cx| store.apply(event, cx)).is_err() {
                    break;
                }
            }
        })
    }

    fn refresh_players_periodically(&self, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(PLAYER_REFRESH).await;
                if this
                    .update(cx, |store, cx| store.refresh_players(cx))
                    .is_err()
                {
                    break;
                }
            }
        })
    }

    fn recheck_updates(&self, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(UPDATE_RECHECK).await;
                if this.update(cx, |store, cx| store.check_update(cx)).is_err() {
                    break;
                }
            }
        })
    }

    fn load_all(&mut self, cx: &mut Context<Self>) {
        self.fetch(cx, self.backend.status(), |store, status, cx| {
            store.status = status;
            cx.notify();
        });
        self.fetch(cx, self.backend.settings(), |store, settings, cx| {
            store.apply_settings(settings, cx);
        });
        self.reload_accounts(cx);
        self.reload_groups(cx);
        self.reload_profiles(cx);
        self.fetch(cx, self.backend.launch_queue(), |store, queue, cx| {
            store.set_queue(queue, cx);
        });
        self.fetch(cx, self.backend.import_progress(), |store, progress, cx| {
            store.import = progress;
            cx.notify();
        });
        self.fetch(
            cx,
            self.backend.group_launch_progress(),
            |store, progress, cx| {
                store.group_launch = progress;
                cx.notify();
            },
        );
        self.fetch(cx, self.backend.online_stats(), |store, stats, cx| {
            store.online = Some(stats);
            cx.notify();
        });
        self.check_update(cx);
    }

    fn apply_settings(&mut self, settings: Settings, cx: &mut Context<Self>) {
        let theme_changed = settings.theme != self.settings.theme || !self.settings_loaded;
        self.settings = settings;
        self.settings_loaded = true;
        crate::sound::set_enabled(self.settings.sound);
        if theme_changed {
            crate::theme::install(self.settings.theme, cx);
            cx.refresh_windows();
        }
        cx.notify();
    }

    pub fn check_update(&mut self, cx: &mut Context<Self>) {
        self.fetch(cx, self.backend.check_update(), |store, info, cx| {
            if info.available && store.update.as_ref().map(|u| &u.latest) != Some(&info.latest) {
                store.toast(format!("Доступно обновление {}", info.latest), false, cx);
            }
            store.update = Some(info);
            cx.notify();
        });
    }

    pub fn reload_accounts(&mut self, cx: &mut Context<Self>) {
        self.fetch(cx, self.backend.accounts(), |store, accounts, cx| {
            store.set_accounts(accounts, cx);
        });
    }

    pub fn reload_groups(&mut self, cx: &mut Context<Self>) {
        self.fetch(cx, self.backend.groups(), |store, groups, cx| {
            store.groups = groups;
            cx.notify();
        });
    }

    pub fn reload_profiles(&mut self, cx: &mut Context<Self>) {
        self.fetch(cx, self.backend.profiles(), |store, profiles, cx| {
            store.profiles = profiles;
            cx.notify();
        });
    }

    fn set_accounts(&mut self, accounts: Vec<Account>, cx: &mut Context<Self>) {
        let names: Vec<String> = accounts
            .iter()
            .filter(|fresh| {
                self.accounts
                    .iter()
                    .any(|old| old.uuid == fresh.uuid && old.running != fresh.running)
            })
            .map(|account| account.name.clone())
            .filter(|name| !name.is_empty())
            .collect();
        self.accounts = accounts;
        self.accounts_loaded = true;
        for name in names {
            self.load_player(name, true, cx);
        }
        let unknown: Vec<String> = self
            .accounts
            .iter()
            .map(|account| account.name.clone())
            .filter(|name| !name.is_empty() && !self.players.contains_key(&player_key(name)))
            .collect();
        for name in unknown {
            self.ensure_player(&name, cx);
        }
        cx.notify();
    }

    fn set_queue(&mut self, queue: Vec<QueueEntry>, cx: &mut Context<Self>) {
        self.queue_peak = if queue.is_empty() {
            0
        } else {
            self.queue_peak.max(queue.len())
        };
        self.queue = queue;
        cx.notify();
    }

    fn apply(&mut self, event: BackendEvent, cx: &mut Context<Self>) {
        match event {
            BackendEvent::Accounts(accounts) => self.set_accounts(accounts, cx),
            BackendEvent::Groups(groups) => {
                self.groups = groups;
                cx.notify();
            }
            BackendEvent::ProfilesChanged => self.reload_profiles(cx),
            BackendEvent::LaunchQueue(queue) => self.set_queue(queue, cx),
            BackendEvent::GroupLaunch(progress) => {
                self.group_launch = progress;
                cx.notify();
            }
            BackendEvent::Import(progress) => {
                let finished = self.import.running && !progress.running;
                self.import = progress;
                if finished {
                    self.reload_accounts(cx);
                }
                cx.notify();
            }
            BackendEvent::UpdateAvailable(info) => {
                if info.available {
                    self.toast(format!("Доступно обновление {}", info.latest), false, cx);
                }
                self.update = Some(info);
                cx.notify();
            }
            BackendEvent::Update(progress) => self.apply_update_progress(progress, cx),
            BackendEvent::OnlineStats(stats) => {
                self.online = Some(stats);
                cx.notify();
            }
            BackendEvent::LogsChanged => cx.emit(StoreEvent::LogsChanged),
            BackendEvent::FocusRequested => cx.emit(StoreEvent::Focus),
            BackendEvent::QuitRequested => cx.emit(StoreEvent::Quit),
        }
    }

    fn apply_update_progress(&mut self, progress: UpdateProgress, cx: &mut Context<Self>) {
        if !progress.error.is_empty() {
            self.toast(progress.error.clone(), true, cx);
        } else if progress.done {
            self.toast("Обновление установлено, перезапуск…", false, cx);
        }
        self.update_progress = Some(progress);
        cx.notify();
    }

    pub fn fetch<T: 'static>(
        &mut self,
        cx: &mut Context<Self>,
        future: ApiFuture<T>,
        done: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
    ) {
        cx.spawn(async move |this, cx| {
            let result = future.await;
            this.update(cx, |store, cx| match result {
                Ok(value) => done(store, value, cx),
                Err(error) => store.toast(error.message().to_owned(), true, cx),
            })
            .ok();
        })
        .detach();
    }

    pub fn act(
        &mut self,
        cx: &mut Context<Self>,
        future: ApiFuture<()>,
        success: impl Into<String>,
    ) {
        let success = success.into();
        self.fetch(cx, future, move |store, (), cx| {
            if !success.is_empty() {
                store.toast(success, false, cx);
            }
        });
    }

    pub fn toast(&mut self, text: impl Into<String>, error: bool, cx: &mut Context<Self>) {
        self.toast_seq += 1;
        let id = self.toast_seq;
        self.toast = Some(Toast {
            id,
            text: text.into(),
            error,
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST_LIFETIME).await;
            this.update(cx, |store, cx| {
                if store.toast.as_ref().is_some_and(|toast| toast.id == id) {
                    store.toast = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn account(&self, uuid: &str) -> Option<&Account> {
        self.accounts.iter().find(|account| account.uuid == uuid)
    }

    pub fn group(&self, id: &str) -> Option<&Group> {
        self.groups.iter().find(|group| group.id == id)
    }

    pub fn player(&self, name: &str) -> Option<&PlayerInfo> {
        if name.is_empty() {
            return None;
        }
        self.players.get(&player_key(name)).and_then(Option::as_ref)
    }

    pub fn player_known(&self, name: &str) -> bool {
        !name.is_empty() && self.players.contains_key(&player_key(name))
    }

    pub fn rank(&self, account: &Account) -> Option<Rank> {
        self.player(&account.name)
            .and_then(PlayerInfo::primary_rank)
    }

    pub fn display_rank(&self, account: &Account) -> Option<Rank> {
        if !self.player_known(&account.name) {
            return None;
        }
        Some(self.rank(account).unwrap_or_else(Rank::player))
    }

    pub fn ensure_player(&mut self, name: &str, cx: &mut Context<Self>) {
        if name.is_empty() {
            return;
        }
        let key = player_key(name);
        if self.players.contains_key(&key) || self.pending_players.contains(&key) {
            return;
        }
        self.load_player(name.to_owned(), false, cx);
    }

    fn load_player(&mut self, name: String, fresh: bool, cx: &mut Context<Self>) {
        let key = player_key(&name);
        if !self.pending_players.insert(key.clone()) {
            return;
        }
        let future = self.backend.player(name, fresh);
        cx.spawn(async move |this, cx| {
            let result = future.await;
            this.update(cx, |store, cx| {
                store.pending_players.remove(&key);
                match result {
                    Ok(Some(info)) if !info.name.is_empty() => {
                        store.players.insert(key, Some(info));
                    }
                    _ => {
                        store.players.entry(key).or_insert(None);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn refresh_players(&mut self, cx: &mut Context<Self>) {
        let names: HashSet<String> = self
            .accounts
            .iter()
            .filter(|account| !account.name.is_empty())
            .map(|account| account.name.clone())
            .collect();
        for name in names {
            self.load_player(name, false, cx);
        }
    }

    pub fn head(
        &mut self,
        uuid: &str,
        size: u32,
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        if self.skins.begin(uuid) {
            let owner = uuid.to_owned();
            let future = self.backend.texture(owner.clone(), TextureKind::Skin);
            cx.spawn(async move |this, cx| {
                let bytes = future.await.ok().flatten();
                this.update(cx, |store, cx| {
                    store.skins.finish(owner, bytes);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        self.skins.head(uuid, size)
    }

    pub fn skin_texture(&self, uuid: &str) -> Option<Texture> {
        self.skins.texture(uuid).cloned()
    }

    pub fn online_status(&self, account: &Account) -> Option<OnlineStatus> {
        online_status(self.player(&account.name), account, now_millis())
    }

    pub fn begin_launch(&mut self, uuid: &str, cx: &mut Context<Self>) -> bool {
        if !self.launching.insert(uuid.to_owned()) {
            return false;
        }
        let owner = uuid.to_owned();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(LAUNCH_DEBOUNCE).await;
            this.update(cx, |store, _| {
                store.launching.remove(&owner);
            })
            .ok();
        })
        .detach();
        true
    }

    pub fn toggle_play(&mut self, uuid: &str, cx: &mut Context<Self>) {
        let Some(account) = self.account(uuid).cloned() else {
            return;
        };
        if account.launching {
            return;
        }
        let name = if account.name.is_empty() {
            None
        } else {
            Some(account.name.clone())
        };
        if account.running {
            let label = name.unwrap_or_else(|| "аккаунт".to_owned());
            self.fetch(
                cx,
                self.backend.stop(account.uuid.clone()),
                move |store, _, cx| {
                    store.toast(format!("Закрываю {label}…"), false, cx);
                },
            );
            return;
        }
        if !account.launchable {
            self.toast("Нет сохранённого токена для этого аккаунта", true, cx);
            return;
        }
        if !self.begin_launch(&account.uuid, cx) {
            return;
        }
        let client = (!account.client.is_empty()).then(|| account.client.clone());
        let label = name.unwrap_or_else(|| "аккаунт".to_owned());
        self.act(
            cx,
            self.backend.launch(account.uuid, client),
            format!("Запускаю {label}…"),
        );
    }

    pub fn stop_launch(&mut self, uuid: &str, cx: &mut Context<Self>) {
        let Some(account) = self.account(uuid) else {
            return;
        };
        let label = if account.name.is_empty() {
            "аккаунта".to_owned()
        } else {
            account.name.clone()
        };
        self.fetch(
            cx,
            self.backend.stop(uuid.to_owned()),
            move |store, _, cx| {
                store.toast(format!("Останавливаю запуск {label}…"), false, cx);
            },
        );
    }

    pub fn toggle_pin(&mut self, uuid: &str, cx: &mut Context<Self>) {
        let Some(account) = self.account(uuid) else {
            return;
        };
        let future = self.backend.set_pinned(uuid.to_owned(), !account.pinned);
        self.act(cx, future, "");
    }
}

pub fn online_status(
    info: Option<&PlayerInfo>,
    account: &Account,
    now: i64,
) -> Option<OnlineStatus> {
    let info = info?;
    let seen = parse_iso_millis(&info.last_seen);
    let local = account.last_online * 1000;
    let status = |text: String, on: bool, hidden: bool| Some(OnlineStatus { text, on, hidden });
    match info.online {
        Presence::Online => {
            let realm = if info.realm.is_empty() {
                String::new()
            } else {
                format!(" | {}", info.realm)
            };
            let since = seen.map_or(String::new(), |at| format!(" {}", span(now - at)));
            status(format!("В сети{since}{realm}"), true, false)
        }
        Presence::Offline => {
            let off = seen.unwrap_or(0).max(local);
            let since = if off > 0 {
                format!(" {}", span(now - off))
            } else {
                String::new()
            };
            status(format!("Не в сети{since}"), false, false)
        }
        Presence::Hidden if account.running => {
            let base = seen.unwrap_or(0).max(account.last_launched * 1000);
            let since = if base > 0 {
                format!(" {}", span(now - base))
            } else {
                String::new()
            };
            status(format!("В сети{since}"), true, false)
        }
        Presence::Hidden if local == 0 => status("Статус скрыт".to_owned(), false, true),
        Presence::Hidden => status(format!("Не в сети {}", span(now - local)), false, false),
        Presence::Unknown => None,
    }
}

pub fn new_store(backend: Arc<dyn Backend>, cx: &mut App) -> Entity<Store> {
    cx.new(|cx| Store::new(backend, cx))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account() -> Account {
        Account {
            uuid: "u".into(),
            name: "n".into(),
            last_online: 100,
            ..Account::default()
        }
    }

    #[test]
    fn statuses_follow_presence() {
        let mut info = PlayerInfo {
            online: Presence::Online,
            realm: "Anarchy".into(),
            ..PlayerInfo::default()
        };
        let online = online_status(Some(&info), &account(), 1_000_000).unwrap();
        assert_eq!(online.text, "В сети | Anarchy");
        assert!(online.on);

        info.online = Presence::Offline;
        let offline = online_status(Some(&info), &account(), 160_000).unwrap();
        assert_eq!(offline.text, "Не в сети 1м");

        info.online = Presence::Hidden;
        let mut hidden_account = account();
        hidden_account.last_online = 0;
        let hidden = online_status(Some(&info), &hidden_account, 1).unwrap();
        assert!(hidden.hidden);
        assert_eq!(hidden.text, "Статус скрыт");

        info.online = Presence::Unknown;
        assert!(online_status(Some(&info), &account(), 1).is_none());
        assert!(online_status(None, &account(), 1).is_none());
    }
}

use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use ac_api::{
    Account, ApiError, ApiFuture, ApiResult, Backend, BackendEvent, Background, Caps, Clients,
    Credentials, CustomLauncher, EventStream, Group, GroupLaunchProgress, GroupSkip, ImportItem,
    ImportProgress, ImportStatus, LaunchSettings, LauncherKind, LogLine, LogSession, LogStats,
    LogSummary, LogView, MINIMAL_PROFILE, OnlineCounts, OnlineStats, PROFILE_FILES, PlayerInfo,
    Presence, ProfileFiles, Profiles, QueueEntry, QueueState, SavedProfile, Settings, Status,
    TextureKind, Theme, UpdateInfo, UpdateProgress, msg, normalize_label,
};
use futures::StreamExt;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use image::{Rgba, RgbaImage};
use parking_lot::Mutex;

use crate::format::now_secs;

const NAMES: [&str; 16] = [
    "xpepelok",
    "Pepel_ok",
    "xpepelokLegend",
    "xpepelokIsBest",
    "xpepelokIsCute",
    "xpepelokPerfect",
    "xpepelokUnique",
    "xpepelokGreatest",
    "xpepelokTalented",
    "xpepelokGenius",
    "xpepelokCharming",
    "xpepelokPowerful",
    "xpepelokPretty",
    "xpepelokAmazing",
    "equeque",
    "Grifon4ik",
];

struct World {
    accounts: Vec<Account>,
    groups: Vec<Group>,
    profiles: BTreeMap<String, ProfileFiles>,
    settings: Settings,
    queue: Vec<QueueEntry>,
    group_launch: GroupLaunchProgress,
    import: ImportProgress,
    import_cancel: bool,
    logs: Vec<(String, Vec<LogSession>)>,
    background: Option<Vec<u8>>,
    subscribers: Vec<UnboundedSender<BackendEvent>>,
}

impl World {
    fn emit(&mut self, event: BackendEvent) {
        self.subscribers
            .retain(|sender| sender.unbounded_send(event.clone()).is_ok());
    }

    fn emit_accounts(&mut self) {
        let accounts = self.accounts.clone();
        self.emit(BackendEvent::Accounts(accounts));
    }

    fn emit_groups(&mut self) {
        let groups = self.groups.clone();
        self.emit(BackendEvent::Groups(groups));
    }

    fn account(&mut self, uuid: &str) -> Option<&mut Account> {
        self.accounts.iter_mut().find(|a| a.uuid == uuid)
    }

    fn group(&mut self, id: &str) -> ApiResult<&mut Group> {
        self.groups
            .iter_mut()
            .find(|g| g.id == id)
            .ok_or(ApiError::fixed(msg::GROUP_NOT_FOUND))
    }
}

pub struct MockBackend {
    world: Arc<Mutex<World>>,
}

fn uuid_for(index: usize) -> String {
    let seed = 0x9e37_79b9_7f4a_7c15_u64.wrapping_mul(index as u64 + 7);
    format!(
        "{:08x}-{:04x}-4{:03x}-a{:03x}-{:012x}",
        (seed >> 32) as u32,
        (seed >> 16) as u16,
        (seed >> 4) as u16 & 0xfff,
        seed as u16 & 0xfff,
        seed.rotate_left(17) & 0xffff_ffff_ffff
    )
}

fn seed_accounts() -> Vec<Account> {
    let now = now_secs();
    NAMES
        .iter()
        .enumerate()
        .map(|(index, name)| Account {
            uuid: uuid_for(index),
            name: (*name).to_owned(),
            launchable: index != 15,
            expired: index == 13,
            expires: now + 86_400 * (20 - index as i64),
            last_launched: match index {
                0 => now - 45 * 60,
                1 => now - 86_400,
                _ => now - 2 * 86_400,
            },
            last_online: now - 3 * 3600,
            running: index == 0,
            launching: false,
            label: if index == 1 {
                "main".to_owned()
            } else {
                String::new()
            },
            pinned: index < 2,
            client: String::new(),
            profile: String::new(),
            launch: LaunchSettings {
                ram: if index == 0 { 4096 } else { 0 },
                ..LaunchSettings::default()
            },
            first_seen: now - 86_400 * 30 + index as i64 * 3600,
        })
        .collect()
}

fn seed_groups(accounts: &[Account]) -> Vec<Group> {
    vec![
        Group {
            id: "g1".into(),
            name: "Основа".into(),
            members: accounts.iter().take(3).map(|a| a.uuid.clone()).collect(),
            profile: String::new(),
            pinned: true,
            order: 0,
        },
        Group {
            id: "g2".into(),
            name: "Фарм".into(),
            members: accounts
                .iter()
                .skip(5)
                .take(6)
                .map(|a| a.uuid.clone())
                .collect(),
            profile: MINIMAL_PROFILE.into(),
            pinned: false,
            order: 1,
        },
        Group {
            id: "g3".into(),
            name: "Пустая".into(),
            members: Vec::new(),
            profile: String::new(),
            pinned: false,
            order: 2,
        },
    ]
}

fn seed_profiles() -> BTreeMap<String, ProfileFiles> {
    let mut files = ProfileFiles::new();
    files.insert(
        "options.txt".into(),
        "renderDistance:8\nmaxFps:120\nfov:0.25\nguiScale:2".into(),
    );
    files.insert(
        "optionsof.txt".into(),
        "ofFastRender:true\nofAnimatedWater:0".into(),
    );
    files.insert("binds.json".into(), "{\"key.drop\":\"Q\"}".into());
    files.insert("voicechat.json".into(), String::new());
    BTreeMap::from([("Анархия".to_owned(), files)])
}

fn seed_logs(accounts: &[Account]) -> Vec<(String, Vec<LogSession>)> {
    let now = now_secs();
    accounts
        .iter()
        .take(3)
        .enumerate()
        .map(|(index, account)| {
            let sessions = (0..=index)
                .map(|n| LogSession {
                    id: format!("s{index}{n}"),
                    started: now - 3600 * (n as i64 + 1) - 600,
                    ended: if n == 0 && index == 0 {
                        0
                    } else {
                        now - 3600 * n as i64
                    },
                    active: n == 0 && index == 0,
                    lines: 40,
                })
                .collect();
            (account.uuid.clone(), sessions)
        })
        .collect()
}

impl MockBackend {
    pub fn new() -> Self {
        let accounts = seed_accounts();
        let groups = seed_groups(&accounts);
        let logs = seed_logs(&accounts);
        Self {
            world: Arc::new(Mutex::new(World {
                accounts,
                groups,
                profiles: seed_profiles(),
                settings: Settings::default(),
                queue: Vec::new(),
                group_launch: GroupLaunchProgress::default(),
                import: ImportProgress::default(),
                import_cancel: false,
                logs,
                background: None,
                subscribers: Vec::new(),
            })),
        }
    }

    fn with<T: Send + 'static>(&self, f: impl FnOnce(&mut World) -> ApiResult<T>) -> ApiFuture<T> {
        let result = f(&mut self.world.lock());
        Box::pin(async move { result })
    }

    fn later(&self, delay: Duration, f: impl FnOnce(&mut World) + Send + 'static) {
        let world = self.world.clone();
        thread::spawn(move || {
            thread::sleep(delay);
            f(&mut world.lock());
        });
    }

    fn start_launch(&self, uuid: String) {
        self.later(Duration::from_millis(300), {
            let uuid = uuid.clone();
            move |world| {
                if let Some(account) = world.account(&uuid) {
                    account.launching = true;
                    let name = account.name.clone();
                    world.queue.push(QueueEntry {
                        uuid: uuid.clone(),
                        name,
                        state: QueueState::Launching,
                    });
                    let queue = world.queue.clone();
                    world.emit(BackendEvent::LaunchQueue(queue));
                    world.emit_accounts();
                }
            }
        });
        self.later(Duration::from_millis(3200), move |world| {
            world.queue.retain(|entry| entry.uuid != uuid);
            if let Some(account) = world.account(&uuid) {
                account.launching = false;
                account.running = true;
                account.last_launched = now_secs();
            }
            let queue = world.queue.clone();
            world.emit(BackendEvent::LaunchQueue(queue));
            world.emit_accounts();
        });
    }

    fn run_import(&self, steps: usize) {
        let world = self.world.clone();
        thread::spawn(move || {
            for index in 0..steps {
                thread::sleep(Duration::from_millis(900));
                let mut world = world.lock();
                if world.import_cancel {
                    for item in &mut world.import.items {
                        if !item.status.finished() {
                            item.status = ImportStatus::Canceled;
                            item.text = msg::CANCELED.to_owned();
                        }
                    }
                    break;
                }
                if let Some(item) = world.import.items.get_mut(index)
                    && item.status == ImportStatus::Pending
                {
                    {
                        item.status = if index % 3 == 2 {
                            ImportStatus::Err
                        } else {
                            ImportStatus::Ok
                        };
                        item.text = if item.status == ImportStatus::Ok {
                            format!("✓ {}", item.login)
                        } else {
                            msg::WRONG_CREDENTIALS.to_owned()
                        };
                    }
                }
                let items = world.import.items.clone();
                world.import = ImportProgress::from_items(true, items);
                let progress = world.import.clone();
                world.emit(BackendEvent::Import(progress));
            }
            let mut world = world.lock();
            let items = world.import.items.clone();
            world.import = ImportProgress::from_items(false, items);
            world.import_cancel = false;
            let progress = world.import.clone();
            world.emit(BackendEvent::Import(progress));
        });
    }
}

fn mock_skin(uuid: &str) -> Vec<u8> {
    let hash = uuid
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    let pick = |shift: u32, palette: &[[u8; 3]]| palette[(hash >> shift) as usize % palette.len()];
    let skin = [181, 132, 108];
    let hair = pick(
        0,
        &[[63, 42, 20], [40, 28, 18], [120, 80, 40], [20, 20, 22]],
    );
    let shirt = pick(
        4,
        &[[0, 155, 160], [70, 120, 200], [180, 60, 60], [90, 150, 70]],
    );
    let pants = pick(8, &[[60, 50, 140], [40, 40, 60], [70, 60, 50]]);
    let mut image = RgbaImage::from_pixel(64, 64, Rgba([0, 0, 0, 0]));
    let mut rect = |x0: u32, y0: u32, w: u32, h: u32, color: [u8; 3]| {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                image.put_pixel(x, y, Rgba([color[0], color[1], color[2], 255]));
            }
        }
    };
    rect(0, 8, 32, 8, skin);
    rect(8, 0, 16, 8, hair);
    rect(8, 8, 8, 2, hair);
    rect(9, 12, 2, 1, [255, 255, 255]);
    rect(13, 12, 2, 1, [255, 255, 255]);
    rect(10, 12, 1, 1, [70, 90, 200]);
    rect(13, 12, 1, 1, [70, 90, 200]);
    rect(11, 14, 2, 1, [150, 90, 70]);
    rect(10, 15, 4, 1, [110, 70, 50]);
    rect(16, 20, 24, 12, shirt);
    rect(40, 20, 16, 12, shirt);
    rect(44, 26, 4, 6, skin);
    rect(0, 20, 16, 12, pants);
    rect(16, 52, 16, 12, pants);
    rect(32, 52, 16, 12, shirt);
    rect(36, 58, 4, 6, skin);
    let mut out = Cursor::new(Vec::new());
    let _ = image.write_to(&mut out, image::ImageFormat::Png);
    out.into_inner()
}

fn player_for(name: &str) -> PlayerInfo {
    let (staff, label, color, donate) = match name {
        "xpepelok" => ("ADMIN", "DEV", "#3b82e8", "NO"),
        "Pepel_ok" => ("MODER", "CUR", "#e06fa0", "NO"),
        "equeque" => ("PLAYER", "", "", "VIP"),
        "xpepelokLegend" => ("PLAYER", "", "", "GOD"),
        _ => ("PLAYER", "", "", "NO"),
    };
    let now = chrono::Utc::now();
    PlayerInfo {
        name: name.to_owned(),
        staff: staff.to_owned(),
        label: label.to_owned(),
        color: color.to_owned(),
        donate: donate.to_owned(),
        donate_color: if donate == "GOD" {
            "#c36bff".into()
        } else {
            String::new()
        },
        online: match name {
            "xpepelok" => Presence::Online,
            "Pepel_ok" | "xpepelokGenius" => Presence::Offline,
            "equeque" | "xpepelokLegend" => Presence::Hidden,
            _ => Presence::Unknown,
        },
        realm: if name == "xpepelok" {
            "Anarchy-1".into()
        } else {
            String::new()
        },
        last_seen: (now - chrono::Duration::minutes(47)).to_rfc3339(),
        registered_at: "2021-03-14T12:30:00Z".into(),
        subscription: if name == "xpepelok" {
            "level_3".into()
        } else {
            String::new()
        },
        likes: 12,
        views: 340,
        ..PlayerInfo::default()
    }
}

impl Backend for MockBackend {
    fn caps(&self) -> Caps {
        Caps::current()
    }

    fn subscribe(&self) -> EventStream {
        let (sender, receiver) = unbounded();
        self.world.lock().subscribers.push(sender);
        receiver.boxed()
    }

    fn status(&self) -> ApiFuture<Status> {
        self.with(|_| {
            Ok(Status {
                launcher_ready: true,
                data_dir: std::env::temp_dir()
                    .join("AccountChangerMock")
                    .display()
                    .to_string(),
            })
        })
    }

    fn accounts(&self) -> ApiFuture<Vec<Account>> {
        self.with(|world| Ok(world.accounts.clone()))
    }

    fn launch(&self, uuid: String, _client: Option<String>) -> ApiFuture<()> {
        let launchable = self
            .world
            .lock()
            .account(&uuid)
            .is_some_and(|account| account.launchable);
        if !launchable {
            return Box::pin(async { Err(ApiError::fixed(msg::NO_TOKEN)) });
        }
        self.start_launch(uuid);
        Box::pin(async { Ok(()) })
    }

    fn launch_guest(&self) -> ApiFuture<()> {
        self.with(|_| Ok(()))
    }

    fn open_launcher(&self) -> ApiFuture<()> {
        self.with(|_| Ok(()))
    }

    fn launch_queue(&self) -> ApiFuture<Vec<QueueEntry>> {
        self.with(|world| Ok(world.queue.clone()))
    }

    fn stop(&self, uuid: String) -> ApiFuture<bool> {
        self.with(move |world| {
            let closed = world.account(&uuid).is_some_and(|account| {
                let was = account.running;
                account.running = false;
                account.launching = false;
                was
            });
            world.queue.retain(|entry| entry.uuid != uuid);
            world.emit_accounts();
            Ok(closed)
        })
    }

    fn forget(&self, uuid: String) -> ApiFuture<()> {
        self.with(move |world| {
            world.accounts.retain(|a| a.uuid != uuid);
            for group in &mut world.groups {
                group.members.retain(|m| *m != uuid);
            }
            world.emit_accounts();
            world.emit_groups();
            Ok(())
        })
    }

    fn set_label(&self, uuid: String, label: String) -> ApiFuture<()> {
        self.with(move |world| {
            if let Some(account) = world.account(&uuid) {
                account.label = normalize_label(&label);
            }
            world.emit_accounts();
            Ok(())
        })
    }

    fn set_pinned(&self, uuid: String, pinned: bool) -> ApiFuture<()> {
        self.with(move |world| {
            if let Some(account) = world.account(&uuid) {
                account.pinned = pinned;
            }
            world.emit_accounts();
            Ok(())
        })
    }

    fn set_account_profile(&self, uuid: String, profile: String) -> ApiFuture<()> {
        self.with(move |world| {
            if let Some(account) = world.account(&uuid) {
                account.profile = profile;
            }
            world.emit_accounts();
            Ok(())
        })
    }

    fn set_ram(&self, uuids: Vec<String>, ram: u32) -> ApiFuture<()> {
        self.with(move |world| {
            for uuid in &uuids {
                if let Some(account) = world.account(uuid) {
                    account.launch.ram = ram;
                }
            }
            world.emit_accounts();
            Ok(())
        })
    }

    fn set_launch_settings(&self, uuids: Vec<String>, settings: LaunchSettings) -> ApiFuture<()> {
        self.with(move |world| {
            for uuid in &uuids {
                if let Some(account) = world.account(uuid) {
                    account.launch = settings.clone();
                }
            }
            world.emit_accounts();
            Ok(())
        })
    }

    fn clients(&self) -> ApiFuture<Clients> {
        self.with(|_| {
            Ok(Clients {
                clients: vec!["minigames".into(), "anarchy".into()],
                current: "minigames".into(),
            })
        })
    }

    fn import_start(&self, accounts: Vec<Credentials>) -> ApiFuture<()> {
        let started = {
            let mut world = self.world.lock();
            if world.import.running {
                Err(ApiError::fixed(msg::IMPORT_RUNNING))
            } else if accounts.is_empty() {
                Err(ApiError::new(msg::import_nothing(0, 0)))
            } else {
                let items = accounts
                    .into_iter()
                    .map(|c| ImportItem {
                        login: c.login,
                        status: ImportStatus::Pending,
                        text: msg::IMPORT_QUEUED.to_owned(),
                        name: String::new(),
                    })
                    .collect();
                world.import = ImportProgress::from_items(true, items);
                let progress = world.import.clone();
                world.emit(BackendEvent::Import(progress));
                Ok(world.import.items.len())
            }
        };
        match started {
            Ok(steps) => {
                self.run_import(steps);
                Box::pin(async { Ok(()) })
            }
            Err(error) => Box::pin(async move { Err(error) }),
        }
    }

    fn import_progress(&self) -> ApiFuture<ImportProgress> {
        self.with(|world| Ok(world.import.clone()))
    }

    fn import_cancel(&self) -> ApiFuture<()> {
        self.with(|world| {
            world.import_cancel = world.import.running;
            Ok(())
        })
    }

    fn refresh_start(&self) -> ApiFuture<()> {
        let names: Vec<Credentials> = {
            let world = self.world.lock();
            if world.import.running {
                return Box::pin(async { Err(ApiError::fixed(msg::REFRESH_RUNNING)) });
            }
            world
                .accounts
                .iter()
                .filter(|a| a.expired)
                .map(|a| Credentials {
                    login: a.name.clone(),
                    password: String::new(),
                })
                .collect()
        };
        if names.is_empty() {
            return Box::pin(async { Err(ApiError::fixed(msg::REFRESH_NOTHING)) });
        }
        self.import_start(names)
    }

    fn profiles(&self) -> ApiFuture<Profiles> {
        self.with(|world| {
            let mut profiles = vec![MINIMAL_PROFILE.to_owned()];
            profiles.extend(world.profiles.keys().cloned());
            Ok(Profiles {
                profiles,
                files: PROFILE_FILES.iter().map(|f| (*f).to_owned()).collect(),
                builtin: vec![MINIMAL_PROFILE.to_owned()],
            })
        })
    }

    fn save_profile(&self, name: String, _client: Option<String>) -> ApiFuture<SavedProfile> {
        self.with(move |world| {
            let name = name.trim().to_owned();
            if name.is_empty() {
                return Err(ApiError::fixed(msg::PROFILE_NAME_REQUIRED));
            }
            if name == MINIMAL_PROFILE {
                return Err(ApiError::fixed(msg::PROFILE_NAME_RESERVED));
            }
            let files = seed_profiles().into_values().next().unwrap_or_default();
            world.profiles.insert(name.clone(), files);
            world.emit(BackendEvent::ProfilesChanged);
            Ok(SavedProfile { name, copied: 3 })
        })
    }

    fn profile_content(&self, name: String) -> ApiFuture<ProfileFiles> {
        self.with(move |world| {
            if name == MINIMAL_PROFILE {
                let mut files = ProfileFiles::new();
                files.insert("options.txt".into(), "renderDistance:2\nmaxFps:30".into());
                files.insert("optionsof.txt".into(), "ofFastRender:true".into());
                files.insert("binds.json".into(), String::new());
                files.insert("voicechat.json".into(), String::new());
                return Ok(files);
            }
            Ok(world.profiles.get(&name).cloned().unwrap_or_default())
        })
    }

    fn update_profile(&self, name: String, files: ProfileFiles) -> ApiFuture<()> {
        self.with(move |world| {
            if name == MINIMAL_PROFILE {
                return Err(ApiError::fixed(msg::PROFILE_BUILTIN_READONLY));
            }
            world.profiles.insert(name, files);
            world.emit(BackendEvent::ProfilesChanged);
            Ok(())
        })
    }

    fn delete_profile(&self, name: String) -> ApiFuture<()> {
        self.with(move |world| {
            if name == MINIMAL_PROFILE {
                return Err(ApiError::fixed(msg::PROFILE_BUILTIN_UNDELETABLE));
            }
            world.profiles.remove(&name);
            world.emit(BackendEvent::ProfilesChanged);
            Ok(())
        })
    }

    fn groups(&self) -> ApiFuture<Vec<Group>> {
        self.with(|world| Ok(world.groups.clone()))
    }

    fn create_group(&self, name: String) -> ApiFuture<Group> {
        self.with(move |world| {
            let name: String = name
                .trim()
                .chars()
                .take(ac_api::GROUP_NAME_MAX_CHARS)
                .collect();
            if name.is_empty() {
                return Err(ApiError::fixed(msg::GROUP_NAME_EMPTY));
            }
            let group = Group {
                id: format!("g{}", world.groups.len() + 10),
                name,
                order: world.groups.len() as i32,
                ..Group::default()
            };
            world.groups.push(group.clone());
            world.emit_groups();
            Ok(group)
        })
    }

    fn delete_group(&self, id: String) -> ApiFuture<()> {
        self.with(move |world| {
            world.groups.retain(|g| g.id != id);
            world.emit_groups();
            Ok(())
        })
    }

    fn rename_group(&self, id: String, name: String) -> ApiFuture<()> {
        self.with(move |world| {
            let name = name.trim().to_owned();
            if name.is_empty() {
                return Err(ApiError::fixed(msg::GROUP_NAME_EMPTY));
            }
            world.group(&id)?.name = name;
            world.emit_groups();
            Ok(())
        })
    }

    fn set_group_pinned(&self, id: String, pinned: bool) -> ApiFuture<()> {
        self.with(move |world| {
            world.group(&id)?.pinned = pinned;
            world.groups.sort_by_key(|g| (!g.pinned, g.order));
            world.emit_groups();
            Ok(())
        })
    }

    fn reorder_groups(&self, ids: Vec<String>) -> ApiFuture<()> {
        self.with(move |world| {
            for (order, id) in ids.iter().enumerate() {
                world.group(id)?.order = order as i32;
            }
            world.groups.sort_by_key(|g| (!g.pinned, g.order));
            world.emit_groups();
            Ok(())
        })
    }

    fn add_group_member(&self, id: String, uuid: String) -> ApiFuture<()> {
        self.with(move |world| {
            let group = world.group(&id)?;
            if !group.members.contains(&uuid) {
                group.members.push(uuid);
            }
            world.emit_groups();
            Ok(())
        })
    }

    fn remove_group_member(&self, id: String, uuid: String) -> ApiFuture<()> {
        self.with(move |world| {
            world.group(&id)?.members.retain(|m| *m != uuid);
            world.emit_groups();
            Ok(())
        })
    }

    fn set_group_profile(&self, id: String, profile: String) -> ApiFuture<()> {
        self.with(move |world| {
            world.group(&id)?.profile = profile;
            world.emit_groups();
            Ok(())
        })
    }

    fn launch_group(&self, id: String) -> ApiFuture<()> {
        let members = {
            let mut world = self.world.lock();
            let members = match world.group(&id) {
                Ok(group) => group.members.clone(),
                Err(error) => return Box::pin(async move { Err(error) }),
            };
            if members.is_empty() {
                return Box::pin(async { Err(ApiError::fixed(msg::GROUP_EMPTY)) });
            }
            world.group_launch = GroupLaunchProgress {
                active: true,
                paused: false,
                done: 0,
                total: members.len() as u32,
                skipped: Vec::new(),
            };
            let progress = world.group_launch.clone();
            world.emit(BackendEvent::GroupLaunch(progress));
            members
        };
        let world = self.world.clone();
        thread::spawn(move || {
            for uuid in members {
                loop {
                    thread::sleep(Duration::from_millis(700));
                    let world = world.lock();
                    if !world.group_launch.active || !world.group_launch.paused {
                        break;
                    }
                }
                let mut world = world.lock();
                if !world.group_launch.active {
                    return;
                }
                let skipped = world.account(&uuid).and_then(|account| {
                    if account.running {
                        Some(GroupSkip {
                            name: account.name.clone(),
                            reason: msg::GROUP_SKIP_RUNNING.to_owned(),
                        })
                    } else {
                        account.running = true;
                        None
                    }
                });
                world.group_launch.skipped.extend(skipped);
                world.group_launch.done += 1;
                let progress = world.group_launch.clone();
                world.emit(BackendEvent::GroupLaunch(progress));
                world.emit_accounts();
            }
            let mut world = world.lock();
            world.group_launch.active = false;
            let progress = world.group_launch.clone();
            world.emit(BackendEvent::GroupLaunch(progress));
        });
        Box::pin(async { Ok(()) })
    }

    fn close_group(&self, id: String) -> ApiFuture<u32> {
        self.with(move |world| {
            let members = world.group(&id)?.members.clone();
            world.group_launch.active = false;
            let mut closed = 0;
            for uuid in members {
                if let Some(account) = world.account(&uuid).filter(|a| a.running) {
                    account.running = false;
                    closed += 1;
                }
            }
            let progress = world.group_launch.clone();
            world.emit(BackendEvent::GroupLaunch(progress));
            world.emit_accounts();
            Ok(closed)
        })
    }

    fn pause_group_launch(&self) -> ApiFuture<bool> {
        self.with(|world| {
            let ok = world.group_launch.active;
            world.group_launch.paused = ok;
            let progress = world.group_launch.clone();
            world.emit(BackendEvent::GroupLaunch(progress));
            Ok(ok)
        })
    }

    fn resume_group_launch(&self) -> ApiFuture<bool> {
        self.with(|world| {
            let ok = world.group_launch.active;
            world.group_launch.paused = false;
            let progress = world.group_launch.clone();
            world.emit(BackendEvent::GroupLaunch(progress));
            Ok(ok)
        })
    }

    fn group_launch_progress(&self) -> ApiFuture<GroupLaunchProgress> {
        self.with(|world| Ok(world.group_launch.clone()))
    }

    fn settings(&self) -> ApiFuture<Settings> {
        self.with(|world| Ok(world.settings.clone()))
    }

    fn set_autostart(&self, enabled: bool) -> ApiFuture<bool> {
        self.with(move |world| {
            world.settings.autostart = enabled;
            Ok(enabled)
        })
    }

    fn set_launcher(&self, launcher: LauncherKind) -> ApiFuture<LauncherKind> {
        self.with(move |world| {
            if launcher.is_exe() && !Caps::current().exe_launchers {
                return Err(ApiError::fixed(msg::LAUNCHER_WINDOWS_ONLY));
            }
            world.settings.launcher = launcher;
            Ok(launcher)
        })
    }

    fn set_custom_launcher(&self, path: PathBuf) -> ApiFuture<CustomLauncher> {
        self.with(move |world| {
            if path.as_os_str().is_empty() {
                return Err(ApiError::fixed(msg::CUSTOM_LAUNCHER_REQUIRED));
            }
            world.settings.launcher = LauncherKind::Custom;
            world.settings.custom_launcher = path.display().to_string();
            Ok(CustomLauncher {
                launcher: LauncherKind::Custom,
                custom_launcher: world.settings.custom_launcher.clone(),
            })
        })
    }

    fn set_auto_play(&self, enabled: bool) -> ApiFuture<bool> {
        self.with(move |world| {
            world.settings.auto_play = enabled;
            Ok(enabled)
        })
    }

    fn set_aggressive(&self, enabled: bool) -> ApiFuture<bool> {
        self.with(move |world| {
            world.settings.aggressive = enabled;
            Ok(enabled)
        })
    }

    fn set_window_title(&self, enabled: bool) -> ApiFuture<bool> {
        self.with(move |world| {
            world.settings.window_title = enabled;
            Ok(enabled)
        })
    }

    fn set_stats(&self, enabled: bool) -> ApiFuture<bool> {
        self.with(move |world| {
            world.settings.stats = enabled;
            let stats = if enabled {
                OnlineStats::Live(OnlineCounts {
                    online: 12,
                    active: 214,
                    total: 1530,
                })
            } else {
                OnlineStats::Disabled
            };
            world.emit(BackendEvent::OnlineStats(stats));
            Ok(enabled)
        })
    }

    fn set_theme(&self, theme: Theme) -> ApiFuture<()> {
        self.with(move |world| {
            world.settings.theme = theme;
            Ok(())
        })
    }

    fn set_sound(&self, enabled: bool) -> ApiFuture<()> {
        self.with(move |world| {
            world.settings.sound = enabled;
            Ok(())
        })
    }

    fn set_background(&self, source: Option<PathBuf>) -> ApiFuture<Option<String>> {
        self.with(move |world| {
            let Some(source) = source else {
                world.background = None;
                world.settings.background = Background::default();
                return Ok(None);
            };
            let bytes = std::fs::read(&source).map_err(|error| ApiError::new(error.to_string()))?;
            let name = format!(
                "background.{}",
                source
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("png")
                    .to_ascii_lowercase()
            );
            world.background = Some(bytes);
            world.settings.background.file = Some(name.clone());
            Ok(Some(name))
        })
    }

    fn set_background_dim(&self, dim: u8) -> ApiFuture<()> {
        self.with(move |world| {
            world.settings.background.dim = dim.min(ac_api::BACKGROUND_DIM_MAX);
            Ok(())
        })
    }

    fn set_background_blur(&self, blur: u8) -> ApiFuture<()> {
        self.with(move |world| {
            world.settings.background.blur = blur.min(ac_api::BACKGROUND_BLUR_MAX);
            Ok(())
        })
    }

    fn background_image(&self) -> ApiFuture<Option<Vec<u8>>> {
        self.with(|world| Ok(world.background.clone()))
    }

    fn background_video(&self) -> ApiFuture<Option<ac_api::VideoBackground>> {
        self.with(|_| Ok(None))
    }

    fn online_stats(&self) -> ApiFuture<OnlineStats> {
        self.with(|world| {
            Ok(if world.settings.stats {
                OnlineStats::Live(OnlineCounts {
                    online: 12,
                    active: 214,
                    total: 1530,
                })
            } else {
                OnlineStats::Disabled
            })
        })
    }

    fn logs(&self) -> ApiFuture<Vec<LogSummary>> {
        self.with(|world| {
            let names: Vec<(String, String)> = world
                .accounts
                .iter()
                .map(|a| (a.uuid.clone(), a.name.clone()))
                .collect();
            Ok(world
                .logs
                .iter()
                .filter_map(|(uuid, sessions)| {
                    let current = sessions.first()?;
                    Some(LogSummary {
                        uuid: uuid.clone(),
                        name: names
                            .iter()
                            .find(|(u, _)| u == uuid)
                            .map(|(_, n)| n.clone())
                            .unwrap_or_default(),
                        lines: current.lines,
                        active: current.active,
                    })
                })
                .collect())
        })
    }

    fn log_stats(&self) -> ApiFuture<LogStats> {
        self.with(|world| {
            let first = world.accounts.first().cloned().unwrap_or_default();
            let second = world.accounts.get(1).cloned().unwrap_or_default();
            Ok(LogStats {
                total_accounts: world.logs.len() as u64,
                total_sessions: world.logs.iter().map(|(_, s)| s.len() as u64).sum(),
                active_now: 1,
                total_duration: 23_400,
                avg_duration: 3_900,
                longest_duration: 7_260,
                longest_name: first.name,
                longest_uuid: first.uuid,
                most_launched_name: second.name,
                most_launched_uuid: second.uuid,
                most_launched_count: 3,
                total_lines: 240,
            })
        })
    }

    fn log(&self, uuid: String, session: Option<String>) -> ApiFuture<LogView> {
        self.with(move |world| {
            let Some((_, sessions)) = world.logs.iter().find(|(u, _)| *u == uuid) else {
                return Ok(LogView::default());
            };
            let chosen = session
                .and_then(|id| sessions.iter().find(|s| s.id == id))
                .or(sessions.first());
            let lines = chosen
                .map(|s| {
                    (0..s.lines)
                        .map(|n| LogLine {
                            at: s.started + n as i64,
                            text: format!(
                                "[{:02}:{:02}:{:02}] [Client thread/INFO]: Line {n} of session {}",
                                n / 3600 % 24,
                                n / 60 % 60,
                                n % 60,
                                s.id
                            ),
                        })
                        .collect()
                })
                .unwrap_or_default();
            Ok(LogView {
                lines,
                sessions: sessions.clone(),
            })
        })
    }

    fn logs_size(&self) -> ApiFuture<u64> {
        self.with(|_| Ok(3_407_872))
    }

    fn clear_logs(&self) -> ApiFuture<()> {
        self.with(|world| {
            world.logs.clear();
            world.emit(BackendEvent::LogsChanged);
            Ok(())
        })
    }

    fn delete_log_session(&self, uuid: String, session: String) -> ApiFuture<()> {
        self.with(move |world| {
            for (u, sessions) in &mut world.logs {
                if *u == uuid {
                    sessions.retain(|s| s.id != session);
                }
            }
            world.emit(BackendEvent::LogsChanged);
            Ok(())
        })
    }

    fn check_update(&self) -> ApiFuture<UpdateInfo> {
        self.with(|_| {
            Ok(UpdateInfo {
                current: env!("CARGO_PKG_VERSION").to_owned(),
                latest: "0.4.1".to_owned(),
                available: true,
                notes: "Нативный интерфейс без WebView.\nФоновые изображения.".to_owned(),
                name: "0.4.1".to_owned(),
                published_at: "2026-11-01T10:00:00Z".to_owned(),
            })
        })
    }

    fn apply_update(&self) -> ApiFuture<()> {
        let world = self.world.clone();
        thread::spawn(move || {
            let total = 18_000_000_u64;
            for step in 1..=20_u64 {
                thread::sleep(Duration::from_millis(150));
                world.lock().emit(BackendEvent::Update(UpdateProgress {
                    active: step < 20,
                    done: step == 20,
                    downloaded: total * step / 20,
                    total,
                    error: String::new(),
                }));
            }
        });
        Box::pin(async { Ok(()) })
    }

    fn update_progress(&self) -> ApiFuture<UpdateProgress> {
        self.with(|_| Ok(UpdateProgress::default()))
    }

    fn player(&self, name: String, _fresh: bool) -> ApiFuture<Option<PlayerInfo>> {
        self.with(move |_| Ok((name != "Grifon4ik").then(|| player_for(&name))))
    }

    fn texture(&self, uuid: String, kind: TextureKind) -> ApiFuture<Option<Vec<u8>>> {
        self.with(move |_| {
            Ok(match kind {
                TextureKind::Skin => Some(mock_skin(&uuid)),
                TextureKind::Cape => None,
            })
        })
    }

    fn focus(&self) -> ApiFuture<()> {
        self.with(|world| {
            world.emit(BackendEvent::FocusRequested);
            Ok(())
        })
    }

    fn quit(&self) -> ApiFuture<()> {
        self.with(|world| {
            world.emit(BackendEvent::QuitRequested);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skins_decode() {
        let bytes = mock_skin("abc");
        let image = image::load_from_memory(&bytes).expect("png");
        assert_eq!((image.width(), image.height()), (64, 64));
    }

    #[test]
    fn ids_are_unique() {
        let accounts = seed_accounts();
        let mut ids: Vec<_> = accounts.iter().map(|a| a.uuid.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), accounts.len());
    }

    #[test]
    fn toggles_round_trip() {
        use ac_api::ClientToggle;
        assert_eq!(
            ClientToggle::from(u8::from(ClientToggle::On)),
            ClientToggle::On
        );
    }
}

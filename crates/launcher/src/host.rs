use std::future::Future;
use std::path::PathBuf;

use ac_api::LauncherKind;

use crate::profiles::MINIMAL_PROFILE;

pub const MIN_RAM: i64 = 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Account {
    pub uuid: String,
    pub name: String,
    pub token: String,
    pub client: String,
    pub profile: String,
    pub ram: i64,
    pub min_graphics: bool,
    pub fullscreen: bool,
    pub discord_rpc: bool,
    pub auto_enter: bool,
    pub debug_mode: bool,
    pub render_distance: i64,
    pub max_fps: i64,
    pub animations: i64,
    pub fast_render: i64,
    pub minimal: bool,
    pub last_launched: i64,
}

impl Account {
    pub fn launchable(&self) -> bool {
        !self.name.is_empty() && !self.token.is_empty()
    }

    pub fn wants_minimal(&self, profile: &str) -> bool {
        self.minimal || profile == MINIMAL_PROFILE
    }

    pub fn launch_options(&self) -> LaunchOptions {
        let ram = if self.wants_minimal(&self.profile) && self.ram < MIN_RAM {
            MIN_RAM
        } else {
            self.ram
        };
        LaunchOptions {
            ram,
            min_graphics: self.min_graphics,
            fullscreen: self.fullscreen,
            discord_rpc: self.discord_rpc,
            auto_enter: self.auto_enter,
            debug_mode: self.debug_mode,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LaunchOptions {
    pub ram: i64,
    pub min_graphics: bool,
    pub fullscreen: bool,
    pub discord_rpc: bool,
    pub auto_enter: bool,
    pub debug_mode: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LaunchConfig {
    pub launcher: LauncherKind,
    pub custom_launcher: PathBuf,
    pub aggressive: bool,
    pub auto_play: bool,
    pub window_title: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LaunchRecord {
    pub uuid: String,
    pub launched_at: i64,
    pub pid: u32,
    pub seen_at: Option<i64>,
    pub before_pids: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Change {
    Queue,
    GroupLaunch,
}

pub trait Host: Send + Sync + 'static {
    fn account(&self, uuid: &str) -> Option<Account>;
    fn accounts(&self) -> Vec<Account>;
    fn config(&self) -> LaunchConfig;
    fn is_staff(&self, name: &str) -> bool;
    fn notify(&self, change: Change);

    fn capture_token(&self, name: &str, token: &str) -> impl Future<Output = ()> + Send;
    fn set_client(&self, uuid: &str, client: &str) -> impl Future<Output = ()> + Send;
    fn mark_launched(&self, uuid: &str) -> impl Future<Output = ()> + Send;
    fn touch_online(&self, uuids: Vec<String>) -> impl Future<Output = ()> + Send;
    fn save_launches(&self, records: Vec<LaunchRecord>) -> impl Future<Output = ()> + Send;
    fn log_begin(&self, uuid: &str, name: &str) -> impl Future<Output = Option<i64>> + Send;
    fn log_append(&self, session: i64, lines: Vec<String>) -> impl Future<Output = ()> + Send;
    fn log_finish(&self, session: i64) -> impl Future<Output = ()> + Send;

    fn log_unsupported(
        &self,
        uuid: &str,
        name: &str,
        message: &str,
    ) -> impl Future<Output = ()> + Send {
        async move {
            if let Some(session) = self.log_begin(uuid, name).await {
                self.log_append(session, vec![message.to_owned()]).await;
                self.log_finish(session).await;
            }
        }
    }
}

pub(crate) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or_default()
}

pub(crate) fn short_uuid(uuid: &str) -> &str {
    uuid.get(..8).unwrap_or(uuid)
}

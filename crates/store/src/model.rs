use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub uuid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub client: String,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub ram: i64,
    #[serde(default)]
    pub min_graphics: bool,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub discord_rpc: bool,
    #[serde(default)]
    pub auto_enter: bool,
    #[serde(default)]
    pub debug_mode: bool,
    #[serde(default)]
    pub render_distance: i64,
    #[serde(default)]
    pub max_fps: i64,
    #[serde(default)]
    pub animations: i64,
    #[serde(default)]
    pub fast_render: i64,
    #[serde(default)]
    pub minimal: bool,
    #[serde(default)]
    pub expires: i64,
    #[serde(default)]
    pub first_seen: i64,
    #[serde(default)]
    pub last_seen: i64,
    #[serde(default)]
    pub last_launched: i64,
    #[serde(default)]
    pub last_online: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LaunchOpts {
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
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub order: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LauncherKind {
    Normal,
    #[default]
    Jar,
    New,
    Custom,
}

impl LauncherKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Jar => "jar",
            Self::New => "new",
            Self::Custom => "custom",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "normal" => Some(Self::Normal),
            "jar" => Some(Self::Jar),
            "new" => Some(Self::New),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub launcher: LauncherKind,
    pub custom_launcher: String,
    pub auto_play: bool,
    pub stats: bool,
    pub aggressive: bool,
    pub window_title: bool,
    pub background: Option<String>,
    pub background_dim: u8,
    pub background_blur: u8,
    pub theme: Theme,
    pub sound: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            launcher: LauncherKind::Jar,
            custom_launcher: String::new(),
            auto_play: true,
            stats: true,
            aggressive: false,
            window_title: false,
            background: None,
            background_dim: 55,
            background_blur: 0,
            theme: Theme::Dark,
            sound: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerCacheEntry {
    pub name: String,
    pub info_json: String,
    pub fetched_at: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameLaunch {
    pub uuid: String,
    pub launched_at: i64,
    pub pid: u32,
    pub seen_at: Option<i64>,
    pub before_pids: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
    pub at: i64,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogSessionInfo {
    pub id: i64,
    pub started: i64,
    pub ended: i64,
    pub active: bool,
    pub lines: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogSummary {
    pub uuid: String,
    pub name: String,
    pub lines: i64,
    pub active: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogStats {
    pub total_accounts: i64,
    pub total_sessions: i64,
    pub active_now: i64,
    pub total_duration: i64,
    pub avg_duration: i64,
    pub longest_duration: i64,
    pub longest_name: String,
    pub longest_uuid: String,
    pub most_launched_name: String,
    pub most_launched_uuid: String,
    pub most_launched_count: i64,
    pub total_lines: i64,
}

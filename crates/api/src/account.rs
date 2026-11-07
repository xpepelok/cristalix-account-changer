use serde::{Deserialize, Serialize};

pub const RAM_MIN: u32 = 1024;
pub const RAM_MAX: u32 = 32768;
pub const RAM_STEP: u32 = 512;
pub const RAM_DEFAULT: u32 = 2048;
pub const LABEL_MAX_CHARS: usize = 8;

pub fn snap_ram(ram: u32) -> u32 {
    let stepped = (ram + RAM_STEP / 2) / RAM_STEP * RAM_STEP;
    stepped.clamp(RAM_MIN, RAM_MAX)
}

pub fn normalize_label(label: &str) -> String {
    label.trim().chars().take(LABEL_MAX_CHARS).collect()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "u8", from = "u8")]
pub enum ClientToggle {
    #[default]
    Keep,
    Off,
    On,
}

impl ClientToggle {
    pub const ALL: [Self; 3] = [Self::Keep, Self::Off, Self::On];
}

impl From<ClientToggle> for u8 {
    fn from(value: ClientToggle) -> Self {
        match value {
            ClientToggle::Keep => 0,
            ClientToggle::Off => 1,
            ClientToggle::On => 2,
        }
    }
}

impl From<u8> for ClientToggle {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Off,
            2 => Self::On,
            _ => Self::Keep,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchSettings {
    pub ram: u32,
    pub min_graphics: bool,
    pub fullscreen: bool,
    #[serde(rename = "discordRPC")]
    pub discord_rpc: bool,
    pub auto_enter: bool,
    pub debug_mode: bool,
    pub render_distance: u32,
    pub max_fps: u32,
    pub animations: ClientToggle,
    pub fast_render: ClientToggle,
    #[serde(default)]
    pub minimal: bool,
}

impl LaunchSettings {
    pub fn auto_ram(&self) -> bool {
        self.ram < RAM_STEP
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub uuid: String,
    pub name: String,
    pub launchable: bool,
    pub expired: bool,
    pub expires: i64,
    pub last_launched: i64,
    pub last_online: i64,
    pub running: bool,
    pub launching: bool,
    pub label: String,
    pub pinned: bool,
    pub client: String,
    pub profile: String,
    #[serde(flatten)]
    pub launch: LaunchSettings,
    pub first_seen: i64,
}

impl Account {
    pub fn short_id(&self) -> &str {
        self.uuid.get(..8).unwrap_or(&self.uuid)
    }

    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            self.short_id()
        } else {
            &self.name
        }
    }

    pub fn token_state(&self) -> TokenState {
        if !self.launchable {
            TokenState::Missing
        } else if self.expired {
            TokenState::Expired
        } else {
            TokenState::Ready
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenState {
    Ready,
    Expired,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    pub login: String,
    pub password: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextureKind {
    Skin,
    Cape,
}

impl TextureKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Skin => "skin",
            Self::Cape => "cape",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clients {
    pub clients: Vec<String>,
    pub current: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub launcher_ready: bool,
    pub data_dir: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Windows,
    Linux,
    Macos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Caps {
    pub os: Os,
    pub credential_import: bool,
    pub auto_play: bool,
    pub exe_launchers: bool,
    pub tray: bool,
}

impl Caps {
    pub const fn current() -> Self {
        let windows = cfg!(windows);
        Self {
            os: if windows {
                Os::Windows
            } else if cfg!(target_os = "macos") {
                Os::Macos
            } else {
                Os::Linux
            },
            credential_import: windows,
            auto_play: windows,
            exe_launchers: windows,
            tray: windows,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueueState {
    Pending,
    Launching,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueEntry {
    pub uuid: String,
    pub name: String,
    pub state: QueueState,
}

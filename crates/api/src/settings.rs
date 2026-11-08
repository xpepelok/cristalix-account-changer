use serde::{Deserialize, Serialize};

pub const BACKGROUND_DIM_DEFAULT: u8 = 55;
pub const BACKGROUND_DIM_MAX: u8 = 100;
pub const BACKGROUND_BLUR_MAX: u8 = 40;
pub const BACKGROUND_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];
pub const VIDEO_EXTENSIONS: [&str; 15] = [
    "mp4", "m4v", "mov", "webm", "mkv", "avi", "wmv", "flv", "mpg", "mpeg", "ts", "m2ts", "3gp",
    "ogv", "apng",
];

pub fn is_video(file: &str) -> bool {
    file.rsplit_once('.').is_some_and(|(_, extension)| {
        VIDEO_EXTENSIONS
            .iter()
            .any(|known| known.eq_ignore_ascii_case(extension))
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoBackground {
    pub ffmpeg: std::path::PathBuf,
    pub file: std::path::PathBuf,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LauncherKind {
    Normal,
    #[default]
    Jar,
    New,
    Custom,
}

impl LauncherKind {
    pub const ALL: [Self; 4] = [Self::Normal, Self::Jar, Self::New, Self::Custom];

    pub const fn is_exe(self) -> bool {
        matches!(self, Self::Normal | Self::New)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub const fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Background {
    pub file: Option<String>,
    pub dim: u8,
    pub blur: u8,
}

impl Default for Background {
    fn default() -> Self {
        Self {
            file: None,
            dim: BACKGROUND_DIM_DEFAULT,
            blur: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub autostart: bool,
    pub launcher: LauncherKind,
    pub custom_launcher: String,
    pub auto_play: bool,
    pub stats: bool,
    pub aggressive: bool,
    pub window_title: bool,
    pub theme: Theme,
    pub sound: bool,
    pub background: Background,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            autostart: false,
            launcher: LauncherKind::Jar,
            custom_launcher: String::new(),
            auto_play: true,
            stats: true,
            aggressive: false,
            window_title: false,
            theme: Theme::Dark,
            sound: true,
            background: Background::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomLauncher {
    pub launcher: LauncherKind,
    pub custom_launcher: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnlineCounts {
    pub online: u64,
    pub active: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum OnlineStats {
    Disabled,
    Unavailable,
    Live(OnlineCounts),
}

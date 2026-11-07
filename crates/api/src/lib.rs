mod account;
mod backend;
mod error;
mod event;
mod groups;
mod jobs;
mod logs;
mod player;
mod profiles;
mod settings;

pub use account::{
    Account, Caps, ClientToggle, Clients, Credentials, LABEL_MAX_CHARS, LaunchSettings, Os,
    QueueEntry, QueueState, RAM_DEFAULT, RAM_MAX, RAM_MIN, RAM_STEP, Status, TextureKind,
    TokenState, normalize_label, snap_ram,
};
pub use backend::{ApiFuture, Backend, EventStream};
pub use error::{ApiError, ApiResult, msg};
pub use event::BackendEvent;
pub use groups::{GROUP_NAME_MAX_CHARS, Group, GroupLaunchProgress, GroupSkip};
pub use jobs::{ImportItem, ImportProgress, ImportStatus, UpdateInfo, UpdateProgress};
pub use logs::{LogLine, LogSession, LogStats, LogSummary, LogView};
pub use player::{PLAYER_RANK_COLOR, PLAYER_RANK_LABEL, PlayerInfo, Presence, Rank};
pub use profiles::{
    MINIMAL_PROFILE, PROFILE_FILES, PROFILE_NAME_MAX_CHARS, ProfileFiles, Profiles, SavedProfile,
};
pub use settings::{
    BACKGROUND_BLUR_MAX, BACKGROUND_DIM_DEFAULT, BACKGROUND_DIM_MAX, BACKGROUND_EXTENSIONS,
    Background, CustomLauncher, LauncherKind, OnlineCounts, OnlineStats, Settings, Theme,
};

#[cfg(test)]
mod tests;

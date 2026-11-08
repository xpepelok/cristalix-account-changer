use std::path::PathBuf;

use futures::future::BoxFuture;
use futures::stream::BoxStream;

use crate::{
    Account, ApiResult, BackendEvent, Caps, Clients, Credentials, CustomLauncher, Group,
    GroupLaunchProgress, ImportProgress, LaunchSettings, LauncherKind, LogStats, LogSummary,
    LogView, OnlineStats, PlayerInfo, ProfileFiles, Profiles, QueueEntry, SavedProfile, Settings,
    Status, TextureKind, Theme, UpdateInfo, UpdateProgress, VideoBackground,
};

pub type ApiFuture<T> = BoxFuture<'static, ApiResult<T>>;
pub type EventStream = BoxStream<'static, BackendEvent>;

pub trait Backend: Send + Sync + 'static {
    fn caps(&self) -> Caps;
    fn subscribe(&self) -> EventStream;

    fn status(&self) -> ApiFuture<Status>;
    fn accounts(&self) -> ApiFuture<Vec<Account>>;
    fn launch(&self, uuid: String, client: Option<String>) -> ApiFuture<()>;
    fn launch_guest(&self) -> ApiFuture<()>;
    fn open_launcher(&self) -> ApiFuture<()>;
    fn launch_queue(&self) -> ApiFuture<Vec<QueueEntry>>;
    fn stop(&self, uuid: String) -> ApiFuture<bool>;
    fn forget(&self, uuid: String) -> ApiFuture<()>;
    fn set_label(&self, uuid: String, label: String) -> ApiFuture<()>;
    fn set_pinned(&self, uuid: String, pinned: bool) -> ApiFuture<()>;
    fn set_account_profile(&self, uuid: String, profile: String) -> ApiFuture<()>;
    fn set_ram(&self, uuids: Vec<String>, ram: u32) -> ApiFuture<()>;
    fn set_launch_settings(&self, uuids: Vec<String>, settings: LaunchSettings) -> ApiFuture<()>;
    fn clients(&self) -> ApiFuture<Clients>;

    fn import_start(&self, accounts: Vec<Credentials>) -> ApiFuture<()>;
    fn import_progress(&self) -> ApiFuture<ImportProgress>;
    fn import_cancel(&self) -> ApiFuture<()>;
    fn refresh_start(&self) -> ApiFuture<()>;

    fn profiles(&self) -> ApiFuture<Profiles>;
    fn save_profile(&self, name: String, client: Option<String>) -> ApiFuture<SavedProfile>;
    fn profile_content(&self, name: String) -> ApiFuture<ProfileFiles>;
    fn update_profile(&self, name: String, files: ProfileFiles) -> ApiFuture<()>;
    fn delete_profile(&self, name: String) -> ApiFuture<()>;

    fn groups(&self) -> ApiFuture<Vec<Group>>;
    fn create_group(&self, name: String) -> ApiFuture<Group>;
    fn delete_group(&self, id: String) -> ApiFuture<()>;
    fn rename_group(&self, id: String, name: String) -> ApiFuture<()>;
    fn set_group_pinned(&self, id: String, pinned: bool) -> ApiFuture<()>;
    fn reorder_groups(&self, ids: Vec<String>) -> ApiFuture<()>;
    fn add_group_member(&self, id: String, uuid: String) -> ApiFuture<()>;
    fn remove_group_member(&self, id: String, uuid: String) -> ApiFuture<()>;
    fn set_group_profile(&self, id: String, profile: String) -> ApiFuture<()>;
    fn launch_group(&self, id: String) -> ApiFuture<()>;
    fn close_group(&self, id: String) -> ApiFuture<u32>;
    fn pause_group_launch(&self) -> ApiFuture<bool>;
    fn resume_group_launch(&self) -> ApiFuture<bool>;
    fn group_launch_progress(&self) -> ApiFuture<GroupLaunchProgress>;

    fn settings(&self) -> ApiFuture<Settings>;
    fn set_autostart(&self, enabled: bool) -> ApiFuture<bool>;
    fn set_launcher(&self, launcher: LauncherKind) -> ApiFuture<LauncherKind>;
    fn set_custom_launcher(&self, path: PathBuf) -> ApiFuture<CustomLauncher>;
    fn set_auto_play(&self, enabled: bool) -> ApiFuture<bool>;
    fn set_aggressive(&self, enabled: bool) -> ApiFuture<bool>;
    fn set_window_title(&self, enabled: bool) -> ApiFuture<bool>;
    fn set_stats(&self, enabled: bool) -> ApiFuture<bool>;
    fn set_theme(&self, theme: Theme) -> ApiFuture<()>;
    fn set_sound(&self, enabled: bool) -> ApiFuture<()>;
    fn set_background(&self, source: Option<PathBuf>) -> ApiFuture<Option<String>>;
    fn set_background_dim(&self, dim: u8) -> ApiFuture<()>;
    fn set_background_blur(&self, blur: u8) -> ApiFuture<()>;
    fn background_image(&self) -> ApiFuture<Option<Vec<u8>>>;
    fn background_video(&self) -> ApiFuture<Option<VideoBackground>>;

    fn online_stats(&self) -> ApiFuture<OnlineStats>;
    fn logs(&self) -> ApiFuture<Vec<LogSummary>>;
    fn log_stats(&self) -> ApiFuture<LogStats>;
    fn log(&self, uuid: String, session: Option<String>) -> ApiFuture<LogView>;
    fn logs_size(&self) -> ApiFuture<u64>;
    fn clear_logs(&self) -> ApiFuture<()>;
    fn delete_log_session(&self, uuid: String, session: String) -> ApiFuture<()>;

    fn check_update(&self) -> ApiFuture<UpdateInfo>;
    fn apply_update(&self) -> ApiFuture<()>;
    fn update_progress(&self) -> ApiFuture<UpdateProgress>;

    fn player(&self, name: String, fresh: bool) -> ApiFuture<Option<PlayerInfo>>;
    fn texture(&self, uuid: String, kind: TextureKind) -> ApiFuture<Option<Vec<u8>>>;

    fn focus(&self) -> ApiFuture<()>;
    fn quit(&self) -> ApiFuture<()>;
}

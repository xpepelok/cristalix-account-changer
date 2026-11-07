#[cfg(windows)]
mod authlogin;
mod autoplay;
mod clients;
mod command;
mod config;
mod download;
mod error;
mod fsutil;
mod games;
mod host;
mod instance;
mod java;
mod logtail;
mod options;
#[cfg(target_os = "linux")]
mod procfs;
mod profiles;
mod queue;
mod start;
mod titles;
mod tracker;
mod watcher;

#[cfg(windows)]
pub use authlogin::{ScriptOutcome, uia_login, uia_logout};
pub use autoplay::{AUTO_PLAY_TIMEOUT, click_play_for_pid};
pub use clients::list_clients;
pub use config::{
    LauncherConfig, annul_account, apply_account, current_client, effective_client, updates_dir,
};
pub use download::{Downloader, JAR_LAUNCHER_URL, LAUNCHER_URL, STAFF_LAUNCHER_URL};
pub use error::{LauncherError, Result};
pub use games::{DEFAULT_GAME_TITLE, Games, Scan, kill_tree, process_tree};
pub use host::{Account, Change, Host, LaunchConfig, LaunchOptions, LaunchRecord, MIN_RAM};
pub use instance::{instance_config, instance_configs, instance_dir};
pub use java::resolve_java;
pub use profiles::{
    MINIMAL_PROFILE, PROFILE_FILES, PROFILE_NAME_MAX_CHARS, delete_profile, list_profiles,
    minimal_profile_content, read_profile, sanitize_profile_name, save_profile_from_client,
    write_profile,
};
pub use queue::LaunchQueue;
pub use start::{spawn_launcher, start_launcher, start_launcher_jar};
pub use titles::Titles;
pub use tracker::{GameTracker, Presence};
pub use watcher::{Watcher, capture_config};

mod error;
mod install;
mod release;
mod updater;
mod version;

pub use error::UpdateError;
pub use install::{UPDATED_FLAG, cleanup_old_exe, restart};
pub use release::{ASSET_NAME, UpdateInfo, asset_matches};
pub use updater::{UpdateProgress, Updater};
pub use version::{CURRENT_VERSION, compare_versions, extract_version};

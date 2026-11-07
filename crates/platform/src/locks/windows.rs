use std::fs::OpenOptions;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

pub(super) fn stale_locks(_dir: &Path, files: Vec<PathBuf>) -> impl Iterator<Item = PathBuf> {
    files.into_iter().filter(|path| {
        OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(path)
            .is_ok()
    })
}

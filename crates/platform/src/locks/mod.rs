#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod sys;
#[cfg(windows)]
#[path = "windows.rs"]
mod sys;

use std::fs;
use std::path::{Path, PathBuf};

pub fn clear_stale_locks(cristalix: &Path) {
    let dir = cristalix.join("locks");
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };
    let files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| !kind.is_dir()))
        .map(|entry| entry.path())
        .collect();
    if files.is_empty() {
        return;
    }
    for path in sys::stale_locks(&dir, files) {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::clear_stale_locks;
    use std::fs;

    #[test]
    fn unused_lock_files_are_removed() {
        let root = tempfile::tempdir().unwrap();
        let locks = root.path().join("locks");
        fs::create_dir_all(locks.join("nested")).unwrap();
        fs::write(locks.join("game.lock"), b"").unwrap();
        clear_stale_locks(root.path());
        assert!(!locks.join("game.lock").exists());
        assert!(locks.join("nested").is_dir());
    }

    #[test]
    fn held_lock_files_survive() {
        let root = tempfile::tempdir().unwrap();
        let locks = root.path().join("locks");
        fs::create_dir_all(&locks).unwrap();
        let held = locks.join("held.lock");
        let _file = fs::File::create(&held).unwrap();
        clear_stale_locks(root.path());
        assert!(held.exists());
    }

    #[test]
    fn missing_directory_is_ignored() {
        let root = tempfile::tempdir().unwrap();
        clear_stale_locks(root.path());
    }
}

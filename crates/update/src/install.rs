use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const UPDATED_FLAG: &str = "--updated";

pub(crate) fn sibling(exe: &Path, suffix: &str) -> PathBuf {
    let mut path = OsString::from(exe.as_os_str());
    path.push(suffix);
    path.into()
}

pub fn cleanup_old_exe(exe: &Path) {
    let _ = fs::remove_file(sibling(exe, ".old"));
}

pub(crate) fn install_downloaded(staged: &Path, exe: &Path) -> io::Result<()> {
    if let Err(err) = make_executable(staged) {
        let _ = fs::remove_file(staged);
        return Err(err);
    }
    let old = sibling(exe, ".old");
    let _ = fs::remove_file(&old);
    if let Err(err) = fs::rename(exe, &old) {
        let _ = fs::remove_file(staged);
        return Err(err);
    }
    if let Err(err) = fs::rename(staged, exe) {
        let _ = fs::rename(&old, exe);
        return Err(err);
    }
    Ok(())
}

pub fn restart(exe: &Path) -> io::Result<()> {
    let mut command = Command::new(exe);
    command.arg(UPDATED_FLAG);
    if let Some(dir) = exe.parent() {
        command.current_dir(dir);
    }
    command.spawn().map(drop)
}

#[cfg(unix)]
fn make_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn make_executable(_: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sibling_appends_suffix() {
        assert_eq!(
            sibling(Path::new("dir/AccountChanger.exe"), ".new"),
            PathBuf::from("dir/AccountChanger.exe.new")
        );
    }

    #[test]
    fn swaps_binary_and_keeps_backup() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("AccountChanger");
        let staged = sibling(&exe, ".new");
        fs::write(&exe, b"old build").unwrap();
        fs::write(&staged, b"new build").unwrap();
        fs::write(sibling(&exe, ".old"), b"stale backup").unwrap();

        install_downloaded(&staged, &exe).unwrap();

        assert_eq!(fs::read(&exe).unwrap(), b"new build");
        assert_eq!(fs::read(sibling(&exe, ".old")).unwrap(), b"old build");
        assert!(!staged.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&exe).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }

        cleanup_old_exe(&exe);
        assert!(!sibling(&exe, ".old").exists());
    }

    #[test]
    fn missing_exe_discards_download() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("AccountChanger");
        let staged = sibling(&exe, ".new");
        fs::write(&staged, b"new build").unwrap();

        assert!(install_downloaded(&staged, &exe).is_err());
        assert!(!staged.exists());
        assert!(!exe.exists());
    }
}

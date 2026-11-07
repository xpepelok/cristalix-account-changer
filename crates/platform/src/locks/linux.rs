use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn stale_locks(dir: &Path, files: Vec<PathBuf>) -> impl Iterator<Item = PathBuf> {
    let open = open_file_paths();
    let real = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    files.into_iter().filter(move |path| {
        let Some(open) = &open else {
            return false;
        };
        path.file_name()
            .is_some_and(|name| !open.contains(&real.join(name)))
    })
}

fn open_file_paths() -> Option<HashSet<PathBuf>> {
    let mut open = HashSet::new();
    for entry in fs::read_dir("/proc").ok()?.filter_map(Result::ok) {
        let name = entry.file_name();
        if name.to_str().is_none_or(|pid| pid.parse::<u32>().is_err()) {
            continue;
        }
        let Ok(fds) = fs::read_dir(entry.path().join("fd")) else {
            continue;
        };
        for fd in fds.filter_map(Result::ok) {
            if let Ok(target) = fs::read_link(fd.path())
                && target.is_absolute()
            {
                open.insert(target);
            }
        }
    }
    Some(open)
}

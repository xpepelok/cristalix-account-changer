use std::fs;
use std::path::{Path, PathBuf};

const INSTANCES: &str = "instances";

pub fn instances_dir(data: &Path) -> PathBuf {
    data.join(INSTANCES)
}

pub fn instance_dir(data: &Path, uuid: &str) -> PathBuf {
    instances_dir(data).join(uuid)
}

pub fn instance_config(dir: &Path) -> PathBuf {
    dir.join(".cristalix").join(".launcher")
}

pub fn instance_configs(data: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(instances_dir(data)) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| instance_config(&entry.path()))
        .collect()
}

fn is_link(path: &Path) -> Option<bool> {
    let meta = fs::symlink_metadata(path).ok()?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        Some(meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
    }
    #[cfg(not(windows))]
    {
        Some(meta.file_type().is_symlink())
    }
}

pub(crate) async fn link_shared_dir(dst: &Path, src: &Path) {
    match is_link(dst) {
        Some(true) => return,
        Some(false) => {
            let _ = fs::remove_dir_all(dst).or_else(|_| fs::remove_file(dst));
        }
        None => {}
    }
    if !src.exists() {
        return;
    }
    make_dir_link(dst, src).await;
}

#[cfg(windows)]
async fn make_dir_link(link: &Path, target: &Path) {
    let mut command = crate::command::detached("cmd");
    command
        .arg("/c")
        .arg("mklink")
        .arg("/J")
        .arg(link)
        .arg(target);
    let _ = command.status().await;
}

#[cfg(not(windows))]
async fn make_dir_link(link: &Path, target: &Path) {
    let _ = std::os::unix::fs::symlink(target, link);
}

pub(crate) fn remove_link(path: &Path) {
    if is_link(path) == Some(true) {
        let _ = fs::remove_dir(path).or_else(|_| fs::remove_file(path));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configs_are_listed_per_instance_directory() {
        let data = tempfile::tempdir().unwrap();
        fs::create_dir_all(instance_dir(data.path(), "a")).unwrap();
        fs::create_dir_all(instance_dir(data.path(), "b")).unwrap();
        fs::write(instances_dir(data.path()).join("stray.txt"), b"").unwrap();
        let mut configs = instance_configs(data.path());
        configs.sort();
        assert_eq!(
            configs,
            [
                instance_config(&instance_dir(data.path(), "a")),
                instance_config(&instance_dir(data.path(), "b")),
            ]
        );
        assert!(instance_configs(&data.path().join("missing")).is_empty());
    }

    #[tokio::test]
    async fn shared_dir_link_points_at_source_and_is_kept() {
        let root = tempfile::tempdir().unwrap();
        let src = root.path().join("runtime");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("java"), b"bin").unwrap();
        let dst = root.path().join("instance").join("runtime");
        fs::create_dir_all(&dst).unwrap();
        fs::write(dst.join("junk"), b"").unwrap();
        link_shared_dir(&dst, &src).await;
        assert_eq!(fs::read(dst.join("java")).unwrap(), b"bin");
        link_shared_dir(&dst, &src).await;
        remove_link(&dst);
        assert!(!dst.exists());
        assert!(src.join("java").exists());
    }
}

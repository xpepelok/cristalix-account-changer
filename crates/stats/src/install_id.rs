use std::fs;
use std::io::{self, Write};
use std::path::Path;

const FILE_NAME: &str = "install.id";
const MIN_LEN: usize = 8;

pub fn install_id(data_dir: &Path) -> Option<String> {
    let path = data_dir.join(FILE_NAME);
    if let Ok(stored) = fs::read_to_string(&path) {
        let stored = stored.trim();
        if stored.len() >= MIN_LEN {
            return Some(stored.to_owned());
        }
    }
    let mut raw = [0u8; 16];
    getrandom::fill(&mut raw).ok()?;
    let id: String = raw.iter().map(|byte| format!("{byte:02x}")).collect();
    let _ = write_private(&path, id.as_bytes());
    Some(id)
}

fn write_private(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_once_and_reuses() {
        let dir = tempfile::tempdir().unwrap();
        let first = install_id(dir.path()).unwrap();
        assert_eq!(first.len(), 32);
        assert!(first.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(install_id(dir.path()).unwrap(), first);
    }

    #[test]
    fn keeps_existing_ids_and_replaces_short_ones() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(FILE_NAME), " legacy-install-id \n").unwrap();
        assert_eq!(install_id(dir.path()).unwrap(), "legacy-install-id");
        fs::write(dir.path().join(FILE_NAME), "short").unwrap();
        let regenerated = install_id(dir.path()).unwrap();
        assert_eq!(regenerated.len(), 32);
        assert_eq!(
            fs::read_to_string(dir.path().join(FILE_NAME)).unwrap(),
            regenerated
        );
    }

    #[test]
    fn unwritable_dir_still_yields_an_id() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(install_id(&dir.path().join("missing")).unwrap().len(), 32);
    }
}

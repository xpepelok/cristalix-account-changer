use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

#[cfg(windows)]
fn open_shared(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_SHARE_DELETE: u32 = 0x4;
    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .open(path)
}

#[cfg(not(windows))]
fn open_shared(path: &Path) -> io::Result<File> {
    OpenOptions::new().read(true).open(path)
}

pub(crate) fn read_shared(path: &Path) -> io::Result<Vec<u8>> {
    let mut file = open_shared(path)?;
    let mut out = Vec::new();
    file.read_to_end(&mut out)?;
    Ok(out)
}

pub(crate) fn read_tail(path: &Path, offset: u64) -> io::Result<(u64, Vec<u8>)> {
    let mut file = open_shared(path)?;
    let len = file.metadata()?.len();
    let start = if len < offset { 0 } else { offset };
    if start == len {
        return Ok((start, Vec::new()));
    }
    file.seek(SeekFrom::Start(start))?;
    let mut out = Vec::with_capacity(usize::try_from(len - start).unwrap_or(0));
    file.read_to_end(&mut out)?;
    Ok((start, out))
}

pub(crate) fn write_atomic(path: &Path, payload: &[u8]) -> io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    fs::write(&tmp, payload)?;
    fs::rename(&tmp, path)
}

pub(crate) fn is_dir(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|meta| meta.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_reads_from_offset_and_resets_on_shrink() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("latest.log");
        fs::write(&path, b"hello\nworld\n").unwrap();
        assert_eq!(read_tail(&path, 6).unwrap(), (6, b"world\n".to_vec()));
        assert_eq!(read_tail(&path, 12).unwrap(), (12, Vec::new()));
        fs::write(&path, b"new\n").unwrap();
        assert_eq!(read_tail(&path, 12).unwrap(), (0, b"new\n".to_vec()));
    }

    #[test]
    fn shared_read_survives_rename_of_open_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".launcher");
        fs::write(&path, b"{}").unwrap();
        let _held = open_shared(&path).unwrap();
        write_atomic(&path, b"{\"a\":1}").unwrap();
        assert_eq!(read_shared(&path).unwrap(), b"{\"a\":1}");
    }
}

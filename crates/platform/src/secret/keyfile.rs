use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use aes_gcm::aead::{Aead, Generate, Key, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};

use crate::error::{PlatformError, Result};

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;

pub(super) struct KeyFile {
    path: PathBuf,
    cipher: Mutex<Option<Aes256Gcm>>,
}

impl KeyFile {
    pub(super) fn new(path: PathBuf) -> Self {
        Self {
            path,
            cipher: Mutex::new(None),
        }
    }

    pub(super) fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>> {
        self.with_cipher(|cipher| {
            let nonce = Nonce::generate();
            let body = cipher
                .encrypt(&nonce, plain)
                .map_err(|_| PlatformError::Encrypt)?;
            let mut sealed = Vec::with_capacity(NONCE_LEN + body.len());
            sealed.extend_from_slice(&nonce);
            sealed.extend_from_slice(&body);
            Ok(sealed)
        })
    }

    pub(super) fn decrypt(&self, sealed: &[u8]) -> Result<Vec<u8>> {
        self.with_cipher(|cipher| {
            let (nonce, body) = sealed
                .split_at_checked(NONCE_LEN)
                .ok_or(PlatformError::CiphertextTooShort)?;
            let nonce = Nonce::try_from(nonce).map_err(|_| PlatformError::CiphertextTooShort)?;
            cipher
                .decrypt(&nonce, body)
                .map_err(|_| PlatformError::Tampered)
        })
    }

    fn with_cipher<T>(&self, run: impl FnOnce(&Aes256Gcm) -> Result<T>) -> Result<T> {
        let mut slot = self.cipher.lock().unwrap_or_else(PoisonError::into_inner);
        let cipher = match &mut *slot {
            Some(cipher) => cipher,
            empty @ None => empty.insert(Aes256Gcm::new(&self.load_or_create()?)),
        };
        run(cipher)
    }

    fn load_or_create(&self) -> Result<Key<Aes256Gcm>> {
        match fs::read(&self.path) {
            Ok(raw) => return self.parse(&raw),
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
        if let Some(dir) = self.path.parent() {
            create_private_dir(dir)?;
        }
        let key = Key::<Aes256Gcm>::generate();
        match create_private_file(&self.path) {
            Ok(file) => {
                if let Err(err) = write_key(file, &key) {
                    let _ = fs::remove_file(&self.path);
                    return Err(err.into());
                }
                Ok(key)
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                self.parse(&fs::read(&self.path)?)
            }
            Err(err) => Err(err.into()),
        }
    }

    fn parse(&self, raw: &[u8]) -> Result<Key<Aes256Gcm>> {
        Key::<Aes256Gcm>::try_from(raw).map_err(|_| PlatformError::CorruptKey {
            path: self.path.clone(),
            len: raw.len(),
            expected: KEY_LEN,
        })
    }
}

fn write_key(mut file: File, key: &[u8]) -> io::Result<()> {
    file.write_all(key)?;
    file.sync_all()
}

fn create_private_dir(dir: &Path) -> io::Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}

fn create_private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::{KEY_LEN, KeyFile};
    use std::fs;

    fn isolated() -> (tempfile::TempDir, KeyFile) {
        let dir = tempfile::tempdir().unwrap();
        let keys = KeyFile::new(dir.path().join("AccountChanger").join("key"));
        (dir, keys)
    }

    #[test]
    fn encrypt_round_trip() {
        let (_dir, keys) = isolated();
        let plain = br#"{"accounts":{"uuid":{"token":"jwt.value.here"}}}"#;
        let sealed = keys.encrypt(plain).unwrap();
        assert!(!sealed.windows(plain.len()).any(|chunk| chunk == plain));
        assert_eq!(keys.decrypt(&sealed).unwrap(), plain);
    }

    #[test]
    fn decrypt_rejects_tampered_ciphertext() {
        let (_dir, keys) = isolated();
        let mut sealed = keys.encrypt(b"secret token").unwrap();
        *sealed.last_mut().unwrap() ^= 0xff;
        assert!(keys.decrypt(&sealed).is_err());
    }

    #[test]
    fn decrypt_rejects_garbage() {
        let (_dir, keys) = isolated();
        assert!(keys.decrypt(b"short").is_err());
        assert!(keys.decrypt(b"not a ciphertext").is_err());
    }

    #[test]
    fn key_file_is_owner_only() {
        let (_dir, keys) = isolated();
        keys.encrypt(b"x").unwrap();
        let meta = fs::metadata(&keys.path).unwrap();
        assert_eq!(meta.len(), KEY_LEN as u64);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn key_is_reused_across_restarts() {
        let (_dir, keys) = isolated();
        let sealed = keys.encrypt(b"token").unwrap();
        let restarted = KeyFile::new(keys.path.clone());
        assert_eq!(restarted.decrypt(&sealed).unwrap(), b"token");
    }

    #[test]
    fn corrupt_key_is_not_silently_replaced() {
        let (_dir, keys) = isolated();
        keys.encrypt(b"x").unwrap();
        fs::write(&keys.path, b"short").unwrap();
        let restarted = KeyFile::new(keys.path.clone());
        assert!(restarted.encrypt(b"x").is_err());
        assert_eq!(fs::read(&keys.path).unwrap(), b"short");
    }

    #[test]
    fn unreadable_key_is_an_error() {
        let (_dir, keys) = isolated();
        fs::create_dir_all(&keys.path).unwrap();
        assert!(keys.encrypt(b"x").is_err());
        assert!(keys.path.is_dir());
    }
}

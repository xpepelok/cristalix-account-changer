#[cfg(windows)]
mod dpapi;
#[cfg(any(target_os = "linux", test))]
mod keyfile;

use crate::error::Result;
use crate::paths::Paths;

pub struct Secrets {
    #[cfg(target_os = "linux")]
    keys: keyfile::KeyFile,
}

#[cfg(windows)]
impl Secrets {
    pub fn new(_paths: &Paths) -> Self {
        Self {}
    }

    pub fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>> {
        dpapi::protect(plain)
    }

    pub fn decrypt(&self, sealed: &[u8]) -> Result<Vec<u8>> {
        dpapi::unprotect(sealed)
    }
}

#[cfg(target_os = "linux")]
impl Secrets {
    pub fn new(paths: &Paths) -> Self {
        Self {
            keys: keyfile::KeyFile::new(paths.data.join("key")),
        }
    }

    pub fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>> {
        self.keys.encrypt(plain)
    }

    pub fn decrypt(&self, sealed: &[u8]) -> Result<Vec<u8>> {
        self.keys.decrypt(sealed)
    }
}

#[cfg(test)]
mod tests {
    use super::Secrets;
    use crate::paths::Paths;

    fn isolated() -> (tempfile::TempDir, Secrets) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_dirs(dir.path(), dir.path().join("data"));
        let secrets = Secrets::new(&paths);
        (dir, secrets)
    }

    #[test]
    fn encrypt_round_trip() {
        let (_dir, secrets) = isolated();
        let plain = br#"{"accounts":{"uuid":{"token":"jwt.value.here"}}}"#;
        let sealed = secrets.encrypt(plain).unwrap();
        assert!(!sealed.windows(plain.len()).any(|chunk| chunk == plain));
        assert_eq!(secrets.decrypt(&sealed).unwrap(), plain);
    }

    #[test]
    fn decrypt_rejects_tampered_ciphertext() {
        let (_dir, secrets) = isolated();
        let mut sealed = secrets.encrypt(b"secret token").unwrap();
        *sealed.last_mut().unwrap() ^= 0xff;
        assert!(secrets.decrypt(&sealed).is_err());
    }

    #[test]
    fn decrypt_rejects_garbage() {
        let (_dir, secrets) = isolated();
        assert!(secrets.decrypt(b"not a ciphertext").is_err());
    }

    #[test]
    fn empty_plaintext_round_trips() {
        let (_dir, secrets) = isolated();
        let sealed = secrets.encrypt(b"").unwrap();
        assert!(secrets.decrypt(&sealed).unwrap().is_empty());
    }
}

use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("key file {} is corrupt ({len} bytes, want {expected})", path.display())]
    CorruptKey {
        path: PathBuf,
        len: usize,
        expected: usize,
    },
    #[error("ciphertext too short")]
    CiphertextTooShort,
    #[error("ciphertext failed authentication")]
    Tampered,
    #[error("encryption failed")]
    Encrypt,
    #[error("payload too large")]
    TooLarge,
    #[cfg(windows)]
    #[error("{context}: {source}")]
    Os {
        context: &'static str,
        source: windows::core::Error,
    },
}

pub type Result<T> = std::result::Result<T, PlatformError>;

#[cfg(windows)]
impl PlatformError {
    pub(crate) fn os(context: &'static str) -> impl FnOnce(windows::core::Error) -> Self {
        move |source| Self::Os { context, source }
    }
}

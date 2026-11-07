use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LauncherError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error("download launcher: status {0}")]
    Status(u16),
    #[error("launcher not found: {0}")]
    NotFound(io::Error),
}

pub type Result<T> = std::result::Result<T, LauncherError>;

use thiserror::Error;

use ac_platform::PlatformError;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("migration failed: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error(transparent)]
    Secret(#[from] PlatformError),
    #[error("corrupt stored value: {0}")]
    Corrupt(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, StoreError>;

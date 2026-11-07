use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("нет доступного обновления")]
    NoUpdate,
    #[error("не удалось скачать обновление (код {0})")]
    Status(u16),
    #[error("{0}")]
    Http(#[from] reqwest::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

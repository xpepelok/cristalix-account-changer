use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlayerError {
    #[error("http client: {0}")]
    Client(#[from] reqwest::Error),
    #[error("players cache: {0}")]
    Cache(#[from] serde_json::Error),
}

#[derive(Debug, Error)]
pub enum TextureError {
    #[error("bad uuid")]
    InvalidUuid,
    #[error("not found")]
    NotFound,
    #[error("upstream error: {0}")]
    Upstream(#[from] reqwest::Error),
}

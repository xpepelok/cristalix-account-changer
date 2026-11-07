use thiserror::Error;

#[derive(Debug, Error)]
pub enum StatsError {
    #[error("http client: {0}")]
    Client(#[from] reqwest::Error),
}

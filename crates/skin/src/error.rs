use thiserror::Error;

#[derive(Debug, Error)]
pub enum SkinError {
    #[error("png: {0}")]
    Decode(#[from] png::DecodingError),
    #[error("unsupported png pixel format")]
    UnsupportedFormat,
    #[error("bad skin size: {0}x{1}")]
    SkinSize(u32, u32),
    #[error("bad cape size: {0}x{1}")]
    CapeSize(u32, u32),
}

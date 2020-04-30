use thiserror::Error;

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum PushError {
    #[error("")]
    NoError,
    #[error("limit: {0}, request: {1}")]
    PushEventTooLarge(usize, usize),
    #[error("substream EOF")]
    SubStreamEof,
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
}

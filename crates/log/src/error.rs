use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Msg(String),
    #[error("logging init failed: {0}")]
    Init(&'static str),
}

pub type Result<T> = core::result::Result<T, Error>;

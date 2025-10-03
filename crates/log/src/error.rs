use alloc::string::String;

#[cfg(feature = "std")]
use thiserror::Error;

#[cfg_attr(feature = "std", derive(Error))]
#[derive(Debug)]
pub enum Error {
    #[cfg(feature = "std")]
    #[error("{0}")]
    Msg(String),

    #[cfg(feature = "std")]
    #[error("logging init failed: {0}")]
    Init(&'static str),
}

pub type Result<T> = core::result::Result<T, Error>;

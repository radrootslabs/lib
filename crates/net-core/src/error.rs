use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Msg(String),

    #[error("mutex lock poisoned!")]
    Poisoned,

    #[cfg(feature = "std")]
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    pub fn msg<M: Into<String>>(msg: M) -> Self {
        Error::Msg(msg.into())
    }
}

pub type Result<T> = core::result::Result<T, Error>;

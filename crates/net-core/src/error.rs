use alloc::string::String;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetError {
    #[error("{0}")]
    Msg(String),

    #[error("mutex lock poisoned")]
    Poisoned,

    #[cfg(feature = "std")]
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("missing key")]
    MissingKey,

    #[error("invalid hex32")]
    InvalidHex32,

    #[error("invalid bech32")]
    InvalidBech32,

    #[error("invalid key file")]
    InvalidKeyFile,

    #[error("key I/O")]
    KeyIo,

    #[error("overwrite denied")]
    OverwriteDenied,

    #[error("persistence unsupported")]
    PersistenceUnsupported,

    #[error("logging init failed: {0}")]
    LoggingInit(&'static str),
}

impl NetError {
    pub fn msg<M: Into<String>>(msg: M) -> Self {
        NetError::Msg(msg.into())
    }
}

impl Clone for NetError {
    fn clone(&self) -> Self {
        match self {
            NetError::Msg(m) => NetError::Msg(m.clone()),
            NetError::Poisoned => NetError::Poisoned,
            #[cfg(feature = "std")]
            NetError::Io(_) => {
                panic!("cannot clone std::io::Error");
            }
            NetError::MissingKey => NetError::MissingKey,
            NetError::InvalidHex32 => NetError::InvalidHex32,
            NetError::InvalidBech32 => NetError::InvalidBech32,
            NetError::InvalidKeyFile => NetError::InvalidKeyFile,
            NetError::KeyIo => NetError::KeyIo,
            NetError::OverwriteDenied => NetError::OverwriteDenied,
            NetError::PersistenceUnsupported => NetError::PersistenceUnsupported,
            NetError::LoggingInit(s) => NetError::LoggingInit(s),
        }
    }
}

pub type Result<T> = core::result::Result<T, NetError>;

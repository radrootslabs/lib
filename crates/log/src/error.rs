use alloc::string::String;

#[cfg(feature = "std")]
use thiserror::Error;

#[cfg_attr(feature = "std", derive(Error))]
#[derive(Debug)]
pub enum Error {
    #[cfg_attr(feature = "std", error("{0}"))]
    Msg(String),

    #[cfg(feature = "std")]
    #[error(transparent)]
    Init(#[from] tracing_subscriber::util::TryInitError),

    #[cfg(feature = "std")]
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[cfg(feature = "std")]
    #[error(transparent)]
    RollingInit(#[from] tracing_appender::rolling::InitError),
}

pub type Result<T> = core::result::Result<T, Error>;

use alloc::string::String;

#[cfg(feature = "std")]
use thiserror::Error;

#[cfg_attr(feature = "std", derive(Error))]
#[derive(Debug)]
pub enum Error {
    #[cfg_attr(feature = "std", error("{0}"))]
    Msg(String),

    #[cfg(feature = "std")]
    #[error("logging is already initialized with a different configuration")]
    ConflictingInitialization,

    #[cfg(feature = "std")]
    #[error("logging cannot be initialized after it has been shut down")]
    InitializationAfterShutdown,

    #[cfg(feature = "std")]
    #[error("logging cannot be shut down before it is initialized")]
    ShutdownBeforeInitialization,

    #[cfg(feature = "std")]
    #[error(transparent)]
    Init(#[from] tracing_subscriber::util::TryInitError),

    #[cfg(feature = "std")]
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = core::result::Result<T, Error>;

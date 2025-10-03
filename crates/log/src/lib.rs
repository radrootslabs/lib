#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod error;

#[cfg(feature = "std")]
mod init;
#[cfg(feature = "std")]
mod options;

pub use error::{Error, Result};

#[cfg(feature = "std")]
pub use init::{init_logging, init_stdout};
#[cfg(feature = "std")]
pub use options::LoggingOptions;

use tracing::{debug, error, info};

#[inline]
pub fn log_info<S: AsRef<str>>(msg: S) {
    info!("{}", msg.as_ref());
}

#[inline]
pub fn log_error<S: AsRef<str>>(msg: S) {
    error!("{}", msg.as_ref());
}

#[inline]
pub fn log_debug<S: AsRef<str>>(msg: S) {
    debug!("{}", msg.as_ref());
}

#[cfg(not(feature = "std"))]
pub fn init_no_std() -> Result<()> {
    Ok(())
}

pub fn init_default() -> Result<()> {
    #[cfg(feature = "std")]
    {
        return init_stdout();
    }
    #[cfg(not(feature = "std"))]
    {
        return init_no_std();
    }
}

mod error;
mod init;
mod options;

pub use error::{Error, Result};
pub use init::{init_logging, init_stdout};
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

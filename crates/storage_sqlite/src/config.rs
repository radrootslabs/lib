//! Validated SQLite connection configuration.

use std::time::Duration;

use crate::{Error, OpenMode, Paths};
use radroots_storage::status::WriterPolicy;

const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const MIN_BUSY_TIMEOUT: Duration = Duration::from_millis(1);
const MAX_BUSY_TIMEOUT: Duration = Duration::from_secs(60);

/// Validated options for opening the runtime and private SQLite stores.
///
/// Foreign-key enforcement is always enabled. Writable stores always use WAL;
/// neither invariant can be disabled through the public API.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenOptions {
    paths: Paths,
    mode: OpenMode,
    busy_timeout: Duration,
}

impl OpenOptions {
    /// Creates options with the governed five-second busy timeout.
    pub fn new(paths: Paths, mode: OpenMode) -> Self {
        Self {
            paths,
            mode,
            busy_timeout: DEFAULT_BUSY_TIMEOUT,
        }
    }

    /// Replaces the busy timeout after enforcing the supported bound.
    pub fn with_busy_timeout(mut self, busy_timeout: Duration) -> Result<Self, Error> {
        if !(MIN_BUSY_TIMEOUT..=MAX_BUSY_TIMEOUT).contains(&busy_timeout) {
            return Err(Error::InvalidBusyTimeout {
                minimum: MIN_BUSY_TIMEOUT,
                maximum: MAX_BUSY_TIMEOUT,
                actual: busy_timeout,
            });
        }
        self.busy_timeout = busy_timeout;
        Ok(self)
    }

    /// Returns the two database paths owned by this backend instance.
    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    /// Returns the requested lifecycle mode.
    pub fn mode(&self) -> OpenMode {
        self.mode
    }

    /// Returns the busy timeout applied to every owned connection.
    pub fn busy_timeout(&self) -> Duration {
        self.busy_timeout
    }

    /// Reports the non-configurable foreign-key policy.
    pub fn foreign_keys_enabled(&self) -> bool {
        true
    }

    /// Reports whether the mode requires WAL for writable connections.
    pub fn wal_enabled(&self) -> bool {
        self.mode.is_writable()
    }

    /// Reports the mandatory writer coordination policy for this mode.
    pub fn writer_policy(&self) -> WriterPolicy {
        if self.mode.is_writable() {
            WriterPolicy::AdvisoryProcessLock
        } else {
            WriterPolicy::NoWriter
        }
    }

    /// Validates current filesystem state without creating or modifying files.
    pub fn validate_filesystem(&self) -> Result<(), Error> {
        self.paths.validate_filesystem(self.mode)
    }
}

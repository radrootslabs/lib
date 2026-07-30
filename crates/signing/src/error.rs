//! Normalized signing failures.

use core::fmt;

/// A signing failure.
///
/// Step 104 replaces this opaque pre-release value with the governed error
/// catalog. It intentionally carries no dependency-specific or secret data.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error;

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("signing operation failed")
    }
}

impl core::error::Error for Error {}

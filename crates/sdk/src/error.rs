//! SDK-owned error boundary.

use std::{error, fmt};

/// Narrow SDK error boundary.
///
/// Concrete variants are introduced with the owning operation contracts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct Error {
    _private: (),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Radroots SDK operation failed")
    }
}

impl error::Error for Error {}

/// SDK result alias.
pub type Result<T> = std::result::Result<T, Error>;

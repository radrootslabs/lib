//! SDK-owned error boundary.

use std::{error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// No storage capability was supplied.
    MissingStorage,
    /// A signer cannot be selected without an outbound event sink.
    SignerWithoutSink,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingStorage => formatter.write_str("SDK storage capability is missing"),
            Self::SignerWithoutSink => {
                formatter.write_str("SDK signer requires an outbound event sink")
            }
        }
    }
}

impl error::Error for Error {}

/// SDK result alias.
pub type Result<T> = std::result::Result<T, Error>;

//! SDK-owned error boundary.

use std::{error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// No storage capability was supplied.
    MissingStorage,
    /// A signer cannot be selected without an outbound event sink.
    SignerWithoutSink,
    /// Another clone is actively closing shared resources.
    CloseInProgress,
    /// Close was cancelled after beginning and requires an explicit retry.
    ClientClosing,
    /// The client has completed explicit shutdown.
    ClientClosed,
    /// The storage close operation failed after shutdown began.
    StorageCloseFailed,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingStorage => formatter.write_str("SDK storage capability is missing"),
            Self::SignerWithoutSink => {
                formatter.write_str("SDK signer requires an outbound event sink")
            }
            Self::CloseInProgress => formatter.write_str("SDK client close is in progress"),
            Self::ClientClosing => {
                formatter.write_str("SDK client close requires completion or retry")
            }
            Self::ClientClosed => formatter.write_str("SDK client is closed"),
            Self::StorageCloseFailed => formatter.write_str("SDK storage close failed"),
        }
    }
}

impl error::Error for Error {}

/// SDK result alias.
pub type Result<T> = std::result::Result<T, Error>;

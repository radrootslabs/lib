use std::fmt;

use crate::download::FetchFailurePhase;

/// Failure returned by GeoNames configuration and lookup operations.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// An asset version was empty or contained surrounding whitespace.
    InvalidAssetVersion,
    /// An asset file name was not one safe, relative path component.
    InvalidAssetFileName,
    /// An asset source or authority was empty or contained whitespace.
    InvalidAssetSource,
    /// An asset source did not use HTTPS with the declared authority.
    UntrustedAssetSource,
    /// An asset declared a zero byte size.
    InvalidAssetByteSize,
    /// The host-selected destination could not be used safely.
    UnsafeAssetDestination,
    /// Another acquisition currently owns the destination lock.
    AssetDestinationBusy,
    /// A filesystem operation failed.
    Io {
        /// Stable operation label without a host path.
        operation: &'static str,
        /// Portable I/O failure category.
        kind: std::io::ErrorKind,
    },
    /// The injected fetcher failed before producing a complete stream.
    Fetch {
        /// Stable acquisition phase.
        phase: FetchFailurePhase,
    },
    /// The acquired or inspected asset had the wrong length.
    AssetSizeMismatch {
        /// Declared asset length.
        expected: u64,
        /// Observed length, capped at `expected + 1` during acquisition.
        actual: u64,
    },
    /// The acquired or inspected asset had the wrong digest.
    AssetHashMismatch,
    /// The verified bytes were not a healthy SQLite database.
    InvalidDatabase,
    /// The database did not contain the governed GeoNames schema.
    InvalidDatabaseSchema,
    /// Exclusive access to the private database connection was unavailable.
    DatabaseConnectionUnavailable,
    /// A read-only database operation failed.
    DatabaseOperationFailed {
        /// Stable operation label without SQL or host paths.
        operation: &'static str,
    },
    /// A coordinate was non-finite or outside its geographic bounds.
    InvalidPoint,
    /// A query string was empty or contained surrounding whitespace.
    InvalidQueryText,
    /// A query limit was outside the supported range.
    InvalidQueryLimit,
    /// A reverse-query radius was non-finite or out of bounds.
    InvalidQueryRadius,
    /// A feature identifier could not be represented by the provider database.
    InvalidFeatureId,
    /// A locality-only option was applied to another query kind.
    QueryOptionNotApplicable,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAssetVersion => {
                formatter.write_str("asset version must be non-empty and normalized")
            }
            Self::InvalidAssetFileName => {
                formatter.write_str("asset file name must be one safe relative path component")
            }
            Self::InvalidAssetSource => {
                formatter.write_str("asset source and authority must be non-empty and normalized")
            }
            Self::UntrustedAssetSource => {
                formatter.write_str("asset source must use HTTPS and the declared authority")
            }
            Self::InvalidAssetByteSize => {
                formatter.write_str("asset byte size must be greater than zero")
            }
            Self::UnsafeAssetDestination => formatter.write_str("asset destination is unsafe"),
            Self::AssetDestinationBusy => {
                formatter.write_str("asset destination is already being acquired")
            }
            Self::Io { operation, kind } => write!(formatter, "{operation} failed: {kind}"),
            Self::Fetch { phase } => write!(formatter, "asset fetch failed during {phase}"),
            Self::AssetSizeMismatch { expected, actual } => write!(
                formatter,
                "asset size mismatch: expected {expected} bytes, observed {actual}"
            ),
            Self::AssetHashMismatch => {
                formatter.write_str("asset SHA-256 does not match its specification")
            }
            Self::InvalidDatabase => formatter.write_str("asset is not a healthy SQLite database"),
            Self::InvalidDatabaseSchema => {
                formatter.write_str("asset does not contain the governed GeoNames schema")
            }
            Self::DatabaseConnectionUnavailable => {
                formatter.write_str("GeoNames database connection is unavailable")
            }
            Self::DatabaseOperationFailed { operation } => {
                write!(formatter, "GeoNames database {operation} failed")
            }
            Self::InvalidPoint => {
                formatter.write_str("point must contain finite, in-range coordinates")
            }
            Self::InvalidQueryText => {
                formatter.write_str("query text must be non-empty and normalized")
            }
            Self::InvalidQueryLimit => {
                formatter.write_str("query limit must be between 1 and 1000")
            }
            Self::InvalidQueryRadius => {
                formatter.write_str("reverse-query radius must be greater than 0 and at most 10")
            }
            Self::InvalidFeatureId => {
                formatter.write_str("feature identifier exceeds the provider database range")
            }
            Self::QueryOptionNotApplicable => {
                formatter.write_str("query option is not applicable to this query kind")
            }
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::Error;
    use crate::download::FetchFailurePhase;

    #[test]
    fn every_public_error_has_a_secret_safe_stable_message() {
        let cases = [
            Error::InvalidAssetVersion,
            Error::InvalidAssetFileName,
            Error::InvalidAssetSource,
            Error::UntrustedAssetSource,
            Error::InvalidAssetByteSize,
            Error::UnsafeAssetDestination,
            Error::AssetDestinationBusy,
            Error::Io {
                operation: "read asset",
                kind: std::io::ErrorKind::PermissionDenied,
            },
            Error::Fetch {
                phase: FetchFailurePhase::Connect,
            },
            Error::AssetSizeMismatch {
                expected: 10,
                actual: 9,
            },
            Error::AssetHashMismatch,
            Error::InvalidDatabase,
            Error::InvalidDatabaseSchema,
            Error::DatabaseConnectionUnavailable,
            Error::DatabaseOperationFailed { operation: "query" },
            Error::InvalidPoint,
            Error::InvalidQueryText,
            Error::InvalidQueryLimit,
            Error::InvalidQueryRadius,
            Error::InvalidFeatureId,
            Error::QueryOptionNotApplicable,
        ];
        for error in cases {
            let message = error.to_string();
            assert!(!message.is_empty());
            for forbidden in ["https://", "/tmp/", "SELECT ", "token="] {
                assert!(!message.contains(forbidden));
            }
        }
    }
}

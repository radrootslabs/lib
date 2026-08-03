use std::fmt;

/// Failure returned by GeoNames configuration and lookup operations.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// An asset version was empty or contained surrounding whitespace.
    InvalidAssetVersion,
    /// An asset file name was not one safe, relative path component.
    InvalidAssetFileName,
    /// An asset source was empty or contained surrounding whitespace.
    InvalidAssetSource,
    /// An asset declared a zero byte size.
    InvalidAssetByteSize,
    /// A coordinate was non-finite or outside its geographic bounds.
    InvalidPoint,
    /// A query string was empty or contained surrounding whitespace.
    InvalidQueryText,
    /// A query limit was outside the supported range.
    InvalidQueryLimit,
    /// A locality-only option was applied to another query kind.
    QueryOptionNotApplicable,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidAssetVersion => "asset version must be non-empty and normalized",
            Self::InvalidAssetFileName => {
                "asset file name must be one safe relative path component"
            }
            Self::InvalidAssetSource => "asset source must be non-empty and normalized",
            Self::InvalidAssetByteSize => "asset byte size must be greater than zero",
            Self::InvalidPoint => "point must contain finite, in-range coordinates",
            Self::InvalidQueryText => "query text must be non-empty and normalized",
            Self::InvalidQueryLimit => "query limit must be between 1 and 100",
            Self::QueryOptionNotApplicable => "query option is not applicable to this query kind",
        })
    }
}

impl std::error::Error for Error {}

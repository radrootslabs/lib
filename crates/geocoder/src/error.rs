use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GeoNamesAssetDownloadPhase {
    Setup,
    Connect,
    Response,
    Read,
    Total,
}

impl std::fmt::Display for GeoNamesAssetDownloadPhase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Setup => "setup",
            Self::Connect => "connect",
            Self::Response => "response",
            Self::Read => "read",
            Self::Total => "total",
        })
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GeoNamesAssetDownloadError {
    #[error("HTTP worker runtime failed: {detail}")]
    Runtime { detail: String },
    #[error("HTTP worker terminated unexpectedly")]
    WorkerTerminated,
    #[error("{phase} request failed: {detail}")]
    Request {
        phase: GeoNamesAssetDownloadPhase,
        detail: String,
    },
    #[error("HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("{phase} timeout after {timeout_ms} ms")]
    Timeout {
        phase: GeoNamesAssetDownloadPhase,
        timeout_ms: u64,
    },
    #[error("response read failed: {detail}")]
    Read { detail: String },
    #[error("response body exceeds maximum {maximum} bytes; observed at least {observed_at_least}")]
    ResponseTooLarge {
        maximum: u64,
        observed_at_least: u64,
    },
}

#[derive(Debug, Error)]
pub enum GeocoderError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] sqlx::Error),
    #[error("sqlite connection lock is unavailable")]
    SqliteConnectionLockUnavailable,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid GeoNames asset URL {url}")]
    InvalidAssetUrl { url: String },
    #[error("invalid GeoNames asset host for {url}: expected {expected_host}, got {actual_host}")]
    InvalidAssetHost {
        url: String,
        expected_host: String,
        actual_host: String,
    },
    #[error("invalid GeoNames asset length at {path}: expected {expected}, got {actual}")]
    InvalidAssetLength {
        path: std::path::PathBuf,
        expected: u64,
        actual: u64,
    },
    #[error("invalid GeoNames asset SHA-256 at {path}: expected {expected}, got {actual}")]
    InvalidAssetSha256 {
        path: std::path::PathBuf,
        expected: String,
        actual: String,
    },
    #[error("invalid GeoNames asset SQLite database at {path}: {detail}")]
    InvalidAssetSqlite {
        path: std::path::PathBuf,
        detail: String,
    },
    #[error("invalid GeoNames asset SQLite integrity at {path}: {result}")]
    InvalidAssetIntegrity {
        path: std::path::PathBuf,
        result: String,
    },
    #[error("invalid GeoNames asset schema at {path}: {detail}")]
    InvalidAssetSchema {
        path: std::path::PathBuf,
        detail: String,
    },
    #[error("GeoNames asset lock is unavailable at {path}")]
    AssetLockUnavailable { path: std::path::PathBuf },
    #[error("GeoNames asset download failed for {url}: {source}")]
    AssetDownload {
        url: String,
        #[source]
        source: GeoNamesAssetDownloadError,
    },
    #[error("country center not found for {country_id}")]
    CountryCenterNotFound { country_id: String },
}

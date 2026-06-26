use thiserror::Error;

#[derive(Debug, Error)]
pub enum GeocoderError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
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
        source: reqwest::Error,
    },
    #[error("country center not found for {country_id}")]
    CountryCenterNotFound { country_id: String },
}

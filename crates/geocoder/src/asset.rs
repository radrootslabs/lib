use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use radroots_runtime_paths::default_shared_geonames_database_path_from_cache_root;
use sha2::{Digest, Sha256};
use sqlx::Connection;
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};
use url::Url;

use crate::{GeoNamesAssetDownloadError, GeoNamesAssetDownloadPhase, GeocoderError};

pub const GEONAMES_ASSET_VERSION: &str = "1.0";
pub const GEONAMES_ASSET_FILE_NAME: &str = "geonames-1.0.db";
pub const GEONAMES_ASSET_URL: &str = "https://assets.radroots.io/data/geonames/geonames-1.0.db";
pub const GEONAMES_ASSET_HOST: &str = "assets.radroots.io";
#[cfg(not(feature = "test-fixture-geonames-asset"))]
pub const GEONAMES_ASSET_BYTE_SIZE: u64 = 12_951_552;
#[cfg(feature = "test-fixture-geonames-asset")]
pub const GEONAMES_ASSET_BYTE_SIZE: u64 = 20_480;
#[cfg(not(feature = "test-fixture-geonames-asset"))]
pub const GEONAMES_ASSET_SHA256: &str =
    "6ca5f1a324de02922d40b1ff33eedf3a5a133c978de921eee5130a0c7876079c";
#[cfg(feature = "test-fixture-geonames-asset")]
pub const GEONAMES_ASSET_SHA256: &str =
    "3f81face93a88cda0a0e0a1c3611c2280177061b1a2bbe9ced42526c762885b6";

pub const GEONAMES_1_0_ASSET: GeoNamesAssetSpec = GeoNamesAssetSpec {
    version: GEONAMES_ASSET_VERSION,
    file_name: GEONAMES_ASSET_FILE_NAME,
    url: GEONAMES_ASSET_URL,
    allowed_host: GEONAMES_ASSET_HOST,
    byte_size: GEONAMES_ASSET_BYTE_SIZE,
    sha256: GEONAMES_ASSET_SHA256,
};

const GEONAMES_HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const GEONAMES_HTTP_RESPONSE_TIMEOUT: Duration = Duration::from_secs(15);
const GEONAMES_HTTP_READ_TIMEOUT: Duration = Duration::from_secs(15);
const GEONAMES_HTTP_TOTAL_TIMEOUT: Duration = Duration::from_secs(120);
const GEONAMES_HTTP_RUNTIME_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(250);
const GEONAMES_HTTP_INITIAL_CAPACITY_MAX: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeoNamesAssetSpec {
    pub version: &'static str,
    pub file_name: &'static str,
    pub url: &'static str,
    pub allowed_host: &'static str,
    pub byte_size: u64,
    pub sha256: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GeoNamesAssetState {
    Missing,
    Available,
    Invalid,
    Refreshed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeoNamesAssetStatus {
    pub state: GeoNamesAssetState,
    pub version: String,
    pub path: PathBuf,
    pub byte_size: Option<u64>,
    pub sha256: Option<String>,
    pub validation_error: Option<String>,
}

pub trait GeoNamesAssetFetcher {
    /// Returns a complete asset from a trusted or injected source.
    ///
    /// The default writer adapter bounds this result before installation.
    /// Network fetchers should override [`Self::fetch_to_writer`] to avoid
    /// buffering the complete asset.
    fn fetch(&self, url: &str) -> Result<Vec<u8>, GeocoderError>;

    /// Returns a complete asset after enforcing its maximum logical size.
    fn fetch_with_max_bytes(
        &self,
        url: &str,
        maximum_bytes: u64,
    ) -> Result<Vec<u8>, GeocoderError> {
        let bytes = self.fetch(url)?;
        let actual = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if actual > maximum_bytes {
            return Err(asset_download_error(
                url,
                GeoNamesAssetDownloadError::ResponseTooLarge {
                    maximum: maximum_bytes,
                    observed_at_least: actual,
                },
            ));
        }
        Ok(bytes)
    }

    /// Writes a logically bounded asset to an installation destination.
    fn fetch_to_writer(
        &self,
        url: &str,
        maximum_bytes: u64,
        destination: &mut (dyn Write + Send),
    ) -> Result<(), GeocoderError> {
        let bytes = self.fetch_with_max_bytes(url, maximum_bytes)?;
        destination.write_all(&bytes)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct GeoNamesBlockingHttpFetcher;

impl GeoNamesAssetFetcher for GeoNamesBlockingHttpFetcher {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fetch(&self, url: &str) -> Result<Vec<u8>, GeocoderError> {
        self.fetch_with_max_bytes(url, GEONAMES_ASSET_BYTE_SIZE)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fetch_with_max_bytes(
        &self,
        url: &str,
        maximum_bytes: u64,
    ) -> Result<Vec<u8>, GeocoderError> {
        let initial_capacity = usize::try_from(maximum_bytes)
            .unwrap_or(usize::MAX)
            .min(GEONAMES_HTTP_INITIAL_CAPACITY_MAX);
        let mut bytes = Vec::with_capacity(initial_capacity);
        fetch_http_asset_to_writer_with_policy(
            url,
            maximum_bytes,
            &mut bytes,
            GeoNamesHttpFetchPolicy::production(),
        )?;
        Ok(bytes)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fetch_to_writer(
        &self,
        url: &str,
        maximum_bytes: u64,
        destination: &mut (dyn Write + Send),
    ) -> Result<(), GeocoderError> {
        fetch_http_asset_to_writer_with_policy(
            url,
            maximum_bytes,
            destination,
            GeoNamesHttpFetchPolicy::production(),
        )
    }
}

#[derive(Clone, Copy)]
struct GeoNamesHttpFetchPolicy {
    connect_timeout: Duration,
    response_timeout: Duration,
    read_timeout: Duration,
    total_timeout: Duration,
    runtime_shutdown_timeout: Duration,
}

impl GeoNamesHttpFetchPolicy {
    const fn production() -> Self {
        Self {
            connect_timeout: GEONAMES_HTTP_CONNECT_TIMEOUT,
            response_timeout: GEONAMES_HTTP_RESPONSE_TIMEOUT,
            read_timeout: GEONAMES_HTTP_READ_TIMEOUT,
            total_timeout: GEONAMES_HTTP_TOTAL_TIMEOUT,
            runtime_shutdown_timeout: GEONAMES_HTTP_RUNTIME_SHUTDOWN_TIMEOUT,
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn fetch_http_asset_to_writer_with_policy(
    url: &str,
    maximum_bytes: u64,
    destination: &mut (dyn Write + Send),
    policy: GeoNamesHttpFetchPolicy,
) -> Result<(), GeocoderError> {
    thread::scope(|scope| {
        let worker_url = url.to_owned();
        let worker = thread::Builder::new()
            .name("radroots-geonames-http".to_owned())
            .spawn_scoped(scope, move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|source| {
                        asset_download_error(
                            &worker_url,
                            GeoNamesAssetDownloadError::Runtime {
                                detail: source.to_string(),
                            },
                        )
                    })?;
                let result = runtime.block_on(async {
                    tokio::time::timeout(
                        policy.total_timeout,
                        fetch_http_asset_async(&worker_url, maximum_bytes, destination, policy),
                    )
                    .await
                    .map_err(|_| {
                        timeout_error(
                            &worker_url,
                            GeoNamesAssetDownloadPhase::Total,
                            policy.total_timeout,
                        )
                    })?
                });
                runtime.shutdown_timeout(policy.runtime_shutdown_timeout);
                result
            })
            .map_err(|source| {
                asset_download_error(
                    url,
                    GeoNamesAssetDownloadError::Runtime {
                        detail: source.to_string(),
                    },
                )
            })?;
        worker
            .join()
            .map_err(|_| asset_download_error(url, GeoNamesAssetDownloadError::WorkerTerminated))?
    })
}

async fn fetch_http_asset_async(
    url: &str,
    maximum_bytes: u64,
    destination: &mut (dyn Write + Send),
    policy: GeoNamesHttpFetchPolicy,
) -> Result<(), GeocoderError> {
    let client = reqwest::Client::builder()
        .connect_timeout(policy.connect_timeout)
        .hickory_dns(true)
        // Source validation grants authority to one exact host.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|source| request_error(url, GeoNamesAssetDownloadPhase::Setup, source))?;
    let mut response = tokio::time::timeout(policy.response_timeout, client.get(url).send())
        .await
        .map_err(|_| {
            timeout_error(
                url,
                GeoNamesAssetDownloadPhase::Response,
                policy.response_timeout,
            )
        })?
        .map_err(|source| {
            if source.is_timeout() && source.is_connect() {
                timeout_error(
                    url,
                    GeoNamesAssetDownloadPhase::Connect,
                    policy.connect_timeout,
                )
            } else {
                let phase = if source.is_connect() {
                    GeoNamesAssetDownloadPhase::Connect
                } else {
                    GeoNamesAssetDownloadPhase::Response
                };
                request_error(url, phase, source)
            }
        })?;

    if !response.status().is_success() {
        return Err(asset_download_error(
            url,
            GeoNamesAssetDownloadError::HttpStatus {
                status: response.status().as_u16(),
            },
        ));
    }
    if let Some(content_length) = response.content_length()
        && content_length > maximum_bytes
    {
        return Err(asset_download_error(
            url,
            GeoNamesAssetDownloadError::ResponseTooLarge {
                maximum: maximum_bytes,
                observed_at_least: content_length,
            },
        ));
    }

    let mut downloaded = 0_u64;
    loop {
        let chunk = tokio::time::timeout(policy.read_timeout, response.chunk())
            .await
            .map_err(|_| timeout_error(url, GeoNamesAssetDownloadPhase::Read, policy.read_timeout))?
            .map_err(|source| response_read_error(url, source))?;
        let Some(chunk) = chunk else {
            break;
        };
        let chunk_len = u64::try_from(chunk.len()).unwrap_or(u64::MAX);
        let observed_at_least = downloaded.saturating_add(chunk_len);
        if observed_at_least > maximum_bytes {
            return Err(asset_download_error(
                url,
                GeoNamesAssetDownloadError::ResponseTooLarge {
                    maximum: maximum_bytes,
                    observed_at_least,
                },
            ));
        }
        destination.write_all(&chunk)?;
        downloaded = observed_at_least;
    }
    destination.flush()?;
    Ok(())
}

fn request_error(
    url: &str,
    phase: GeoNamesAssetDownloadPhase,
    source: reqwest::Error,
) -> GeocoderError {
    asset_download_error(
        url,
        GeoNamesAssetDownloadError::Request {
            phase,
            detail: source.to_string(),
        },
    )
}

fn response_read_error(url: &str, source: reqwest::Error) -> GeocoderError {
    asset_download_error(
        url,
        GeoNamesAssetDownloadError::Read {
            detail: source.to_string(),
        },
    )
}

fn timeout_error(url: &str, phase: GeoNamesAssetDownloadPhase, timeout: Duration) -> GeocoderError {
    asset_download_error(
        url,
        GeoNamesAssetDownloadError::Timeout {
            phase,
            timeout_ms: duration_millis(timeout),
        },
    )
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn asset_download_error(url: &str, source: GeoNamesAssetDownloadError) -> GeocoderError {
    GeocoderError::AssetDownload {
        url: url.to_owned(),
        source,
    }
}

pub fn default_geonames_asset_path_from_cache_root(cache_root: impl AsRef<Path>) -> PathBuf {
    default_shared_geonames_database_path_from_cache_root(cache_root, GEONAMES_ASSET_VERSION)
}

pub fn inspect_default_geonames_asset_in_cache_root(
    cache_root: impl AsRef<Path>,
) -> Result<GeoNamesAssetStatus, GeocoderError> {
    inspect_geonames_asset_path(
        default_geonames_asset_path_from_cache_root(cache_root),
        &GEONAMES_1_0_ASSET,
    )
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn ensure_default_geonames_asset_in_cache_root(
    cache_root: impl AsRef<Path>,
) -> Result<GeoNamesAssetStatus, GeocoderError> {
    let fetcher = GeoNamesBlockingHttpFetcher;
    ensure_geonames_asset_in_cache_root_with_fetcher(cache_root, &GEONAMES_1_0_ASSET, &fetcher)
}

pub fn ensure_geonames_asset_in_cache_root_with_fetcher<F>(
    cache_root: impl AsRef<Path>,
    spec: &GeoNamesAssetSpec,
    fetcher: &F,
) -> Result<GeoNamesAssetStatus, GeocoderError>
where
    F: GeoNamesAssetFetcher,
{
    let path = default_shared_geonames_database_path_from_cache_root(cache_root, spec.version);
    ensure_geonames_asset_path_with_fetcher(path, spec, fetcher)
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn ensure_geonames_asset_path_with_fetcher<F>(
    path: impl AsRef<Path>,
    spec: &GeoNamesAssetSpec,
    fetcher: &F,
) -> Result<GeoNamesAssetStatus, GeocoderError>
where
    F: GeoNamesAssetFetcher,
{
    validate_geonames_asset_spec_source(spec)?;
    let path = path.as_ref();
    let inspection = inspect_geonames_asset_path(path, spec)?;
    if inspection.state == GeoNamesAssetState::Available {
        return Ok(inspection);
    }
    let _lock = GeoNamesAssetLock::acquire(lock_path_for_asset(path))?;
    let inspection = inspect_geonames_asset_path(path, spec)?;
    if inspection.state == GeoNamesAssetState::Available {
        return Ok(inspection);
    }
    install_geonames_asset_with_fetcher(path, spec, fetcher)?;
    let mut status = validate_geonames_asset_file(path, spec)?;
    status.state = GeoNamesAssetState::Refreshed;
    Ok(status)
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn inspect_geonames_asset_path(
    path: impl AsRef<Path>,
    spec: &GeoNamesAssetSpec,
) -> Result<GeoNamesAssetStatus, GeocoderError> {
    let path = path.as_ref();
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(GeoNamesAssetStatus {
                state: GeoNamesAssetState::Missing,
                version: spec.version.to_owned(),
                path: path.to_path_buf(),
                byte_size: None,
                sha256: None,
                validation_error: None,
            });
        }
        Err(error) => return Err(GeocoderError::Io(error)),
    };
    let actual_size = metadata.len();
    let actual_sha256 = sha256_file(path)?;
    match validate_geonames_asset_file(path, spec) {
        Ok(status) => Ok(status),
        Err(error) if is_invalid_asset_error(&error) => Ok(GeoNamesAssetStatus {
            state: GeoNamesAssetState::Invalid,
            version: spec.version.to_owned(),
            path: path.to_path_buf(),
            byte_size: Some(actual_size),
            sha256: Some(actual_sha256),
            validation_error: Some(error.to_string()),
        }),
        Err(error) => Err(error),
    }
}

pub fn validate_geonames_asset_file(
    path: impl AsRef<Path>,
    spec: &GeoNamesAssetSpec,
) -> Result<GeoNamesAssetStatus, GeocoderError> {
    let path = path.as_ref();
    let metadata = fs::metadata(path)?;
    let actual_size = metadata.len();
    if actual_size != spec.byte_size {
        return Err(GeocoderError::InvalidAssetLength {
            path: path.to_path_buf(),
            expected: spec.byte_size,
            actual: actual_size,
        });
    }
    let actual_sha256 = sha256_file(path)?;
    if actual_sha256 != spec.sha256 {
        return Err(GeocoderError::InvalidAssetSha256 {
            path: path.to_path_buf(),
            expected: spec.sha256.to_owned(),
            actual: actual_sha256,
        });
    }
    validate_sqlite_integrity_and_schema(path)?;
    Ok(GeoNamesAssetStatus {
        state: GeoNamesAssetState::Available,
        version: spec.version.to_owned(),
        path: path.to_path_buf(),
        byte_size: Some(actual_size),
        sha256: Some(actual_sha256),
        validation_error: None,
    })
}

pub fn validate_geonames_asset_spec_source(spec: &GeoNamesAssetSpec) -> Result<(), GeocoderError> {
    let parsed = Url::parse(spec.url).map_err(|_| GeocoderError::InvalidAssetUrl {
        url: spec.url.to_owned(),
    })?;
    if parsed.scheme() != "https" {
        return Err(GeocoderError::InvalidAssetUrl {
            url: spec.url.to_owned(),
        });
    }
    let actual_host = parsed.host_str().unwrap_or("").to_owned();
    if actual_host != spec.allowed_host {
        return Err(GeocoderError::InvalidAssetHost {
            url: spec.url.to_owned(),
            expected_host: spec.allowed_host.to_owned(),
            actual_host,
        });
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn install_geonames_asset_with_fetcher<F>(
    path: &Path,
    spec: &GeoNamesAssetSpec,
    fetcher: &F,
) -> Result<(), GeocoderError>
where
    F: GeoNamesAssetFetcher,
{
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let mut tempfile = tempfile::Builder::new()
        .prefix(&format!(".{}.", spec.file_name))
        .suffix(".tmp")
        .tempfile_in(parent)?;
    let identity = {
        let mut writer = GeoNamesAssetIdentityWriter::new(tempfile.as_file_mut(), spec.byte_size);
        fetcher.fetch_to_writer(spec.url, spec.byte_size, &mut writer)?;
        writer.flush()?;
        writer.finish()
    };
    validate_downloaded_asset_identity(path, spec, &identity)?;
    tempfile.as_file_mut().sync_all()?;
    validate_sqlite_integrity_and_schema(tempfile.path())?;
    tempfile
        .persist(path)
        .map(|_| ())
        .map_err(|error| GeocoderError::Io(error.error))
}

struct GeoNamesAssetIdentity {
    byte_size: u64,
    sha256: String,
}

struct GeoNamesAssetIdentityWriter<'a> {
    destination: &'a mut File,
    maximum_bytes: u64,
    byte_size: u64,
    hasher: Sha256,
}

impl<'a> GeoNamesAssetIdentityWriter<'a> {
    fn new(destination: &'a mut File, maximum_bytes: u64) -> Self {
        Self {
            destination,
            maximum_bytes,
            byte_size: 0,
            hasher: Sha256::new(),
        }
    }

    fn finish(self) -> GeoNamesAssetIdentity {
        GeoNamesAssetIdentity {
            byte_size: self.byte_size,
            sha256: hex::encode(self.hasher.finalize()),
        }
    }
}

impl Write for GeoNamesAssetIdentityWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let requested = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        let observed_at_least = self.byte_size.saturating_add(requested);
        if observed_at_least > self.maximum_bytes {
            return Err(std::io::Error::new(
                std::io::ErrorKind::FileTooLarge,
                format!(
                    "GeoNames asset exceeds maximum {} bytes; observed at least {observed_at_least}",
                    self.maximum_bytes
                ),
            ));
        }
        let written = self.destination.write(bytes)?;
        self.byte_size = self
            .byte_size
            .saturating_add(u64::try_from(written).unwrap_or(u64::MAX));
        self.hasher.update(&bytes[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.destination.flush()
    }
}

fn validate_downloaded_asset_identity(
    path: &Path,
    spec: &GeoNamesAssetSpec,
    identity: &GeoNamesAssetIdentity,
) -> Result<(), GeocoderError> {
    if identity.byte_size != spec.byte_size {
        return Err(GeocoderError::InvalidAssetLength {
            path: path.to_path_buf(),
            expected: spec.byte_size,
            actual: identity.byte_size,
        });
    }
    if identity.sha256 != spec.sha256 {
        return Err(GeocoderError::InvalidAssetSha256 {
            path: path.to_path_buf(),
            expected: spec.sha256.to_owned(),
            actual: identity.sha256.clone(),
        });
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn validate_sqlite_integrity_and_schema(path: &Path) -> Result<(), GeocoderError> {
    let mut conn = futures_executor::block_on(SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(path).read_only(true),
    ))
    .map_err(|error| GeocoderError::InvalidAssetSqlite {
        path: path.to_path_buf(),
        detail: error.to_string(),
    })?;
    validate_sqlite_integrity(path, &mut conn)?;
    for query in [
        "SELECT id, name FROM countries LIMIT 1",
        "SELECT country_id, id, name FROM admin1 LIMIT 1",
        "SELECT id, name, country_id, admin1_id FROM features LIMIT 1",
        "SELECT feature_id, latitude, longitude FROM coordinates LIMIT 1",
        "SELECT id, name, admin1_id, admin1_name, country_id, country_name, latitude, longitude FROM geonames LIMIT 1",
    ] {
        futures_executor::block_on(sqlx::query(query).fetch_optional(&mut conn))
            .map(|_| ())
            .map_err(|error| GeocoderError::InvalidAssetSchema {
                path: path.to_path_buf(),
                detail: error.to_string(),
            })?;
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn validate_sqlite_integrity(
    path: &Path,
    conn: &mut SqliteConnection,
) -> Result<(), GeocoderError> {
    let results = futures_executor::block_on(
        sqlx::query_scalar::<_, String>("PRAGMA integrity_check").fetch_all(conn),
    )
    .map_err(|error| GeocoderError::InvalidAssetSqlite {
        path: path.to_path_buf(),
        detail: error.to_string(),
    })?;
    if results.as_slice() == ["ok"] {
        return Ok(());
    }
    Err(GeocoderError::InvalidAssetIntegrity {
        path: path.to_path_buf(),
        result: results.join("; "),
    })
}

fn sha256_file(path: &Path) -> Result<String, GeocoderError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn lock_path_for_asset(path: &Path) -> PathBuf {
    path.with_extension("db.lock")
}

fn is_invalid_asset_error(error: &GeocoderError) -> bool {
    matches!(
        error,
        GeocoderError::InvalidAssetLength { .. }
            | GeocoderError::InvalidAssetSha256 { .. }
            | GeocoderError::InvalidAssetSqlite { .. }
            | GeocoderError::InvalidAssetIntegrity { .. }
            | GeocoderError::InvalidAssetSchema { .. }
    )
}

struct GeoNamesAssetLock {
    path: PathBuf,
}

impl GeoNamesAssetLock {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn acquire(path: PathBuf) -> Result<Self, GeocoderError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => Ok(Self { path }),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(GeocoderError::AssetLockUnavailable { path })
            }
            Err(error) => Err(GeocoderError::Io(error)),
        }
    }
}

impl Drop for GeoNamesAssetLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::{Path, PathBuf};
    use std::thread;
    use std::time::{Duration, Instant};

    use sha2::Digest;
    use sqlx::Connection;
    use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};

    use super::{
        GEONAMES_ASSET_HOST, GeoNamesAssetFetcher, GeoNamesAssetIdentityWriter, GeoNamesAssetSpec,
        GeoNamesAssetState, GeoNamesHttpFetchPolicy, ensure_geonames_asset_path_with_fetcher,
        fetch_http_asset_to_writer_with_policy, inspect_geonames_asset_path,
        is_invalid_asset_error, lock_path_for_asset, validate_geonames_asset_file,
        validate_geonames_asset_spec_source,
    };
    use crate::{GeoNamesAssetDownloadError, GeoNamesAssetDownloadPhase, GeocoderError};

    const TEST_URL: &str = "https://assets.radroots.io/data/geonames/geonames-test.db";

    struct BytesFetcher {
        bytes: Vec<u8>,
        calls: Cell<usize>,
    }

    impl GeoNamesAssetFetcher for BytesFetcher {
        fn fetch(&self, _url: &str) -> Result<Vec<u8>, GeocoderError> {
            self.calls.set(self.calls.get() + 1);
            Ok(self.bytes.clone())
        }
    }

    struct WriterOnlyFetcher {
        bytes: Vec<u8>,
        calls: Cell<usize>,
    }

    impl GeoNamesAssetFetcher for WriterOnlyFetcher {
        fn fetch(&self, _url: &str) -> Result<Vec<u8>, GeocoderError> {
            panic!("writer-oriented install must not call the Vec adapter")
        }

        fn fetch_to_writer(
            &self,
            _url: &str,
            maximum_bytes: u64,
            destination: &mut (dyn Write + Send),
        ) -> Result<(), GeocoderError> {
            self.calls.set(self.calls.get() + 1);
            assert!(self.bytes.len() as u64 <= maximum_bytes);
            for chunk in self.bytes.chunks(257) {
                destination.write_all(chunk)?;
            }
            Ok(())
        }
    }

    struct RejectingWriter;

    impl Write for RejectingWriter {
        fn write(&mut self, _bytes: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "injected writer failure",
            ))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn default_fetch_adapters_enforce_bounds_and_propagate_writer_errors() {
        let fetcher = BytesFetcher {
            bytes: b"asset".to_vec(),
            calls: Cell::new(0),
        };
        assert_eq!(fetcher.fetch_with_max_bytes(TEST_URL, 5).unwrap(), b"asset");
        assert!(matches!(
            fetcher.fetch_with_max_bytes(TEST_URL, 4),
            Err(GeocoderError::AssetDownload {
                source: GeoNamesAssetDownloadError::ResponseTooLarge {
                    maximum: 4,
                    observed_at_least: 5,
                },
                ..
            })
        ));

        let mut destination = Vec::new();
        fetcher
            .fetch_to_writer(TEST_URL, 5, &mut destination)
            .unwrap();
        assert_eq!(destination, b"asset");

        assert!(matches!(
            fetcher.fetch_to_writer(TEST_URL, 5, &mut RejectingWriter),
            Err(GeocoderError::Io(error)) if error.kind() == std::io::ErrorKind::BrokenPipe
        ));

        let tempdir = tempfile::tempdir().expect("bounded writer tempdir");
        let mut bounded_destination =
            fs::File::create(tempdir.path().join("bounded.bin")).expect("bounded writer file");
        let error = GeoNamesAssetIdentityWriter::new(&mut bounded_destination, 4)
            .write_all(b"asset")
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::FileTooLarge);
    }

    #[test]
    fn blocking_http_fetch_rejects_oversized_declared_content_length() {
        let server = LoopbackHttpServer::spawn(|mut stream| {
            read_request(&mut stream);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\n")
                .expect("oversized response headers");
        });

        assert!(matches!(
            fetch_http_bytes(&server.url, 5, test_http_policy()),
            Err(GeocoderError::AssetDownload {
                source: GeoNamesAssetDownloadError::ResponseTooLarge {
                    maximum: 5,
                    observed_at_least: 6,
                },
                ..
            })
        ));
    }

    #[test]
    fn blocking_http_fetch_streams_a_bounded_success_response() {
        let server = LoopbackHttpServer::spawn(|mut stream| {
            read_request(&mut stream);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
                )
                .expect("success response");
        });

        let bytes = fetch_http_bytes(&server.url, 5, test_http_policy()).expect("bounded download");
        assert_eq!(bytes, b"hello");
    }

    #[test]
    fn blocking_http_fetch_allows_progress_to_exceed_read_timeout_cumulatively() {
        let server = LoopbackHttpServer::spawn(|mut stream| {
            read_request(&mut stream);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\n")
                .expect("response headers");
            for byte in b"hello" {
                stream.write_all(&[*byte]).expect("response byte");
                stream.flush().expect("response flush");
                thread::sleep(Duration::from_millis(75));
            }
        });
        let policy = GeoNamesHttpFetchPolicy {
            connect_timeout: Duration::from_millis(500),
            response_timeout: Duration::from_secs(1),
            read_timeout: Duration::from_millis(200),
            total_timeout: Duration::from_secs(2),
            runtime_shutdown_timeout: Duration::from_millis(100),
        };

        let bytes = fetch_http_bytes(&server.url, 5, policy).expect("progressing download");
        assert_eq!(bytes, b"hello");
    }

    #[test]
    fn blocking_http_fetch_rejects_oversized_chunked_response_without_buffering_it() {
        let server = LoopbackHttpServer::spawn(|mut stream| {
            read_request(&mut stream);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n4\r\nabcd\r\n4\r\nefgh\r\n0\r\n\r\n",
                )
                .expect("chunked response");
        });

        assert!(matches!(
            fetch_http_bytes(&server.url, 5, test_http_policy()),
            Err(GeocoderError::AssetDownload {
                source: GeoNamesAssetDownloadError::ResponseTooLarge {
                    maximum: 5,
                    observed_at_least,
                },
                ..
            }) if observed_at_least >= 6
        ));
    }

    #[test]
    fn blocking_http_fetch_times_out_a_stalled_response_body() {
        let server = LoopbackHttpServer::spawn(|mut stream| {
            read_request(&mut stream);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n")
                .expect("response headers");
            stream.flush().expect("response flush");
            thread::sleep(Duration::from_millis(600));
        });
        let policy = GeoNamesHttpFetchPolicy {
            connect_timeout: Duration::from_millis(500),
            response_timeout: Duration::from_secs(1),
            read_timeout: Duration::from_millis(200),
            total_timeout: Duration::from_secs(2),
            runtime_shutdown_timeout: Duration::from_millis(100),
        };

        let started_at = Instant::now();
        let result = fetch_http_bytes(&server.url, 1, policy);
        let elapsed = started_at.elapsed();
        assert!(
            matches!(
                result,
                Err(GeocoderError::AssetDownload {
                    source: GeoNamesAssetDownloadError::Timeout {
                        phase: GeoNamesAssetDownloadPhase::Read,
                        timeout_ms: 200,
                    },
                    ..
                })
            ),
            "{result:?}"
        );
        assert!(elapsed >= Duration::from_millis(100), "{elapsed:?}");
        assert!(elapsed < Duration::from_secs(3), "{elapsed:?}");
    }

    #[test]
    fn blocking_http_fetch_reports_response_timeout_without_elapsed_heuristics() {
        let server = LoopbackHttpServer::spawn(|mut stream| {
            read_request(&mut stream);
            thread::sleep(Duration::from_millis(600));
        });
        let policy = GeoNamesHttpFetchPolicy {
            connect_timeout: Duration::from_millis(500),
            response_timeout: Duration::from_millis(200),
            read_timeout: Duration::from_secs(1),
            total_timeout: Duration::from_secs(2),
            runtime_shutdown_timeout: Duration::from_millis(100),
        };

        assert!(matches!(
            fetch_http_bytes(&server.url, 1, policy),
            Err(GeocoderError::AssetDownload {
                source: GeoNamesAssetDownloadError::Timeout {
                    phase: GeoNamesAssetDownloadPhase::Response,
                    timeout_ms: 200,
                },
                ..
            })
        ));
    }

    #[test]
    fn blocking_http_fetch_classifies_refused_connections() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("reserve loopback port");
        let address = listener.local_addr().expect("loopback address");
        drop(listener);
        let url = format!("http://{address}/geonames.db");

        assert!(matches!(
            fetch_http_bytes(&url, 1, test_http_policy()),
            Err(GeocoderError::AssetDownload {
                source: GeoNamesAssetDownloadError::Request {
                    phase: GeoNamesAssetDownloadPhase::Connect,
                    ..
                },
                ..
            })
        ));
    }

    #[test]
    fn blocking_http_fetch_enforces_total_deadline_across_progressing_reads() {
        let server = LoopbackHttpServer::spawn(|mut stream| {
            read_request(&mut stream);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\n")
                .expect("response headers");
            for byte in b"slow" {
                if stream.write_all(&[*byte]).is_err() {
                    break;
                }
                let _ = stream.flush();
                thread::sleep(Duration::from_millis(150));
            }
        });
        let policy = GeoNamesHttpFetchPolicy {
            connect_timeout: Duration::from_millis(500),
            response_timeout: Duration::from_secs(1),
            read_timeout: Duration::from_millis(500),
            total_timeout: Duration::from_millis(250),
            runtime_shutdown_timeout: Duration::from_millis(100),
        };

        let started_at = Instant::now();
        let result = fetch_http_bytes(&server.url, 4, policy);
        let elapsed = started_at.elapsed();
        assert!(matches!(
            result,
            Err(GeocoderError::AssetDownload {
                source: GeoNamesAssetDownloadError::Timeout {
                    phase: GeoNamesAssetDownloadPhase::Total,
                    timeout_ms: 250,
                },
                ..
            })
        ));
        assert!(elapsed >= Duration::from_millis(125), "{elapsed:?}");
        assert!(elapsed < Duration::from_secs(3), "{elapsed:?}");
    }

    #[test]
    fn blocking_http_fetch_does_not_follow_redirects_outside_the_validated_source() {
        let server = LoopbackHttpServer::spawn(|mut stream| {
            read_request(&mut stream);
            stream
                .write_all(
                    b"HTTP/1.1 302 Found\r\nLocation: https://example.com/geonames.db\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .expect("redirect response");
        });

        assert!(matches!(
            fetch_http_bytes(&server.url, 5, test_http_policy()),
            Err(GeocoderError::AssetDownload {
                source: GeoNamesAssetDownloadError::HttpStatus { status: 302 },
                ..
            })
        ));
    }

    #[test]
    fn geonames_asset_missing_available_invalid_and_refreshed_states_are_reported() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let bytes = fixture_database_bytes();
        let spec = fixture_spec(&bytes, TEST_URL);
        let target = tempdir.path().join("shared/geonames/geonames-test.db");

        let missing = inspect_geonames_asset_path(&target, &spec).expect("missing inspection");
        assert_eq!(missing.state, GeoNamesAssetState::Missing);
        assert_eq!(missing.byte_size, None);

        fs::create_dir_all(target.parent().expect("target parent")).expect("target parent");
        fs::write(&target, b"not sqlite").expect("write invalid");
        let invalid = inspect_geonames_asset_path(&target, &spec).expect("invalid inspection");
        assert_eq!(invalid.state, GeoNamesAssetState::Invalid);
        assert!(
            invalid
                .validation_error
                .expect("validation error")
                .contains("length")
        );

        let fetcher = BytesFetcher {
            bytes,
            calls: Cell::new(0),
        };
        let refreshed =
            ensure_geonames_asset_path_with_fetcher(&target, &spec, &fetcher).expect("refresh");
        assert_eq!(refreshed.state, GeoNamesAssetState::Refreshed);
        assert_eq!(fetcher.calls.get(), 1);

        let available =
            ensure_geonames_asset_path_with_fetcher(&target, &spec, &fetcher).expect("available");
        assert_eq!(available.state, GeoNamesAssetState::Available);
        assert_eq!(fetcher.calls.get(), 1);
    }

    #[test]
    fn geonames_asset_install_uses_streaming_writer_and_atomic_replacement() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let bytes = fixture_database_bytes();
        let spec = fixture_spec(&bytes, TEST_URL);
        let target = tempdir.path().join("geonames-test.db");
        let fetcher = WriterOnlyFetcher {
            bytes,
            calls: Cell::new(0),
        };

        let status =
            ensure_geonames_asset_path_with_fetcher(&target, &spec, &fetcher).expect("install");
        assert_eq!(status.state, GeoNamesAssetState::Refreshed);
        assert_eq!(fetcher.calls.get(), 1);
        assert_no_install_tempfiles(tempdir.path());
    }

    #[test]
    fn geonames_asset_failed_identity_validation_preserves_existing_target() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let bytes = fixture_database_bytes();
        let spec = fixture_spec(&bytes, TEST_URL);
        let target = tempdir.path().join("geonames-test.db");
        let previous = b"previous asset";
        fs::write(&target, previous).expect("previous target");
        let mut wrong_bytes = bytes;
        let last = wrong_bytes.last_mut().expect("nonempty fixture");
        *last ^= 0x01;
        let fetcher = WriterOnlyFetcher {
            bytes: wrong_bytes,
            calls: Cell::new(0),
        };

        assert!(matches!(
            ensure_geonames_asset_path_with_fetcher(&target, &spec, &fetcher),
            Err(GeocoderError::InvalidAssetSha256 { .. })
        ));
        assert_eq!(fs::read(&target).expect("preserved target"), previous);
        assert_eq!(fetcher.calls.get(), 1);
        assert_no_install_tempfiles(tempdir.path());
    }

    #[test]
    fn geonames_asset_rejects_wrong_host_length_hash_sqlite_and_schema() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let bytes = fixture_database_bytes();

        let bad_host_spec = fixture_spec(
            &bytes,
            "https://static.radroots.io/data/geonames/geonames-test.db",
        );
        assert!(matches!(
            validate_geonames_asset_spec_source(&bad_host_spec),
            Err(GeocoderError::InvalidAssetHost { .. })
        ));
        for url in [
            "http://assets.radroots.io/data/geonames/geonames-test.db",
            "not-a-url",
        ] {
            let invalid_url_spec = fixture_spec(&bytes, url);
            assert!(matches!(
                validate_geonames_asset_spec_source(&invalid_url_spec),
                Err(GeocoderError::InvalidAssetUrl { .. })
            ));
        }

        let short_target = tempdir.path().join("short.db");
        let short_spec = fixture_spec(&bytes, TEST_URL);
        let short_fetcher = BytesFetcher {
            bytes: b"short".to_vec(),
            calls: Cell::new(0),
        };
        assert!(matches!(
            ensure_geonames_asset_path_with_fetcher(&short_target, &short_spec, &short_fetcher),
            Err(GeocoderError::InvalidAssetLength { .. })
        ));

        let wrong_hash_target = tempdir.path().join("wrong-hash.db");
        let wrong_hash_spec = GeoNamesAssetSpec {
            sha256: "0000000000000000000000000000000000000000000000000000000000000000",
            ..fixture_spec(&bytes, TEST_URL)
        };
        let wrong_hash_fetcher = BytesFetcher {
            bytes: bytes.clone(),
            calls: Cell::new(0),
        };
        assert!(matches!(
            ensure_geonames_asset_path_with_fetcher(
                &wrong_hash_target,
                &wrong_hash_spec,
                &wrong_hash_fetcher,
            ),
            Err(GeocoderError::InvalidAssetSha256 { .. })
        ));
        fs::write(&wrong_hash_target, &bytes).expect("write wrong-hash fixture");
        assert!(matches!(
            validate_geonames_asset_file(&wrong_hash_target, &wrong_hash_spec),
            Err(GeocoderError::InvalidAssetSha256 { .. })
        ));

        let sqlite_target = tempdir.path().join("corrupt-sqlite.db");
        let sqlite_bytes = padded_corrupt_bytes(bytes.len());
        fs::write(&sqlite_target, &sqlite_bytes).expect("write corrupt sqlite");
        let sqlite_spec = fixture_spec_with_hash(&sqlite_bytes, TEST_URL);
        assert!(matches!(
            validate_geonames_asset_file(&sqlite_target, &sqlite_spec),
            Err(GeocoderError::InvalidAssetSqlite { .. })
                | Err(GeocoderError::InvalidAssetIntegrity { .. })
        ));

        let schema_target = tempdir.path().join("bad-schema.db");
        build_bad_schema_database(&schema_target);
        let schema_bytes = fs::read(&schema_target).expect("bad schema bytes");
        let schema_spec = fixture_spec_with_hash(&schema_bytes, TEST_URL);
        assert!(matches!(
            validate_geonames_asset_file(&schema_target, &schema_spec),
            Err(GeocoderError::InvalidAssetSchema { .. })
        ));
    }

    #[test]
    fn geonames_asset_lock_prevents_concurrent_install_writes() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let bytes = fixture_database_bytes();
        let spec = fixture_spec(&bytes, TEST_URL);
        let target = tempdir.path().join("geonames-test.db");
        fs::create_dir_all(target.parent().expect("target parent")).expect("target parent");
        fs::write(lock_path_for_asset(&target), b"locked").expect("lock file");
        let fetcher = BytesFetcher {
            bytes,
            calls: Cell::new(0),
        };

        assert!(matches!(
            ensure_geonames_asset_path_with_fetcher(&target, &spec, &fetcher),
            Err(GeocoderError::AssetLockUnavailable { .. })
        ));
        assert_eq!(fetcher.calls.get(), 0);
    }

    #[test]
    fn geonames_asset_invalid_asset_classifier_rejects_runtime_errors() {
        assert!(!is_invalid_asset_error(
            &GeocoderError::AssetLockUnavailable {
                path: PathBuf::from("geonames-test.db.lock"),
            },
        ));
    }

    fn fixture_database_bytes() -> Vec<u8> {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("fixture.db");
        build_fixture_database(&path);
        fs::read(path).expect("fixture database bytes")
    }

    fn fixture_spec(bytes: &[u8], url: &'static str) -> GeoNamesAssetSpec {
        fixture_spec_with_hash(bytes, url)
    }

    fn fixture_spec_with_hash(bytes: &[u8], url: &'static str) -> GeoNamesAssetSpec {
        let digest = sha2::Sha256::digest(bytes);
        let hash = Box::leak(hex::encode(digest).into_boxed_str());
        GeoNamesAssetSpec {
            version: "test",
            file_name: "geonames-test.db",
            url,
            allowed_host: GEONAMES_ASSET_HOST,
            byte_size: bytes.len() as u64,
            sha256: hash,
        }
    }

    fn padded_corrupt_bytes(len: usize) -> Vec<u8> {
        let mut bytes = vec![0_u8; len.max(32)];
        bytes[..16].copy_from_slice(b"not sqlite bytes");
        bytes
    }

    fn build_fixture_database(path: &Path) {
        let mut conn = open_test_path_connection(path);
        execute_batch(&mut conn, FIXTURE_SCHEMA);
        futures_executor::block_on(
            sqlx::query("INSERT INTO countries (id, name) VALUES (?, ?)")
                .bind("FX")
                .bind("Fixtureland")
                .execute(&mut conn),
        )
        .expect("insert country");
        futures_executor::block_on(
            sqlx::query("INSERT INTO admin1 (country_id, id, name) VALUES (?, ?, ?)")
                .bind("FX")
                .bind(1_i64)
                .bind("Fixture Region")
                .execute(&mut conn),
        )
        .expect("insert admin1");
        futures_executor::block_on(
            sqlx::query(
                "INSERT INTO features (id, name, country_id, admin1_id) VALUES (?, ?, ?, ?)",
            )
            .bind(1_i64)
            .bind("Fixture Town")
            .bind("FX")
            .bind(1_i64)
            .execute(&mut conn),
        )
        .expect("insert feature");
        futures_executor::block_on(
            sqlx::query(
                "INSERT INTO coordinates (feature_id, latitude, longitude) VALUES (?, ?, ?)",
            )
            .bind(1_i64)
            .bind(12.25_f64)
            .bind(-34.5_f64)
            .execute(&mut conn),
        )
        .expect("insert coordinates");
    }

    fn build_bad_schema_database(path: &Path) {
        let mut conn = open_test_path_connection(path);
        execute_batch(
            &mut conn,
            r#"
            CREATE TABLE countries(id TEXT, name TEXT);
            "#,
        );
    }

    fn open_test_path_connection(path: &Path) -> SqliteConnection {
        futures_executor::block_on(SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        ))
        .expect("open fixture database")
    }

    fn execute_batch(conn: &mut SqliteConnection, sql: &str) {
        futures_executor::block_on(sqlx::raw_sql(sqlx::AssertSqlSafe(sql)).execute(conn))
            .expect("execute fixture sql batch");
    }

    fn fetch_http_bytes(
        url: &str,
        maximum_bytes: u64,
        policy: GeoNamesHttpFetchPolicy,
    ) -> Result<Vec<u8>, GeocoderError> {
        let mut bytes = Vec::new();
        fetch_http_asset_to_writer_with_policy(url, maximum_bytes, &mut bytes, policy)?;
        Ok(bytes)
    }

    fn test_http_policy() -> GeoNamesHttpFetchPolicy {
        GeoNamesHttpFetchPolicy {
            connect_timeout: Duration::from_secs(1),
            response_timeout: Duration::from_secs(1),
            read_timeout: Duration::from_secs(1),
            total_timeout: Duration::from_secs(2),
            runtime_shutdown_timeout: Duration::from_millis(100),
        }
    }

    struct LoopbackHttpServer {
        url: String,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl LoopbackHttpServer {
        fn spawn(handler: impl FnOnce(TcpStream) + Send + 'static) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("loopback bind");
            let address = listener.local_addr().expect("loopback address");
            let thread = thread::spawn(move || {
                let (stream, _) = listener.accept().expect("loopback accept");
                handler(stream);
            });
            Self {
                url: format!("http://{address}/geonames.db"),
                thread: Some(thread),
            }
        }
    }

    impl Drop for LoopbackHttpServer {
        fn drop(&mut self) {
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    fn read_request(stream: &mut TcpStream) {
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("request read timeout");
        let mut request = Vec::with_capacity(2048);
        let mut buffer = [0_u8; 256];
        loop {
            let read = stream.read(&mut buffer).expect("request");
            assert_ne!(read, 0, "connection closed before complete request headers");
            request.extend_from_slice(&buffer[..read]);
            assert!(request.len() <= 32 * 1024, "request headers are bounded");
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                return;
            }
        }
    }

    fn assert_no_install_tempfiles(parent: &Path) {
        let entries = fs::read_dir(parent).expect("asset parent");
        for entry in entries {
            let file_name = entry.expect("asset parent entry").file_name();
            let file_name = file_name.to_string_lossy();
            assert!(
                !(file_name.starts_with(".geonames-test.db.") && file_name.ends_with(".tmp")),
                "temporary install file leaked: {file_name}"
            );
        }
    }

    const FIXTURE_SCHEMA: &str = r#"
        CREATE TABLE countries(id TEXT, name TEXT);
        CREATE TABLE admin1(country_id TEXT, id INTEGER, name TEXT);
        CREATE TABLE features(id INTEGER, name TEXT, country_id TEXT, admin1_id INTEGER);
        CREATE TABLE coordinates(feature_id INTEGER, latitude REAL, longitude REAL);
        CREATE VIEW geonames AS
            SELECT
                features.id AS id,
                features.name AS name,
                admin1.id AS admin1_id,
                admin1.name AS admin1_name,
                countries.id AS country_id,
                countries.name AS country_name,
                coordinates.latitude AS latitude,
                coordinates.longitude AS longitude
            FROM features
            JOIN countries ON features.country_id = countries.id
            JOIN admin1 ON features.country_id = admin1.country_id
                AND features.admin1_id = admin1.id
            JOIN coordinates ON features.id = coordinates.feature_id;
    "#;
}

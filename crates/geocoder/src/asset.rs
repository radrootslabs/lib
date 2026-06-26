use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use radroots_runtime_paths::default_shared_geonames_database_path_from_cache_root;
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use url::Url;

use crate::GeocoderError;

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
    fn fetch(&self, url: &str) -> Result<Vec<u8>, GeocoderError>;
}

#[derive(Clone, Debug, Default)]
pub struct GeoNamesBlockingHttpFetcher;

impl GeoNamesAssetFetcher for GeoNamesBlockingHttpFetcher {
    fn fetch(&self, url: &str) -> Result<Vec<u8>, GeocoderError> {
        let response =
            reqwest::blocking::get(url).map_err(|source| GeocoderError::AssetDownload {
                url: url.to_owned(),
                source,
            })?;
        let response =
            response
                .error_for_status()
                .map_err(|source| GeocoderError::AssetDownload {
                    url: url.to_owned(),
                    source,
                })?;
        response
            .bytes()
            .map(|bytes| bytes.to_vec())
            .map_err(|source| GeocoderError::AssetDownload {
                url: url.to_owned(),
                source,
            })
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
    let bytes = fetcher.fetch(spec.url)?;
    install_geonames_asset_bytes(path, spec, &bytes)?;
    let mut status = validate_geonames_asset_file(path, spec)?;
    status.state = GeoNamesAssetState::Refreshed;
    Ok(status)
}

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

fn install_geonames_asset_bytes(
    path: &Path,
    spec: &GeoNamesAssetSpec,
    bytes: &[u8],
) -> Result<(), GeocoderError> {
    if bytes.len() as u64 != spec.byte_size {
        return Err(GeocoderError::InvalidAssetLength {
            path: path.to_path_buf(),
            expected: spec.byte_size,
            actual: bytes.len() as u64,
        });
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let mut tempfile = tempfile::Builder::new()
        .prefix(&format!(".{}.", spec.file_name))
        .suffix(".tmp")
        .tempfile_in(parent)?;
    tempfile.as_file_mut().write_all(bytes)?;
    tempfile.as_file_mut().sync_all()?;
    validate_geonames_asset_file(tempfile.path(), spec)?;
    tempfile
        .persist(path)
        .map(|_| ())
        .map_err(|error| GeocoderError::Io(error.error))
}

fn validate_sqlite_integrity_and_schema(path: &Path) -> Result<(), GeocoderError> {
    let conn =
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|error| {
            GeocoderError::InvalidAssetSqlite {
                path: path.to_path_buf(),
                detail: error.to_string(),
            }
        })?;
    validate_sqlite_integrity(path, &conn)?;
    for query in [
        "SELECT id, name FROM countries LIMIT 1",
        "SELECT country_id, id, name FROM admin1 LIMIT 1",
        "SELECT id, name, country_id, admin1_id FROM features LIMIT 1",
        "SELECT feature_id, latitude, longitude FROM coordinates LIMIT 1",
        "SELECT id, name, admin1_id, admin1_name, country_id, country_name, latitude, longitude FROM geonames LIMIT 1",
    ] {
        conn.prepare(query)
            .map(|_| ())
            .map_err(|error| GeocoderError::InvalidAssetSchema {
                path: path.to_path_buf(),
                detail: error.to_string(),
            })?;
    }
    Ok(())
}

fn validate_sqlite_integrity(path: &Path, conn: &Connection) -> Result<(), GeocoderError> {
    let mut stmt = conn.prepare("PRAGMA integrity_check").map_err(|error| {
        GeocoderError::InvalidAssetSqlite {
            path: path.to_path_buf(),
            detail: error.to_string(),
        }
    })?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| GeocoderError::InvalidAssetSqlite {
            path: path.to_path_buf(),
            detail: error.to_string(),
        })?;
    let results =
        rows.collect::<Result<Vec<_>, _>>()
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
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::path::{Path, PathBuf};

    use rusqlite::Connection;
    use sha2::Digest;

    use super::{
        GEONAMES_ASSET_HOST, GeoNamesAssetFetcher, GeoNamesAssetSpec, GeoNamesAssetState,
        ensure_geonames_asset_path_with_fetcher, inspect_geonames_asset_path, lock_path_for_asset,
        validate_geonames_asset_file, validate_geonames_asset_spec_source,
    };
    use crate::GeocoderError;

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
        let conn = Connection::open(path).expect("open fixture db");
        conn.execute_batch(FIXTURE_SCHEMA).expect("fixture schema");
        conn.execute(
            "INSERT INTO countries (id, name) VALUES (?1, ?2)",
            ("FX", "Fixtureland"),
        )
        .expect("insert country");
        conn.execute(
            "INSERT INTO admin1 (country_id, id, name) VALUES (?1, ?2, ?3)",
            ("FX", 1_i64, "Fixture Region"),
        )
        .expect("insert admin1");
        conn.execute(
            "INSERT INTO features (id, name, country_id, admin1_id) VALUES (?1, ?2, ?3, ?4)",
            (1_i64, "Fixture Town", "FX", 1_i64),
        )
        .expect("insert feature");
        conn.execute(
            "INSERT INTO coordinates (feature_id, latitude, longitude) VALUES (?1, ?2, ?3)",
            (1_i64, 12.25_f64, -34.5_f64),
        )
        .expect("insert coordinates");
    }

    fn build_bad_schema_database(path: &PathBuf) {
        let conn = Connection::open(path).expect("open bad schema db");
        conn.execute_batch(
            r#"
            CREATE TABLE countries(id TEXT, name TEXT);
            "#,
        )
        .expect("bad schema");
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

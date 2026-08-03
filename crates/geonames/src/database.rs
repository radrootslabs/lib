//! Explicit GeoNames database lifecycle.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

use crate::asset::verify_file;
use crate::{AssetSpec, Error};

const REQUIRED_GEONAMES_COLUMNS: &[&str] = &[
    "id",
    "name",
    "admin1_id",
    "admin1_name",
    "country_id",
    "country_name",
    "latitude",
    "longitude",
];
const REQUIRED_COORDINATE_COLUMNS: &[&str] = &["feature_id", "latitude", "longitude"];

/// An opened, verified GeoNames database.
///
/// The connection is read-only and serialized by this type. It owns no path
/// policy, migration authority, runtime, download, or background worker.
#[derive(Debug)]
pub struct Geocoder {
    connection: Mutex<Connection>,
}

impl Geocoder {
    /// Opens an explicitly selected asset after complete identity and schema checks.
    pub fn open(path: impl AsRef<Path>, spec: &AssetSpec) -> Result<Self, Error> {
        let path = path.as_ref();
        let metadata = path
            .symlink_metadata()
            .map_err(|error| crate::asset::io_error("inspect database asset", error))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(Error::UnsafeAssetDestination);
        }
        verify_file(path, spec)?;

        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|_| Error::InvalidDatabase)?;
        configure_connection(&connection)?;
        validate_integrity(&connection)?;
        validate_schema(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Closes the database and reports a terminal SQLite close failure.
    pub fn close(self) -> Result<(), Error> {
        let connection = self
            .connection
            .into_inner()
            .map_err(|_| Error::DatabaseConnectionUnavailable)?;
        connection
            .close()
            .map_err(|_| Error::DatabaseOperationFailed { operation: "close" })
    }
}

fn configure_connection(connection: &Connection) -> Result<(), Error> {
    connection
        .busy_timeout(Duration::from_secs(5))
        .and_then(|()| connection.pragma_update(None, "query_only", true))
        .and_then(|()| connection.pragma_update(None, "trusted_schema", false))
        .map_err(|_| Error::InvalidDatabase)
}

fn validate_integrity(connection: &Connection) -> Result<(), Error> {
    let result = connection
        .query_row("PRAGMA quick_check(1)", [], |row| row.get::<_, String>(0))
        .map_err(|_| Error::InvalidDatabase)?;
    if result != "ok" {
        return Err(Error::InvalidDatabase);
    }
    Ok(())
}

fn validate_schema(connection: &Connection) -> Result<(), Error> {
    validate_table(
        connection,
        "geonames",
        REQUIRED_GEONAMES_COLUMNS,
        "PRAGMA table_info('geonames')",
    )?;
    validate_table(
        connection,
        "coordinates",
        REQUIRED_COORDINATE_COLUMNS,
        "PRAGMA table_info('coordinates')",
    )
}

fn validate_table(
    connection: &Connection,
    table: &str,
    required_columns: &[&str],
    column_pragma: &str,
) -> Result<(), Error> {
    let object_type = connection
        .query_row(
            "SELECT type FROM sqlite_schema WHERE name = ?1 AND type = 'table'",
            [table],
            |row| row.get::<_, String>(0),
        )
        .map_err(|_| Error::InvalidDatabaseSchema)?;
    if object_type != "table" {
        return Err(Error::InvalidDatabaseSchema);
    }

    let mut statement = connection
        .prepare(column_pragma)
        .map_err(|_| Error::InvalidDatabaseSchema)?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|_| Error::InvalidDatabaseSchema)?
        .collect::<Result<BTreeSet<_>, _>>()
        .map_err(|_| Error::InvalidDatabaseSchema)?;
    if required_columns
        .iter()
        .any(|column| !columns.contains(*column))
    {
        return Err(Error::InvalidDatabaseSchema);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rusqlite::Connection;
    use sha2::{Digest, Sha256};
    use tempfile::{TempDir, tempdir};

    use super::Geocoder;
    use crate::{AssetSpec, Error};

    fn database_fixture(schema: &str) -> (TempDir, std::path::PathBuf, AssetSpec) {
        let directory = tempdir().expect("tempdir");
        let path = directory.path().join("geonames-test.db");
        let connection = Connection::open(&path).expect("create fixture database");
        connection
            .execute_batch(schema)
            .expect("install fixture schema");
        connection.close().expect("close fixture writer");
        let bytes = fs::read(&path).expect("read fixture");
        let spec = AssetSpec::new(
            "test-v1",
            "geonames-test.db",
            "https://assets.example/geonames-test.db",
            "assets.example",
            u64::try_from(bytes.len()).expect("fixture length"),
            Sha256::digest(&bytes).into(),
        )
        .expect("fixture spec");
        (directory, path, spec)
    }

    fn governed_schema() -> &'static str {
        "
        CREATE TABLE geonames (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            admin1_id,
            admin1_name TEXT,
            country_id TEXT NOT NULL,
            country_name TEXT,
            latitude REAL NOT NULL,
            longitude REAL NOT NULL
        );
        CREATE TABLE coordinates (
            feature_id INTEGER PRIMARY KEY,
            latitude REAL NOT NULL,
            longitude REAL NOT NULL
        );
        INSERT INTO geonames VALUES
            (6174041, 'Victoria', 'BC', 'British Columbia', 'CA', 'Canada', 48.4284, -123.3656);
        INSERT INTO coordinates VALUES (6174041, 48.4284, -123.3656);
        "
    }

    #[test]
    fn verified_governed_database_opens_read_only_and_closes_explicitly() {
        let (_directory, path, spec) = database_fixture(governed_schema());
        let geocoder = Geocoder::open(&path, &spec).expect("open verified database");
        let connection = geocoder.connection.lock().expect("connection lock");
        let count = connection
            .query_row("SELECT COUNT(*) FROM geonames", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("query fixture");
        assert_eq!(count, 1);
        assert!(matches!(
            connection.execute("DELETE FROM geonames", []),
            Err(rusqlite::Error::SqliteFailure(_, _))
        ));
        drop(connection);
        geocoder.close().expect("explicit close");
    }

    #[test]
    fn corrupt_bytes_and_incomplete_schema_fail_closed() {
        let directory = tempdir().expect("tempdir");
        let path = directory.path().join("geonames-test.db");
        fs::write(&path, b"not sqlite").expect("write corrupt fixture");
        let corrupt_spec = AssetSpec::new(
            "test-v1",
            "geonames-test.db",
            "https://assets.example/geonames-test.db",
            "assets.example",
            10,
            Sha256::digest(b"not sqlite").into(),
        )
        .expect("corrupt spec");
        assert!(matches!(
            Geocoder::open(&path, &corrupt_spec),
            Err(Error::InvalidDatabase)
        ));

        let (_directory, path, spec) = database_fixture("CREATE TABLE geonames (id INTEGER);");
        assert!(matches!(
            Geocoder::open(path, &spec),
            Err(Error::InvalidDatabaseSchema)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn verified_database_open_rejects_symlink_assets() {
        use std::os::unix::fs::symlink;

        let (directory, path, spec) = database_fixture(governed_schema());
        let link = directory.path().join("linked.db");
        symlink(path, &link).expect("asset symlink");
        assert!(matches!(
            Geocoder::open(link, &spec),
            Err(Error::UnsafeAssetDestination)
        ));
    }
}

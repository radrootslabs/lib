//! Explicit GeoNames database lifecycle.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, Row, params};

use crate::asset::verify_file;
use crate::model::Country;
use crate::query::{QueryKind, QueryResult};
use crate::{AssetSpec, Candidate, Error, Point, Query};

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

    /// Executes one validated query with deterministic provider ordering.
    pub fn query(&self, query: &Query) -> Result<QueryResult, Error> {
        self.with_connection("query", |connection| match &query.kind {
            QueryKind::Locality {
                locality,
                region,
                country,
            } => query_locality(
                connection,
                locality,
                region.as_deref(),
                country.as_deref(),
                query.limit(),
            ),
            QueryKind::Freeform(query_text) => {
                let parsed = parse_freeform_query(query_text);
                query_locality(
                    connection,
                    &parsed.locality,
                    parsed.region.as_deref(),
                    parsed.country.as_deref(),
                    query.limit(),
                )
            }
            QueryKind::FeatureId(feature_id) => query_feature(connection, *feature_id),
            QueryKind::Reverse {
                point,
                radius_degrees,
            } => query_reverse(connection, *point, *radius_degrees, query.limit()),
            QueryKind::Countries => query_countries(connection, query.limit()),
        })
    }

    fn with_connection<T>(
        &self,
        operation: &'static str,
        use_connection: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> Result<T, Error> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| Error::DatabaseConnectionUnavailable)?;
        use_connection(&connection).map_err(|_| Error::DatabaseOperationFailed { operation })
    }
}

fn query_locality(
    connection: &Connection,
    locality: &str,
    region: Option<&str>,
    country: Option<&str>,
    limit: usize,
) -> rusqlite::Result<QueryResult> {
    let locality = normalize_name(locality);
    let country = country.map(normalize_name);
    let region = region.map(normalize_name);
    let mut statement = connection.prepare(
        "
        SELECT id, name, CAST(admin1_id AS TEXT), admin1_name,
               country_id, country_name, latitude, longitude
        FROM geonames
        WHERE lower(name) = ?1
        ORDER BY lower(name), lower(country_id),
                 lower(coalesce(country_name, '')),
                 lower(coalesce(admin1_name, '')),
                 CASE
                   WHEN admin1_id IS NULL THEN 0
                   WHEN typeof(admin1_id) IN ('integer', 'real') THEN 1
                   WHEN typeof(admin1_id) = 'text' THEN 2
                   ELSE 3
                 END,
                 CASE WHEN typeof(admin1_id) IN ('integer', 'real')
                      THEN admin1_id ELSE NULL END,
                 CASE WHEN typeof(admin1_id) = 'text'
                      THEN CAST(admin1_id AS TEXT) ELSE NULL END COLLATE BINARY,
                 id
        ",
    )?;
    let candidates = statement
        .query_map([locality], map_candidate)?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|candidate| {
            country
                .as_deref()
                .is_none_or(|value| country_matches(candidate, value))
                && region
                    .as_deref()
                    .is_none_or(|value| region_matches(candidate, value))
        })
        .take(limit)
        .collect();
    Ok(QueryResult::candidates(candidates))
}

fn query_feature(connection: &Connection, feature_id: i64) -> rusqlite::Result<QueryResult> {
    let mut statement = connection.prepare(
        "
        SELECT id, name, CAST(admin1_id AS TEXT), admin1_name,
               country_id, country_name, latitude, longitude
        FROM geonames
        WHERE id = ?1
        LIMIT 1
        ",
    )?;
    let candidates = statement
        .query_map([feature_id], map_candidate)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(QueryResult::candidates(candidates))
}

fn query_reverse(
    connection: &Connection,
    point: Point,
    radius_degrees: f64,
    limit: usize,
) -> rusqlite::Result<QueryResult> {
    let latitude = point.latitude();
    let longitude = point.longitude();
    let longitude_weight = latitude.to_radians().cos().powi(2);
    let mut statement = connection.prepare(
        "
        SELECT g.id, g.name, CAST(g.admin1_id AS TEXT), g.admin1_name,
               g.country_id, g.country_name, g.latitude, g.longitude
        FROM geonames AS g
        JOIN coordinates AS c ON g.id = c.feature_id
        WHERE c.latitude BETWEEN ?1 - ?3 AND ?1 + ?3
          AND (
            abs(?1) + ?3 >= 90.0
            OR (
              ?2 - ?3 >= -180.0 AND ?2 + ?3 <= 180.0
              AND c.longitude BETWEEN ?2 - ?3 AND ?2 + ?3
            )
            OR (
              ?2 - ?3 < -180.0
              AND (c.longitude >= ?2 - ?3 + 360.0 OR c.longitude <= ?2 + ?3)
            )
            OR (
              ?2 + ?3 > 180.0
              AND (c.longitude >= ?2 - ?3 OR c.longitude <= ?2 + ?3 - 360.0)
            )
          )
        ORDER BY ((?1 - c.latitude) * (?1 - c.latitude))
               + (min(abs(?2 - c.longitude), 360.0 - abs(?2 - c.longitude))
                  * min(abs(?2 - c.longitude), 360.0 - abs(?2 - c.longitude))
                  * ?4),
                 g.id
        LIMIT ?5
        ",
    )?;
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    let candidates = statement
        .query_map(
            params![latitude, longitude, radius_degrees, longitude_weight, limit],
            map_candidate,
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(QueryResult::candidates(candidates))
}

fn query_countries(connection: &Connection, limit: usize) -> rusqlite::Result<QueryResult> {
    let mut statement = connection.prepare(
        "
        SELECT country_id, country_name, AVG(latitude), AVG(longitude)
        FROM geonames
        GROUP BY country_id, country_name
        ORDER BY lower(country_id), lower(coalesce(country_name, ''))
        LIMIT ?1
        ",
    )?;
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    let countries = statement
        .query_map([limit], map_country)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(QueryResult::countries(countries))
}

fn map_candidate(row: &Row<'_>) -> rusqlite::Result<Candidate> {
    let feature_id = row.get::<_, i64>(0)?;
    let feature_id = u64::try_from(feature_id)
        .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, feature_id))?;
    let latitude = row.get::<_, f64>(6)?;
    let longitude = row.get::<_, f64>(7)?;
    let point = Point::new(latitude, longitude).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Real, Box::new(error))
    })?;
    Ok(Candidate::from_provider_row(
        feature_id,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        point,
    ))
}

fn map_country(row: &Row<'_>) -> rusqlite::Result<Country> {
    let latitude = row.get::<_, f64>(2)?;
    let longitude = row.get::<_, f64>(3)?;
    let point = Point::new(latitude, longitude).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Real, Box::new(error))
    })?;
    Ok(Country::from_provider_row(row.get(0)?, row.get(1)?, point))
}

struct ParsedQuery {
    locality: String,
    region: Option<String>,
    country: Option<String>,
}

fn parse_freeform_query(query: &str) -> ParsedQuery {
    let parts = query
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    match parts.as_slice() {
        [] => ParsedQuery {
            locality: String::new(),
            region: None,
            country: None,
        },
        [locality] => ParsedQuery {
            locality: (*locality).to_owned(),
            region: None,
            country: None,
        },
        [locality, region] => ParsedQuery {
            locality: (*locality).to_owned(),
            region: Some((*region).to_owned()),
            country: None,
        },
        parts => ParsedQuery {
            locality: parts[..parts.len() - 2].join(", "),
            region: Some(parts[parts.len() - 2].to_owned()),
            country: Some(parts[parts.len() - 1].to_owned()),
        },
    }
}

fn normalize_name(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn normalize_region_code(value: &str) -> String {
    value
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_uppercase())
        .collect()
}

fn country_matches(candidate: &Candidate, expected: &str) -> bool {
    normalize_name(candidate.country_id()) == expected
        || candidate
            .country_name()
            .is_some_and(|name| normalize_name(name) == expected)
}

fn region_matches(candidate: &Candidate, expected: &str) -> bool {
    if candidate
        .admin1_id()
        .is_some_and(|id| normalize_name(id) == expected)
        || candidate
            .admin1_name()
            .is_some_and(|name| normalize_name(name) == expected)
    {
        return true;
    }
    let expected_code = normalize_region_code(expected);
    region_aliases(candidate.country_id())
        .iter()
        .any(|(code, name)| {
            normalize_region_code(code) == expected_code
                && candidate
                    .admin1_name()
                    .is_some_and(|admin_name| normalize_name(admin_name) == normalize_name(name))
        })
}

fn region_aliases(country_id: &str) -> &'static [(&'static str, &'static str)] {
    match country_id.to_ascii_uppercase().as_str() {
        "CA" => &[
            ("AB", "Alberta"),
            ("BC", "British Columbia"),
            ("MB", "Manitoba"),
            ("NB", "New Brunswick"),
            ("NL", "Newfoundland and Labrador"),
            ("NS", "Nova Scotia"),
            ("NT", "Northwest Territories"),
            ("NU", "Nunavut"),
            ("ON", "Ontario"),
            ("PE", "Prince Edward Island"),
            ("QC", "Quebec"),
            ("SK", "Saskatchewan"),
            ("YT", "Yukon"),
        ],
        "US" => &[
            ("AL", "Alabama"),
            ("AK", "Alaska"),
            ("AZ", "Arizona"),
            ("AR", "Arkansas"),
            ("CA", "California"),
            ("CO", "Colorado"),
            ("CT", "Connecticut"),
            ("DC", "District of Columbia"),
            ("DE", "Delaware"),
            ("FL", "Florida"),
            ("GA", "Georgia"),
            ("HI", "Hawaii"),
            ("ID", "Idaho"),
            ("IL", "Illinois"),
            ("IN", "Indiana"),
            ("IA", "Iowa"),
            ("KS", "Kansas"),
            ("KY", "Kentucky"),
            ("LA", "Louisiana"),
            ("ME", "Maine"),
            ("MD", "Maryland"),
            ("MA", "Massachusetts"),
            ("MI", "Michigan"),
            ("MN", "Minnesota"),
            ("MS", "Mississippi"),
            ("MO", "Missouri"),
            ("MT", "Montana"),
            ("NE", "Nebraska"),
            ("NV", "Nevada"),
            ("NH", "New Hampshire"),
            ("NJ", "New Jersey"),
            ("NM", "New Mexico"),
            ("NY", "New York"),
            ("NC", "North Carolina"),
            ("ND", "North Dakota"),
            ("OH", "Ohio"),
            ("OK", "Oklahoma"),
            ("OR", "Oregon"),
            ("PA", "Pennsylvania"),
            ("RI", "Rhode Island"),
            ("SC", "South Carolina"),
            ("SD", "South Dakota"),
            ("TN", "Tennessee"),
            ("TX", "Texas"),
            ("UT", "Utah"),
            ("VT", "Vermont"),
            ("VA", "Virginia"),
            ("WA", "Washington"),
            ("WV", "West Virginia"),
            ("WI", "Wisconsin"),
            ("WY", "Wyoming"),
        ],
        _ => &[],
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
            (6174041, 'Victoria', 2, 'British Columbia', 'CA', 'Canada', 48.4284, -123.3656),
            (5815135, 'Victoria', 'WA', 'Washington', 'US', 'United States', 48.1000, -122.8000),
            (10, 'Twin A', 'BC', 'British Columbia', 'CA', 'Canada', 49.0000, -124.0000),
            (11, 'Twin B', 'BC', 'British Columbia', 'CA', 'Canada', 49.0000, -124.0000),
            (20, 'Date East', NULL, NULL, 'FJ', 'Fiji', 0.0000, 179.9000),
            (21, 'Date West', NULL, NULL, 'FJ', 'Fiji', 0.0000, -179.9000),
            (30, 'Pole Prime', NULL, NULL, 'AQ', 'Antarctica', 89.9000, 0.0000),
            (31, 'Pole East', NULL, NULL, 'AQ', 'Antarctica', 89.9000, 120.0000);
        INSERT INTO coordinates VALUES
            (6174041, 48.4284, -123.3656),
            (5815135, 48.1000, -122.8000),
            (10, 49.0000, -124.0000),
            (11, 49.0000, -124.0000),
            (20, 0.0000, 179.9000),
            (21, 0.0000, -179.9000),
            (30, 89.9000, 0.0000),
            (31, 89.9000, 120.0000);
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
        assert_eq!(count, 8);
        assert!(matches!(
            connection.execute("DELETE FROM geonames", []),
            Err(rusqlite::Error::SqliteFailure(_, _))
        ));
        drop(connection);
        geocoder.close().expect("explicit close");
    }

    #[test]
    fn forward_and_feature_queries_preserve_text_ids_and_stable_order() {
        let (_directory, path, spec) = database_fixture(governed_schema());
        let geocoder = Geocoder::open(path, &spec).expect("geocoder");

        let structured = crate::Query::locality("Victoria")
            .expect("locality")
            .with_region("BC")
            .expect("region")
            .with_country("Canada")
            .expect("country");
        let result = geocoder.query(&structured).expect("structured query");
        let candidates = result.as_candidates().expect("candidate result");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].feature_id(), 6_174_041);
        assert_eq!(candidates[0].admin1_id(), Some("2"));
        assert_eq!(
            candidates[0].display_name(),
            "Victoria, British Columbia, Canada"
        );

        let freeform = crate::Query::freeform("Victoria, BC, CA").expect("freeform");
        assert_eq!(
            geocoder
                .query(&freeform)
                .expect("freeform query")
                .as_candidates()
                .expect("candidates")[0]
                .feature_id(),
            6_174_041
        );

        let ambiguous = crate::Query::locality("Victoria")
            .expect("locality")
            .with_limit(2)
            .expect("limit");
        let candidates = geocoder
            .query(&ambiguous)
            .expect("ambiguous query")
            .as_candidates()
            .expect("candidates")
            .to_vec();
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.country_id())
                .collect::<Vec<_>>(),
            vec!["CA", "US"]
        );

        let feature = crate::Query::feature_id(5_815_135).expect("feature query");
        assert_eq!(
            geocoder
                .query(&feature)
                .expect("feature result")
                .as_candidates()
                .expect("candidates")[0]
                .admin1_id(),
            Some("WA")
        );
    }

    #[test]
    fn reverse_and_country_queries_are_bounded_and_deterministic() {
        let (_directory, path, spec) = database_fixture(governed_schema());
        let geocoder = Geocoder::open(path, &spec).expect("geocoder");
        let reverse = crate::Query::reverse(crate::Point::new(49.0, -124.0).expect("point"))
            .with_radius_degrees(0.1)
            .expect("radius")
            .with_limit(2)
            .expect("limit");
        let candidates = geocoder
            .query(&reverse)
            .expect("reverse result")
            .as_candidates()
            .expect("candidates")
            .to_vec();
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.feature_id())
                .collect::<Vec<_>>(),
            vec![10, 11]
        );

        let countries = geocoder
            .query(&crate::Query::countries())
            .expect("country result");
        let countries = countries.as_countries().expect("countries");
        assert_eq!(
            countries
                .iter()
                .map(|country| country.id())
                .collect::<Vec<_>>(),
            vec!["AQ", "CA", "FJ", "US"]
        );
        assert_eq!(countries[1].name(), Some("Canada"));
        assert!(countries[0].center().latitude().is_finite());

        let dateline = crate::Query::reverse(crate::Point::new(0.0, 180.0).expect("point"))
            .with_radius_degrees(0.2)
            .expect("radius")
            .with_limit(2)
            .expect("limit");
        assert_eq!(
            geocoder
                .query(&dateline)
                .expect("dateline result")
                .as_candidates()
                .expect("candidates")
                .iter()
                .map(|candidate| candidate.feature_id())
                .collect::<Vec<_>>(),
            vec![20, 21]
        );

        let pole = crate::Query::reverse(crate::Point::new(90.0, 0.0).expect("point"))
            .with_radius_degrees(0.2)
            .expect("radius")
            .with_limit(2)
            .expect("limit");
        assert_eq!(
            geocoder
                .query(&pole)
                .expect("pole result")
                .as_candidates()
                .expect("candidates")
                .iter()
                .map(|candidate| candidate.feature_id())
                .collect::<Vec<_>>(),
            vec![30, 31]
        );
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

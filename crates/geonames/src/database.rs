//! Explicit GeoNames database lifecycle.

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;
use std::time::Duration;

use futures::TryStreamExt;
use sqlx::{
    ConnectOptions, Connection as _, Row, SqliteConnection,
    sqlite::{SqliteConnectOptions, SqliteRow},
};
use tokio::sync::Mutex;

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
/// policy, migration authority, runtime, download, or background task.
pub struct Geocoder {
    connection: Mutex<SqliteConnection>,
}

impl fmt::Debug for Geocoder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Geocoder").finish_non_exhaustive()
    }
}

impl Geocoder {
    /// Opens an explicitly selected asset after complete identity and schema checks.
    pub async fn open(path: impl AsRef<Path>, spec: &AssetSpec) -> Result<Self, Error> {
        let path = path.as_ref();
        let metadata = path
            .symlink_metadata()
            .map_err(|error| crate::asset::io_error("inspect database asset", error))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(Error::UnsafeAssetDestination);
        }
        verify_file(path, spec)?;

        let options = SqliteConnectOptions::new()
            .filename(path)
            .read_only(true)
            .create_if_missing(false)
            .immutable(true)
            .busy_timeout(Duration::from_secs(5))
            .disable_statement_logging();
        let mut connection = SqliteConnection::connect_with(&options)
            .await
            .map_err(|_| Error::InvalidDatabase)?;
        let validation = async {
            configure_connection(&mut connection).await?;
            validate_integrity(&mut connection).await?;
            validate_schema(&mut connection).await
        }
        .await;
        if let Err(error) = validation {
            let _ = connection.close().await;
            return Err(error);
        }
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Closes the database and reports a terminal SQLite close failure.
    pub async fn close(self) -> Result<(), Error> {
        let connection = self.connection.into_inner();
        connection
            .close()
            .await
            .map_err(|_| Error::DatabaseOperationFailed { operation: "close" })
    }

    /// Executes one validated query with deterministic provider ordering.
    pub async fn query(&self, query: &Query) -> Result<QueryResult, Error> {
        let mut connection = self.connection.lock().await;
        match &query.kind {
            QueryKind::Locality {
                locality,
                region,
                country,
            } => {
                query_locality(
                    &mut connection,
                    locality,
                    region.as_deref(),
                    country.as_deref(),
                    query.limit(),
                )
                .await
            }
            QueryKind::Freeform(query_text) => {
                let parsed = parse_freeform_query(query_text);
                query_locality(
                    &mut connection,
                    &parsed.locality,
                    parsed.region.as_deref(),
                    parsed.country.as_deref(),
                    query.limit(),
                )
                .await
            }
            QueryKind::FeatureId(feature_id) => query_feature(&mut connection, *feature_id).await,
            QueryKind::Reverse {
                point,
                radius_degrees,
            } => query_reverse(&mut connection, *point, *radius_degrees, query.limit()).await,
            QueryKind::Countries => query_countries(&mut connection, query.limit()).await,
        }
    }
}

async fn query_locality(
    connection: &mut SqliteConnection,
    locality: &str,
    region: Option<&str>,
    country: Option<&str>,
    limit: usize,
) -> Result<QueryResult, Error> {
    let locality = normalize_name(locality);
    let country = country.map(normalize_name);
    let region = region.map(normalize_name);
    let mut rows = sqlx::query(
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
    )
    .bind(locality)
    .fetch(&mut *connection);
    let mut candidates = Vec::with_capacity(limit);
    while let Some(row) = rows.try_next().await.map_err(query_failed)? {
        let candidate = map_candidate(&row)?;
        if country
            .as_deref()
            .is_none_or(|value| country_matches(&candidate, value))
            && region
                .as_deref()
                .is_none_or(|value| region_matches(&candidate, value))
        {
            candidates.push(candidate);
            if candidates.len() == limit {
                break;
            }
        }
    }
    Ok(QueryResult::candidates(candidates))
}

async fn query_feature(
    connection: &mut SqliteConnection,
    feature_id: i64,
) -> Result<QueryResult, Error> {
    let row = sqlx::query(
        "
        SELECT id, name, CAST(admin1_id AS TEXT), admin1_name,
               country_id, country_name, latitude, longitude
        FROM geonames
        WHERE id = ?1
        LIMIT 1
        ",
    )
    .bind(feature_id)
    .fetch_optional(connection)
    .await
    .map_err(query_failed)?;
    let candidates = row
        .as_ref()
        .map(map_candidate)
        .transpose()?
        .into_iter()
        .collect();
    Ok(QueryResult::candidates(candidates))
}

async fn query_reverse(
    connection: &mut SqliteConnection,
    point: Point,
    radius_degrees: f64,
    limit: usize,
) -> Result<QueryResult, Error> {
    let latitude = point.latitude();
    let longitude = point.longitude();
    let longitude_weight = latitude.to_radians().cos().powi(2);
    let rows = sqlx::query(
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
    )
    .bind(latitude)
    .bind(longitude)
    .bind(radius_degrees)
    .bind(longitude_weight)
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await
    .map_err(query_failed)?;
    let candidates = rows
        .iter()
        .map(map_candidate)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(QueryResult::candidates(candidates))
}

async fn query_countries(
    connection: &mut SqliteConnection,
    limit: usize,
) -> Result<QueryResult, Error> {
    let rows = sqlx::query(
        "
        SELECT country_id, country_name, AVG(latitude), AVG(longitude)
        FROM geonames
        GROUP BY country_id, country_name
        ORDER BY lower(country_id), lower(coalesce(country_name, ''))
        LIMIT ?1
        ",
    )
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await
    .map_err(query_failed)?;
    let countries = rows
        .iter()
        .map(map_country)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(QueryResult::countries(countries))
}

fn map_candidate(row: &SqliteRow) -> Result<Candidate, Error> {
    let feature_id = row.try_get::<i64, _>(0).map_err(query_failed)?;
    let feature_id = u64::try_from(feature_id).map_err(|_| query_failed(()))?;
    let latitude = row.try_get::<f64, _>(6).map_err(query_failed)?;
    let longitude = row.try_get::<f64, _>(7).map_err(query_failed)?;
    let point = Point::new(latitude, longitude).map_err(|_| query_failed(()))?;
    Ok(Candidate::from_provider_row(
        feature_id,
        row.try_get(1).map_err(query_failed)?,
        row.try_get(2).map_err(query_failed)?,
        row.try_get(3).map_err(query_failed)?,
        row.try_get(4).map_err(query_failed)?,
        row.try_get(5).map_err(query_failed)?,
        point,
    ))
}

fn map_country(row: &SqliteRow) -> Result<Country, Error> {
    let latitude = row.try_get::<f64, _>(2).map_err(query_failed)?;
    let longitude = row.try_get::<f64, _>(3).map_err(query_failed)?;
    let point = Point::new(latitude, longitude).map_err(|_| query_failed(()))?;
    Ok(Country::from_provider_row(
        row.try_get(0).map_err(query_failed)?,
        row.try_get(1).map_err(query_failed)?,
        point,
    ))
}

fn query_failed<T>(_source: T) -> Error {
    Error::DatabaseOperationFailed { operation: "query" }
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

async fn configure_connection(connection: &mut SqliteConnection) -> Result<(), Error> {
    sqlx::query("PRAGMA query_only = ON")
        .execute(&mut *connection)
        .await
        .map_err(|_| Error::InvalidDatabase)?;
    sqlx::query("PRAGMA trusted_schema = OFF")
        .execute(&mut *connection)
        .await
        .map_err(|_| Error::InvalidDatabase)?;
    let query_only = sqlx::query_scalar::<_, i64>("PRAGMA query_only")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| Error::InvalidDatabase)?;
    let trusted_schema = sqlx::query_scalar::<_, i64>("PRAGMA trusted_schema")
        .fetch_one(connection)
        .await
        .map_err(|_| Error::InvalidDatabase)?;
    if query_only != 1 || trusted_schema != 0 {
        return Err(Error::InvalidDatabase);
    }
    Ok(())
}

async fn validate_integrity(connection: &mut SqliteConnection) -> Result<(), Error> {
    let rows = sqlx::query_scalar::<_, String>("PRAGMA quick_check(1)")
        .fetch_all(connection)
        .await
        .map_err(|_| Error::InvalidDatabase)?;
    if rows.len() != 1 || rows[0] != "ok" {
        return Err(Error::InvalidDatabase);
    }
    Ok(())
}

async fn validate_schema(connection: &mut SqliteConnection) -> Result<(), Error> {
    validate_table(connection, "geonames", REQUIRED_GEONAMES_COLUMNS).await?;
    validate_table(connection, "coordinates", REQUIRED_COORDINATE_COLUMNS).await
}

async fn validate_table(
    connection: &mut SqliteConnection,
    table: &str,
    required_columns: &[&str],
) -> Result<(), Error> {
    let table_exists = sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM sqlite_schema WHERE name = ?1 AND type = 'table' LIMIT 1",
    )
    .bind(table)
    .fetch_optional(&mut *connection)
    .await
    .map_err(|_| Error::InvalidDatabaseSchema)?;
    if table_exists != Some(1) {
        return Err(Error::InvalidDatabaseSchema);
    }

    let columns = sqlx::query_scalar::<_, String>("SELECT name FROM pragma_table_info(?1)")
        .bind(table)
        .fetch_all(connection)
        .await
        .map_err(|_| Error::InvalidDatabaseSchema)?;
    let columns = columns.into_iter().collect::<BTreeSet<_>>();
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

    use sha2::{Digest, Sha256};
    use sqlx::{ConnectOptions, Connection as _, SqliteConnection, sqlite::SqliteConnectOptions};
    use tempfile::{TempDir, tempdir};

    use super::{
        Geocoder, country_matches, normalize_name, normalize_region_code, parse_freeform_query,
        region_aliases, region_matches,
    };
    use crate::{AssetSpec, Candidate, Error, Point};

    async fn database_fixture(schema: &str) -> (TempDir, std::path::PathBuf, AssetSpec) {
        let directory = tempdir().expect("tempdir");
        let path = directory.path().join("geonames-test.db");
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .disable_statement_logging();
        let mut connection = SqliteConnection::connect_with(&options)
            .await
            .expect("create fixture database");
        sqlx::raw_sql(sqlx::AssertSqlSafe(schema))
            .execute(&mut connection)
            .await
            .expect("install fixture schema");
        connection.close().await.expect("close fixture writer");
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

    #[tokio::test(flavor = "current_thread")]
    async fn verified_governed_database_opens_read_only_and_closes_explicitly() {
        let (_directory, path, spec) = database_fixture(governed_schema()).await;
        let geocoder = Geocoder::open(path.clone(), &spec)
            .await
            .expect("open verified database");
        let mut connection = geocoder.connection.lock().await;
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM geonames")
            .fetch_one(&mut *connection)
            .await
            .expect("query fixture");
        assert_eq!(count, 8);
        let query_only = sqlx::query_scalar::<_, i64>("PRAGMA query_only")
            .fetch_one(&mut *connection)
            .await
            .expect("read query-only policy");
        let trusted_schema = sqlx::query_scalar::<_, i64>("PRAGMA trusted_schema")
            .fetch_one(&mut *connection)
            .await
            .expect("read trusted-schema policy");
        assert_eq!((query_only, trusted_schema), (1, 0));
        assert!(
            sqlx::query("DELETE FROM geonames")
                .execute(&mut *connection)
                .await
                .is_err()
        );
        drop(connection);
        geocoder.close().await.expect("explicit close");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn forward_and_feature_queries_preserve_text_ids_and_stable_order() {
        let (_directory, path, spec) = database_fixture(governed_schema()).await;
        let geocoder = Geocoder::open(path, &spec).await.expect("geocoder");

        let structured = crate::Query::locality("Victoria")
            .expect("locality")
            .with_region("BC")
            .expect("region")
            .with_country("Canada")
            .expect("country");
        let result = geocoder.query(&structured).await.expect("structured query");
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
                .await
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
            .await
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
                .await
                .expect("feature result")
                .as_candidates()
                .expect("candidates")[0]
                .admin1_id(),
            Some("WA")
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reverse_and_country_queries_are_bounded_and_deterministic() {
        let (_directory, path, spec) = database_fixture(governed_schema()).await;
        let geocoder = Geocoder::open(path, &spec).await.expect("geocoder");
        let reverse = crate::Query::reverse(crate::Point::new(49.0, -124.0).expect("point"))
            .with_radius_degrees(0.1)
            .expect("radius")
            .with_limit(2)
            .expect("limit");
        let candidates = geocoder
            .query(&reverse)
            .await
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
            .await
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
                .await
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
                .await
                .expect("pole result")
                .as_candidates()
                .expect("candidates")
                .iter()
                .map(|candidate| candidate.feature_id())
                .collect::<Vec<_>>(),
            vec![30, 31]
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn corrupt_bytes_and_incomplete_schema_fail_closed() {
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
            Geocoder::open(&path, &corrupt_spec).await,
            Err(Error::InvalidDatabase)
        ));

        let (_directory, path, spec) =
            database_fixture("CREATE TABLE geonames (id INTEGER);").await;
        assert!(matches!(
            Geocoder::open(path, &spec).await,
            Err(Error::InvalidDatabaseSchema)
        ));
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "current_thread")]
    async fn verified_database_open_rejects_symlink_assets() {
        use std::os::unix::fs::symlink;

        let (directory, path, spec) = database_fixture(governed_schema()).await;
        let link = directory.path().join("linked.db");
        symlink(path, &link).expect("asset symlink");
        assert!(matches!(
            Geocoder::open(link, &spec).await,
            Err(Error::UnsafeAssetDestination)
        ));
    }

    #[test]
    fn parsing_and_filter_helpers_cover_direct_alias_and_no_match_paths() {
        let point = Point::new(1.0, 2.0).expect("point");
        let washington = Candidate::from_provider_row(
            1,
            "Victoria".to_owned(),
            Some("WA".to_owned()),
            Some("Washington".to_owned()),
            "US".to_owned(),
            Some("United States".to_owned()),
            point,
        );
        assert!(country_matches(&washington, "us"));
        assert!(country_matches(&washington, "united states"));
        assert!(!country_matches(&washington, "canada"));
        assert!(region_matches(&washington, "wa"));
        assert!(region_matches(&washington, "washington"));

        let legacy_washington = Candidate::from_provider_row(
            2,
            "Legacy".to_owned(),
            Some("53".to_owned()),
            Some("Washington".to_owned()),
            "US".to_owned(),
            None,
            point,
        );
        assert!(region_matches(&legacy_washington, "wa"));
        assert!(!country_matches(&legacy_washington, "canada"));

        let unclassified = Candidate::from_provider_row(
            3,
            "Island".to_owned(),
            None,
            None,
            "FJ".to_owned(),
            None,
            point,
        );
        assert!(!region_matches(&unclassified, "unknown"));
        assert!(region_aliases("FJ").is_empty());
        assert!(!region_aliases("CA").is_empty());
        assert!(!region_aliases("us").is_empty());
        assert_eq!(normalize_name("  New   York  "), "new york");
        assert_eq!(normalize_region_code("b.c."), "BC");

        let empty = parse_freeform_query(", ,");
        assert!(empty.locality.is_empty());
        let one = parse_freeform_query("Victoria");
        assert_eq!(one.locality, "Victoria");
        let two = parse_freeform_query("Victoria, BC");
        assert_eq!(two.region.as_deref(), Some("BC"));
        assert_eq!(two.country, None);
        let many = parse_freeform_query("Greater, Victoria, BC, CA");
        assert_eq!(many.locality, "Greater, Victoria");
        assert_eq!(many.country.as_deref(), Some("CA"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn database_open_and_row_mapping_fail_closed_for_invalid_shapes() {
        let directory = tempdir().expect("tempdir");
        let placeholder = AssetSpec::new(
            "v1",
            "asset.db",
            "https://assets.example/a",
            "assets.example",
            1,
            [0; 32],
        )
        .expect("placeholder spec");
        assert!(matches!(
            Geocoder::open(directory.path(), &placeholder).await,
            Err(Error::UnsafeAssetDestination)
        ));

        let invalid_row_schema = governed_schema().replace(
            "(6174041, 'Victoria', 2, 'British Columbia', 'CA', 'Canada', 48.4284, -123.3656)",
            "(-1, 'Victoria', 2, 'British Columbia', 'CA', 'Canada', 48.4284, -123.3656)",
        );
        let (_directory, path, spec) = database_fixture(&invalid_row_schema).await;
        let geocoder = Geocoder::open(path, &spec)
            .await
            .expect("open negative-id fixture");
        let query = crate::Query::locality("Victoria").expect("query");
        assert!(matches!(
            geocoder.query(&query).await,
            Err(Error::DatabaseOperationFailed { operation: "query" })
        ));
    }
}

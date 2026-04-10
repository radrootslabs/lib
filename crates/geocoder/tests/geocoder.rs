use radroots_geocoder::{
    Geocoder, GeocoderCountryListResult, GeocoderError, GeocoderPoint, GeocoderReverseOptions,
};
use rusqlite::Connection;
use std::fs;
use std::path::Path;
use tempfile::NamedTempFile;

#[test]
fn reverse_returns_nearest_match_by_default() {
    let geocoder = open_fixture_geocoder();

    let results = geocoder
        .reverse(
            GeocoderPoint {
                lat: 37.7749,
                lng: -122.4194,
            },
            None,
        )
        .expect("reverse query");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 1);
    assert_eq!(results[0].name, "San Francisco");
    assert_eq!(results[0].country_id, "US");
    assert_eq!(results[0].admin1_id, Some(6));
    assert_eq!(results[0].admin1_name.as_deref(), Some("California"));
}

#[test]
fn reverse_respects_limit_and_returns_sorted_matches() {
    let geocoder = open_fixture_geocoder();

    let results = geocoder
        .reverse(
            GeocoderPoint {
                lat: 37.7749,
                lng: -122.4194,
            },
            Some(GeocoderReverseOptions {
                limit: 2,
                degree_offset: 10.0,
            }),
        )
        .expect("reverse query");

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, 1);
    assert_eq!(results[1].id, 2);
}

#[test]
fn reverse_orders_high_latitude_results_by_scaled_longitude_distance() {
    let geocoder = open_high_latitude_geocoder();

    let results = geocoder
        .reverse(
            GeocoderPoint {
                lat: 75.0,
                lng: 0.0,
            },
            Some(GeocoderReverseOptions {
                limit: 2,
                degree_offset: 1.0,
            }),
        )
        .expect("reverse query");

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, 1);
    assert_eq!(results[0].name, "Polar East");
    assert_eq!(results[1].id, 2);
    assert_eq!(results[1].name, "Polar North");
}

#[test]
fn open_bytes_supports_reverse_queries() {
    let path = build_fixture_database();
    let bytes = fs::read(&path).expect("read fixture database bytes");
    let geocoder = Geocoder::open_bytes(&bytes).expect("open byte-backed geocoder");

    let results = geocoder
        .reverse(
            GeocoderPoint {
                lat: 34.0522,
                lng: -118.2437,
            },
            None,
        )
        .expect("reverse query");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 2);
    assert_eq!(results[0].name, "Los Angeles");
}

#[test]
fn open_path_supports_string_and_path_ref_inputs() {
    let path = build_fixture_database();
    let path_str = path.to_str().expect("utf-8 fixture path");

    let geocoder_from_str = Geocoder::open_path(path_str).expect("open geocoder from string path");
    let string_results = geocoder_from_str
        .country("US")
        .expect("country query from string-path geocoder");
    assert_eq!(string_results.len(), 3);

    let geocoder_from_path =
        Geocoder::open_path(Path::new(path_str)).expect("open geocoder from path ref");
    let path_results = geocoder_from_path
        .country("US")
        .expect("country query from path-ref geocoder");
    assert_eq!(path_results.len(), 3);
}

#[test]
fn open_path_supports_pathbuf_inputs() {
    let temp_path = build_fixture_database();
    let path = temp_path.to_path_buf();

    let geocoder_from_pathbuf =
        Geocoder::open_path(path.clone()).expect("open geocoder from pathbuf");
    let pathbuf_results = geocoder_from_pathbuf
        .country("US")
        .expect("country query from pathbuf geocoder");
    assert_eq!(pathbuf_results.len(), 3);

    let geocoder_from_pathbuf_ref =
        Geocoder::open_path(&path).expect("open geocoder from pathbuf ref");
    let pathbuf_ref_results = geocoder_from_pathbuf_ref
        .country("US")
        .expect("country query from pathbuf-ref geocoder");
    assert_eq!(pathbuf_ref_results.len(), 3);
}

#[test]
fn country_returns_all_rows_for_requested_country() {
    let geocoder = open_fixture_geocoder();

    let results = geocoder.country("US").expect("country query");

    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|result| result.country_id == "US"));
}

#[test]
fn country_list_returns_average_centers() {
    let geocoder = open_fixture_geocoder();

    let results = geocoder.country_list().expect("country list query");

    assert_eq!(results.len(), 2);
    assert_eq!(
        results[0],
        GeocoderCountryListResult {
            country_id: "BR".to_owned(),
            country: Some("Brazil".to_owned()),
            lat: -23.5505,
            lng: -46.6333,
        }
    );
    assert_eq!(results[1].country_id, "US");
    assert_eq!(results[1].country.as_deref(), Some("United States"));
    assert!(approx_eq(
        results[1].lat,
        (37.7749 + 34.0522 + 40.7128) / 3.0
    ));
    assert!(approx_eq(
        results[1].lng,
        (-122.4194 + -118.2437 + -74.0060) / 3.0
    ));
}

#[test]
fn country_center_returns_average_for_country() {
    let geocoder = open_fixture_geocoder();

    let result = geocoder.country_center("US").expect("country center query");

    assert!(approx_eq(result.lat, (37.7749 + 34.0522 + 40.7128) / 3.0));
    assert!(approx_eq(
        result.lng,
        (-122.4194 + -118.2437 + -74.0060) / 3.0
    ));
}

#[test]
fn country_center_returns_not_found_for_missing_country() {
    let geocoder = open_fixture_geocoder();

    let err = geocoder
        .country_center("ZZ")
        .expect_err("missing country should return not found");
    assert_country_center_not_found(err, "ZZ");
}

#[test]
fn reverse_country_and_country_list_report_missing_schema_errors() {
    let geocoder = open_empty_geocoder();

    let reverse_err = geocoder
        .reverse(
            GeocoderPoint {
                lat: 37.7749,
                lng: -122.4194,
            },
            None,
        )
        .expect_err("reverse should fail without schema");
    assert_sqlite_error_contains(reverse_err, "no such");

    let country_err = geocoder
        .country("US")
        .expect_err("country should fail without schema");
    assert_sqlite_error_contains(country_err, "no such");

    let country_list_err = geocoder
        .country_list()
        .expect_err("country_list should fail without schema");
    assert_sqlite_error_contains(country_list_err, "no such");
}

#[test]
fn country_center_reports_missing_schema_errors() {
    let geocoder = open_empty_geocoder();

    let err = geocoder
        .country_center("US")
        .expect_err("country_center should fail without schema");
    assert_sqlite_error_contains(err, "no such");
}

#[test]
fn reverse_and_country_propagate_row_mapping_errors() {
    let geocoder = open_reverse_country_row_error_geocoder();

    let reverse_err = geocoder
        .reverse(
            GeocoderPoint {
                lat: 37.7749,
                lng: -122.4194,
            },
            Some(GeocoderReverseOptions {
                limit: 1,
                degree_offset: 10.0,
            }),
        )
        .expect_err("reverse should fail on invalid row mapping");
    assert_sqlite_error_contains(reverse_err, "Invalid column type");

    let country_err = geocoder
        .country("US")
        .expect_err("country should fail on invalid row mapping");
    assert_sqlite_error_contains(country_err, "Invalid column type");
}

#[test]
fn country_list_propagates_aggregate_row_mapping_errors() {
    let geocoder = open_country_list_row_error_geocoder();

    let err = geocoder
        .country_list()
        .expect_err("country_list should fail on null aggregate row");
    assert_sqlite_error_contains(err, "Invalid column type");
}

fn open_fixture_geocoder() -> Geocoder {
    let path = build_fixture_database();
    Geocoder::open_path(&path).expect("open geocoder")
}

fn open_high_latitude_geocoder() -> Geocoder {
    let path = build_high_latitude_database();
    Geocoder::open_path(&path).expect("open geocoder")
}

fn open_empty_geocoder() -> Geocoder {
    let temp = NamedTempFile::new().expect("temp db");
    let path = temp.into_temp_path();
    Geocoder::open_path(&path).expect("open empty geocoder")
}

fn open_reverse_country_row_error_geocoder() -> Geocoder {
    let temp = NamedTempFile::new().expect("temp db");
    let path = temp.into_temp_path();
    seed_reverse_country_row_error_database(path.to_str().expect("utf-8 temp path"));
    Geocoder::open_path(&path).expect("open invalid row geocoder")
}

fn open_country_list_row_error_geocoder() -> Geocoder {
    let temp = NamedTempFile::new().expect("temp db");
    let path = temp.into_temp_path();
    seed_country_list_row_error_database(path.to_str().expect("utf-8 temp path"));
    Geocoder::open_path(&path).expect("open aggregate error geocoder")
}

fn build_fixture_database() -> tempfile::TempPath {
    let temp = NamedTempFile::new().expect("temp db");
    let path = temp.into_temp_path();
    seed_fixture_database(path.to_str().expect("utf-8 temp path"));
    path
}

fn build_high_latitude_database() -> tempfile::TempPath {
    let temp = NamedTempFile::new().expect("temp db");
    let path = temp.into_temp_path();
    seed_high_latitude_database(path.to_str().expect("utf-8 temp path"));
    path
}

fn seed_fixture_database(path: &str) {
    let conn = Connection::open(path).expect("open fixture database");
    seed_schema(&conn);

    insert_country(&conn, "US", "United States");
    insert_country(&conn, "BR", "Brazil");

    insert_admin1(&conn, "US", 6, "California");
    insert_admin1(&conn, "US", 36, "New York");
    insert_admin1(&conn, "BR", 27, "Sao Paulo");

    insert_feature(&conn, 1, "San Francisco", "US", 6, 37.7749, -122.4194);
    insert_feature(&conn, 2, "Los Angeles", "US", 6, 34.0522, -118.2437);
    insert_feature(&conn, 3, "New York City", "US", 36, 40.7128, -74.0060);
    insert_feature(&conn, 4, "Sao Paulo", "BR", 27, -23.5505, -46.6333);
}

fn seed_high_latitude_database(path: &str) {
    let conn = Connection::open(path).expect("open fixture database");
    seed_schema(&conn);

    insert_country(&conn, "NO", "Norway");
    insert_admin1(&conn, "NO", 1, "Nord");

    insert_feature(&conn, 1, "Polar East", "NO", 1, 75.02, 0.10);
    insert_feature(&conn, 2, "Polar North", "NO", 1, 75.05, 0.05);
}

fn seed_reverse_country_row_error_database(path: &str) {
    let conn = Connection::open(path).expect("open invalid row fixture database");
    conn.execute_batch(
        r#"
        CREATE TABLE geonames(
          id INTEGER,
          name TEXT,
          admin1_id INTEGER,
          admin1_name TEXT,
          country_id TEXT,
          country_name TEXT,
          latitude REAL,
          longitude REAL
        );
        CREATE TABLE coordinates(
          feature_id INTEGER,
          latitude REAL,
          longitude REAL
        );
        "#,
    )
    .expect("create invalid row schema");
    conn.execute(
        "INSERT INTO geonames (id, name, admin1_id, admin1_name, country_id, country_name, latitude, longitude) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![1_i64, Option::<String>::None, Option::<i64>::None, Option::<String>::None, "US", "United States", 37.7749_f64, -122.4194_f64],
    )
    .expect("insert invalid reverse/country row");
    conn.execute(
        "INSERT INTO coordinates (feature_id, latitude, longitude) VALUES (?1, ?2, ?3)",
        (1_i64, 37.7749_f64, -122.4194_f64),
    )
    .expect("insert invalid reverse/country coordinate");
}

fn seed_country_list_row_error_database(path: &str) {
    let conn = Connection::open(path).expect("open aggregate error fixture database");
    conn.execute_batch(
        r#"
        CREATE TABLE geonames(
          country_id TEXT,
          country_name TEXT,
          latitude REAL,
          longitude REAL
        );
        "#,
    )
    .expect("create aggregate error schema");
    conn.execute(
        "INSERT INTO geonames (country_id, country_name, latitude, longitude) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params!["US", "United States", Option::<f64>::None, Option::<f64>::None],
    )
    .expect("insert aggregate error row");
}

fn seed_schema(conn: &Connection) {
    conn.execute_batch(
        r#"
        CREATE TABLE countries(
          id TEXT,
          name TEXT,
          PRIMARY KEY (id)
        );
        CREATE TABLE admin1(
          country_id TEXT,
          id INTEGER,
          name TEXT,
          PRIMARY KEY (country_id, id)
        );
        CREATE TABLE features(
          id INTEGER,
          name TEXT,
          country_id TEXT,
          admin1_id INTEGER,
          PRIMARY KEY (id)
        );
        CREATE TABLE coordinates(
          feature_id INTEGER,
          latitude REAL,
          longitude REAL,
          PRIMARY KEY (feature_id)
        );
        CREATE INDEX coordinates_lat_lng ON coordinates (latitude, longitude);
        CREATE VIEW geonames AS
          SELECT
            features.id,
            features.name,
            admin1.id AS admin1_id,
            admin1.name AS admin1_name,
            countries.id AS country_id,
            countries.name AS country_name,
            coordinates.latitude AS latitude,
            coordinates.longitude AS longitude
          FROM features
            LEFT JOIN countries ON features.country_id = countries.id
            LEFT JOIN admin1 ON features.country_id = admin1.country_id AND features.admin1_id = admin1.id
            JOIN coordinates ON features.id = coordinates.feature_id;
        "#,
    )
    .expect("create fixture schema");
}

fn insert_country(conn: &Connection, id: &str, name: &str) {
    conn.execute(
        "INSERT INTO countries (id, name) VALUES (?1, ?2)",
        (id, name),
    )
    .expect("insert country");
}

fn insert_admin1(conn: &Connection, country_id: &str, id: i64, name: &str) {
    conn.execute(
        "INSERT INTO admin1 (country_id, id, name) VALUES (?1, ?2, ?3)",
        (country_id, id, name),
    )
    .expect("insert admin1");
}

fn insert_feature(
    conn: &Connection,
    id: i64,
    name: &str,
    country_id: &str,
    admin1_id: i64,
    latitude: f64,
    longitude: f64,
) {
    conn.execute(
        "INSERT INTO features (id, name, country_id, admin1_id) VALUES (?1, ?2, ?3, ?4)",
        (id, name, country_id, admin1_id),
    )
    .expect("insert feature");
    conn.execute(
        "INSERT INTO coordinates (feature_id, latitude, longitude) VALUES (?1, ?2, ?3)",
        (id, latitude, longitude),
    )
    .expect("insert coordinate");
}

fn approx_eq(left: f64, right: f64) -> bool {
    (left - right).abs() < 0.000_001
}

fn assert_sqlite_error_contains(err: GeocoderError, needle: &str) {
    match err {
        GeocoderError::Sqlite(inner) => assert!(
            inner.to_string().contains(needle),
            "expected sqlite error containing {needle:?}, got {inner}"
        ),
        other => panic!("expected sqlite error, got {other}"),
    }
}

fn assert_country_center_not_found(err: GeocoderError, country_id: &str) {
    match err {
        GeocoderError::CountryCenterNotFound { country_id: actual } => {
            assert_eq!(actual, country_id);
        }
        other => panic!("expected CountryCenterNotFound, got {other}"),
    }
}

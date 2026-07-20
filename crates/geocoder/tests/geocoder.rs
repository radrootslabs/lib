use radroots_geocoder::{
    GEONAMES_ASSET_HOST, GeoNamesAssetFetcher, GeoNamesAssetSpec, GeoNamesAssetState, Geocoder,
    GeocoderCountryListResult, GeocoderError, GeocoderLocalityLookup, GeocoderLocalityQuery,
    GeocoderPoint, GeocoderReverseOptions, default_geonames_asset_path_from_cache_root,
    ensure_geonames_asset_in_cache_root_with_fetcher, ensure_geonames_asset_path_with_fetcher,
    inspect_default_geonames_asset_in_cache_root, inspect_geonames_asset_path,
    validate_geonames_asset_file, validate_geonames_asset_spec_source,
};
use sha2::Digest;
use sqlx::Connection;
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};
use std::cell::Cell;
use std::fs;
use std::path::Path;
use tempfile::NamedTempFile;

const TEST_ASSET_URL: &str = "https://assets.radroots.io/data/geonames/geonames-test.db";

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
    assert_eq!(results[0].admin1_id.as_deref(), Some("6"));
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
fn locality_resolves_structured_query_freeform_query_id_and_ambiguity() {
    let geocoder = open_forward_fixture_geocoder();

    let structured = geocoder
        .locality(
            &GeocoderLocalityQuery::structured("Fixture Victoria")
                .with_region("BC")
                .with_country("CA"),
        )
        .expect("structured lookup");
    assert_unique_locality(
        structured,
        3001,
        "Fixture Victoria, British Columbia, Canada",
    );

    let freeform = geocoder
        .locality(&GeocoderLocalityQuery::query("Fixture Victoria, BC, CA"))
        .expect("freeform lookup");
    assert_unique_locality(freeform, 3001, "Fixture Victoria, British Columbia, Canada");

    let feature_id = geocoder
        .locality(&GeocoderLocalityQuery::feature_id(3004))
        .expect("feature-id lookup");
    assert_unique_locality(
        feature_id,
        3004,
        "Identifier Grove, British Columbia, Canada",
    );

    let ambiguous = geocoder
        .locality(&GeocoderLocalityQuery::structured("Shared Market").with_country("CA"))
        .expect("ambiguous lookup");
    let GeocoderLocalityLookup::Ambiguous { candidates } = ambiguous else {
        panic!("expected ambiguous lookup");
    };
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>(),
        vec![3002, 3003]
    );

    let no_match = geocoder
        .locality(&GeocoderLocalityQuery::structured("Missing Market").with_country("CA"))
        .expect("no-match lookup");
    assert!(matches!(no_match, GeocoderLocalityLookup::NoMatch));
}

#[test]
fn locality_normalizes_and_orders_mixed_administrative_identifiers() {
    let geocoder = open_mixed_admin1_geocoder();

    let lookup = geocoder
        .locality(&GeocoderLocalityQuery::structured("Mixed Locality").with_country("ZZ"))
        .expect("mixed-type locality lookup");
    let GeocoderLocalityLookup::Ambiguous { candidates } = lookup else {
        panic!("expected ambiguous mixed-type locality lookup");
    };

    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>(),
        vec![9004, 9002, 9003, 9001]
    );
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.admin1_id.as_deref())
            .collect::<Vec<_>>(),
        vec![None, Some("2"), Some("10"), Some("HCW")]
    );
}

#[test]
fn reverse_normalizes_mixed_administrative_identifiers_with_stable_ties() {
    let geocoder = open_mixed_admin1_geocoder();

    let results = geocoder
        .reverse(
            GeocoderPoint { lat: 1.0, lng: 2.0 },
            Some(GeocoderReverseOptions {
                limit: 4,
                degree_offset: 0.1,
            }),
        )
        .expect("mixed-type reverse lookup");

    assert_eq!(
        results.iter().map(|result| result.id).collect::<Vec<_>>(),
        vec![9001, 9002, 9003, 9004]
    );
    assert_eq!(
        results
            .iter()
            .map(|result| result.admin1_id.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("HCW"), Some("2"), Some("10"), None]
    );
}

#[test]
fn locality_query_builders_cover_blank_single_region_alias_and_display_fallbacks() {
    let geocoder = open_forward_fixture_geocoder();

    let ignored_builder_fields = GeocoderLocalityQuery::feature_id(3004)
        .with_region("ignored")
        .with_country("ignored")
        .with_limit(0);
    let selected = geocoder
        .locality(&ignored_builder_fields)
        .expect("feature-id lookup");
    assert_unique_locality(selected, 3004, "Identifier Grove, British Columbia, Canada");

    let blank = geocoder
        .locality(&GeocoderLocalityQuery::query(" , , "))
        .expect("blank freeform lookup");
    assert!(matches!(blank, GeocoderLocalityLookup::NoMatch));

    let single = geocoder
        .locality(&GeocoderLocalityQuery::query("Fixture Victoria"))
        .expect("single-part freeform lookup");
    assert_unique_locality(single, 3001, "Fixture Victoria, British Columbia, Canada");

    let two_part = geocoder
        .locality(&GeocoderLocalityQuery::query(
            "Fixture Victoria, British Columbia",
        ))
        .expect("two-part freeform lookup");
    assert_unique_locality(two_part, 3001, "Fixture Victoria, British Columbia, Canada");

    let us_alias = geocoder
        .locality(
            &GeocoderLocalityQuery::structured("Alias Market")
                .with_region("CA")
                .with_country("US"),
        )
        .expect("us alias lookup");
    assert_unique_locality(us_alias, 3006, "Alias Market, California, United States");

    let fallback_display = geocoder
        .locality(&GeocoderLocalityQuery::feature_id(3007))
        .expect("fallback display lookup");
    assert_unique_locality(fallback_display, 3007, "No Country Place, ZZ");

    let missing_region = geocoder
        .locality(
            &GeocoderLocalityQuery::structured("No Country Place")
                .with_region("Missing Region")
                .with_country("ZZ"),
        )
        .expect("missing region lookup");
    assert!(matches!(missing_region, GeocoderLocalityLookup::NoMatch));

    let no_alias_region = geocoder
        .locality(
            &GeocoderLocalityQuery::structured("No Alias Place")
                .with_region("NA")
                .with_country("ZZ"),
        )
        .expect("country without region alias lookup");
    assert!(matches!(no_alias_region, GeocoderLocalityLookup::NoMatch));

    let ambiguous_zero_limit = geocoder
        .locality(
            &GeocoderLocalityQuery::structured("Shared Market")
                .with_country("CA")
                .with_limit(0),
        )
        .expect("zero-limit ambiguous lookup");
    let GeocoderLocalityLookup::Ambiguous { candidates } = ambiguous_zero_limit else {
        panic!("expected ambiguous lookup");
    };
    assert_eq!(candidates.len(), 1);
}

#[test]
fn geonames_asset_public_helpers_refresh_validate_and_open_verified_fixture() {
    let cache_root = tempfile::tempdir().expect("cache root");
    let source_path = build_fixture_database();
    let bytes = fs::read(&source_path).expect("fixture database bytes");
    let spec = fixture_asset_spec(&bytes, TEST_ASSET_URL);

    let default_path = default_geonames_asset_path_from_cache_root(cache_root.path());
    assert!(default_path.ends_with(Path::new("geonames-1.0.db")));

    let default_missing = inspect_default_geonames_asset_in_cache_root(cache_root.path())
        .expect("inspect default missing asset");
    assert_eq!(default_missing.state, GeoNamesAssetState::Missing);

    let fetcher = BytesFetcher {
        bytes,
        calls: Cell::new(0),
    };
    let refreshed =
        ensure_geonames_asset_in_cache_root_with_fetcher(cache_root.path(), &spec, &fetcher)
            .expect("refresh asset");
    assert_eq!(refreshed.state, GeoNamesAssetState::Refreshed);
    assert_eq!(fetcher.calls.get(), 1);

    let inspected = inspect_geonames_asset_path(&refreshed.path, &spec).expect("inspect asset");
    assert_eq!(inspected.state, GeoNamesAssetState::Available);

    let validated = validate_geonames_asset_file(&refreshed.path, &spec).expect("validate asset");
    assert_eq!(validated.sha256, inspected.sha256);

    let available = ensure_geonames_asset_path_with_fetcher(&refreshed.path, &spec, &fetcher)
        .expect("available asset");
    assert_eq!(available.state, GeoNamesAssetState::Available);
    assert_eq!(fetcher.calls.get(), 1);

    let geocoder = Geocoder::open_verified_geonames_asset(&refreshed.path, &spec)
        .expect("open verified geocoder");
    let country = geocoder.country("US").expect("country query");
    assert_eq!(country.len(), 3);
}

#[test]
fn geonames_asset_public_validation_rejects_invalid_url_shapes() {
    let source_path = build_fixture_database();
    let bytes = fs::read(&source_path).expect("fixture database bytes");

    let bad_parse = GeoNamesAssetSpec {
        url: "not a url",
        ..fixture_asset_spec(&bytes, TEST_ASSET_URL)
    };
    assert!(matches!(
        validate_geonames_asset_spec_source(&bad_parse),
        Err(GeocoderError::InvalidAssetUrl { .. })
    ));

    let bad_scheme = GeoNamesAssetSpec {
        url: "http://assets.radroots.io/data/geonames/geonames-test.db",
        ..fixture_asset_spec(&bytes, TEST_ASSET_URL)
    };
    assert!(matches!(
        validate_geonames_asset_spec_source(&bad_scheme),
        Err(GeocoderError::InvalidAssetUrl { .. })
    ));
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
    assert_sqlite_error_contains(reverse_err, "unexpected null");

    let country_err = geocoder
        .country("US")
        .expect_err("country should fail on invalid row mapping");
    assert_sqlite_error_contains(country_err, "unexpected null");
}

#[test]
fn country_list_propagates_aggregate_row_mapping_errors() {
    let geocoder = open_country_list_row_error_geocoder();

    let err = geocoder
        .country_list()
        .expect_err("country_list should fail on null aggregate row");
    assert_sqlite_error_contains(err, "unexpected null");
}

fn open_fixture_geocoder() -> Geocoder {
    let path = build_fixture_database();
    Geocoder::open_path(&path).expect("open geocoder")
}

fn open_high_latitude_geocoder() -> Geocoder {
    let path = build_high_latitude_database();
    Geocoder::open_path(&path).expect("open geocoder")
}

fn open_forward_fixture_geocoder() -> Geocoder {
    let path = build_forward_fixture_database();
    Geocoder::open_path(&path).expect("open geocoder")
}

fn open_mixed_admin1_geocoder() -> Geocoder {
    let path = build_mixed_admin1_database();
    Geocoder::open_path(&path).expect("open mixed administrative identifier geocoder")
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

fn build_forward_fixture_database() -> tempfile::TempPath {
    let temp = NamedTempFile::new().expect("temp db");
    let path = temp.into_temp_path();
    seed_forward_fixture_database(path.to_str().expect("utf-8 temp path"));
    path
}

fn build_mixed_admin1_database() -> tempfile::TempPath {
    let temp = NamedTempFile::new().expect("temp db");
    let path = temp.into_temp_path();
    seed_mixed_admin1_database(path.to_str().expect("utf-8 temp path"));
    path
}

fn seed_fixture_database(path: &str) {
    let mut conn = open_test_path_connection(path);
    seed_schema(&mut conn);

    insert_country(&mut conn, "US", "United States");
    insert_country(&mut conn, "BR", "Brazil");

    insert_admin1(&mut conn, "US", 6, "California");
    insert_admin1(&mut conn, "US", 36, "New York");
    insert_admin1(&mut conn, "BR", 27, "Sao Paulo");

    insert_feature(&mut conn, 1, "San Francisco", "US", 6, 37.7749, -122.4194);
    insert_feature(&mut conn, 2, "Los Angeles", "US", 6, 34.0522, -118.2437);
    insert_feature(&mut conn, 3, "New York City", "US", 36, 40.7128, -74.0060);
    insert_feature(&mut conn, 4, "Sao Paulo", "BR", 27, -23.5505, -46.6333);
}

fn seed_high_latitude_database(path: &str) {
    let mut conn = open_test_path_connection(path);
    seed_schema(&mut conn);

    insert_country(&mut conn, "NO", "Norway");
    insert_admin1(&mut conn, "NO", 1, "Nord");

    insert_feature(&mut conn, 1, "Polar East", "NO", 1, 75.02, 0.10);
    insert_feature(&mut conn, 2, "Polar North", "NO", 1, 75.05, 0.05);
}

fn seed_forward_fixture_database(path: &str) {
    let mut conn = open_test_path_connection(path);
    seed_schema(&mut conn);

    insert_country(&mut conn, "CA", "Canada");
    insert_country(&mut conn, "US", "United States");
    futures_executor::block_on(
        sqlx::query("INSERT INTO countries (id, name) VALUES (?, ?)")
            .bind("ZZ")
            .bind(Option::<String>::None)
            .execute(&mut conn),
    )
    .expect("insert unnamed country");

    insert_admin1(&mut conn, "CA", 2, "British Columbia");
    insert_admin1(&mut conn, "CA", 3, "Prairie Region");
    insert_admin1(&mut conn, "US", 4, "River Region");
    insert_admin1(&mut conn, "US", 6, "California");
    insert_admin1(&mut conn, "ZZ", 100, "No Alias Region");

    insert_feature(
        &mut conn,
        3001,
        "Fixture Victoria",
        "CA",
        2,
        48.4359,
        -123.35155,
    );
    insert_feature(&mut conn, 3002, "Shared Market", "CA", 2, 48.7, -123.2);
    insert_feature(&mut conn, 3003, "Shared Market", "CA", 3, 50.2, -110.4);
    insert_feature(&mut conn, 3004, "Identifier Grove", "CA", 2, 48.9, -123.4);
    insert_feature(&mut conn, 3005, "Query Hamlet", "US", 4, 39.25, -77.5);
    insert_feature(&mut conn, 3006, "Alias Market", "US", 6, 38.5, -121.5);
    insert_feature(&mut conn, 3007, "No Country Place", "ZZ", 99, 10.0, 11.0);
    insert_feature(&mut conn, 3008, "No Alias Place", "ZZ", 100, 10.5, 11.5);
}

fn seed_mixed_admin1_database(path: &str) {
    let mut conn = open_test_path_connection(path);
    execute_batch(
        &mut conn,
        r#"
        CREATE TABLE geonames(
          id INTEGER,
          name TEXT,
          admin1_id,
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
        INSERT INTO geonames
          (id, name, admin1_id, admin1_name, country_id, country_name, latitude, longitude)
        VALUES
          (9004, 'Mixed Locality', NULL, 'Fixture Region', 'ZZ', 'Fixtureland', 1.0, 2.0),
          (9003, 'Mixed Locality', 10, 'Fixture Region', 'ZZ', 'Fixtureland', 1.0, 2.0),
          (9002, 'Mixed Locality', 2, 'Fixture Region', 'ZZ', 'Fixtureland', 1.0, 2.0),
          (9001, 'Mixed Locality', 'HCW', 'Fixture Region', 'ZZ', 'Fixtureland', 1.0, 2.0);
        INSERT INTO coordinates (feature_id, latitude, longitude)
        VALUES
          (9004, 1.0, 2.0),
          (9003, 1.0, 2.0),
          (9002, 1.0, 2.0),
          (9001, 1.0, 2.0);
        "#,
    );
}

fn seed_reverse_country_row_error_database(path: &str) {
    let mut conn = open_test_path_connection(path);
    execute_batch(
        &mut conn,
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
    );
    futures_executor::block_on(
        sqlx::query("INSERT INTO geonames (id, name, admin1_id, admin1_name, country_id, country_name, latitude, longitude) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(1_i64)
            .bind(Option::<String>::None)
            .bind(Option::<i64>::None)
            .bind(Option::<String>::None)
            .bind("US")
            .bind("United States")
            .bind(37.7749_f64)
            .bind(-122.4194_f64)
            .execute(&mut conn),
    )
    .expect("insert invalid reverse/country row");
    futures_executor::block_on(
        sqlx::query("INSERT INTO coordinates (feature_id, latitude, longitude) VALUES (?, ?, ?)")
            .bind(1_i64)
            .bind(37.7749_f64)
            .bind(-122.4194_f64)
            .execute(&mut conn),
    )
    .expect("insert invalid reverse/country coordinate");
}

fn seed_country_list_row_error_database(path: &str) {
    let mut conn = open_test_path_connection(path);
    execute_batch(
        &mut conn,
        r#"
        CREATE TABLE geonames(
          country_id TEXT,
          country_name TEXT,
          latitude REAL,
          longitude REAL
        );
        "#,
    );
    futures_executor::block_on(
        sqlx::query(
            "INSERT INTO geonames (country_id, country_name, latitude, longitude) VALUES (?, ?, ?, ?)",
        )
        .bind("US")
        .bind("United States")
        .bind(Option::<f64>::None)
        .bind(Option::<f64>::None)
        .execute(&mut conn),
    )
    .expect("insert aggregate error row");
}

fn open_test_path_connection(path: &str) -> SqliteConnection {
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

fn seed_schema(conn: &mut SqliteConnection) {
    execute_batch(
        conn,
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
    );
}

fn insert_country(conn: &mut SqliteConnection, id: &str, name: &str) {
    futures_executor::block_on(
        sqlx::query("INSERT INTO countries (id, name) VALUES (?, ?)")
            .bind(id)
            .bind(name)
            .execute(conn),
    )
    .expect("insert country");
}

fn insert_admin1(conn: &mut SqliteConnection, country_id: &str, id: i64, name: &str) {
    futures_executor::block_on(
        sqlx::query("INSERT INTO admin1 (country_id, id, name) VALUES (?, ?, ?)")
            .bind(country_id)
            .bind(id)
            .bind(name)
            .execute(conn),
    )
    .expect("insert admin1");
}

fn insert_feature(
    conn: &mut SqliteConnection,
    id: i64,
    name: &str,
    country_id: &str,
    admin1_id: i64,
    latitude: f64,
    longitude: f64,
) {
    futures_executor::block_on(
        sqlx::query("INSERT INTO features (id, name, country_id, admin1_id) VALUES (?, ?, ?, ?)")
            .bind(id)
            .bind(name)
            .bind(country_id)
            .bind(admin1_id)
            .execute(&mut *conn),
    )
    .expect("insert feature");
    futures_executor::block_on(
        sqlx::query("INSERT INTO coordinates (feature_id, latitude, longitude) VALUES (?, ?, ?)")
            .bind(id)
            .bind(latitude)
            .bind(longitude)
            .execute(conn),
    )
    .expect("insert coordinate");
}

fn approx_eq(left: f64, right: f64) -> bool {
    (left - right).abs() < 0.000_001
}

fn fixture_asset_spec(bytes: &[u8], url: &'static str) -> GeoNamesAssetSpec {
    let digest = sha2::Sha256::digest(bytes);
    let sha256: &'static str = Box::leak(hex::encode(digest).into_boxed_str());
    GeoNamesAssetSpec {
        version: "test",
        file_name: "geonames-test.db",
        url,
        allowed_host: GEONAMES_ASSET_HOST,
        byte_size: bytes.len() as u64,
        sha256,
    }
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

fn assert_unique_locality(
    lookup: GeocoderLocalityLookup,
    expected_id: i64,
    expected_display_name: &str,
) {
    let GeocoderLocalityLookup::Unique { candidate } = lookup else {
        panic!("expected unique lookup");
    };
    assert_eq!(candidate.id, expected_id);
    assert_eq!(candidate.display_name, expected_display_name);
}

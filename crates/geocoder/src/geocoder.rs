use crate::error::GeocoderError;
use crate::model::{
    GeocoderCountryListResult, GeocoderPoint, GeocoderReverseOptions, GeocoderReverseResult,
};
use rusqlite::{Connection, MAIN_DB, named_params};
use std::path::Path;

pub struct Geocoder {
    conn: Connection,
}

impl Geocoder {
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn open_path<P: AsRef<Path>>(path: P) -> Result<Self, GeocoderError> {
        let conn = Connection::open(path)?;
        Ok(Self { conn })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn open_bytes(bytes: &[u8]) -> Result<Self, GeocoderError> {
        let mut conn = Connection::open_in_memory()?;
        conn.deserialize_read_exact(MAIN_DB, bytes, bytes.len(), true)?;
        Ok(Self { conn })
    }

    pub fn reverse(
        &self,
        point: GeocoderPoint,
        options: Option<GeocoderReverseOptions>,
    ) -> Result<Vec<GeocoderReverseResult>, GeocoderError> {
        let options = options.unwrap_or_default();
        let lng_weight = point.lat.to_radians().cos().powi(2);
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
              g.id,
              g.name,
              g.admin1_id,
              g.admin1_name,
              g.country_id,
              g.country_name,
              g.latitude,
              g.longitude
            FROM geonames AS g
            JOIN coordinates AS c
              ON g.id = c.feature_id
            WHERE c.latitude BETWEEN :lat - :degree_offset AND :lat + :degree_offset
              AND c.longitude BETWEEN :lng - :degree_offset AND :lng + :degree_offset
            ORDER BY
              ((:lat - c.latitude) * (:lat - c.latitude))
              + ((:lng - c.longitude) * (:lng - c.longitude) * :lng_weight) ASC
            LIMIT :limit
            "#,
        )?;
        let params = named_params! {
            ":lat": point.lat,
            ":lng": point.lng,
            ":degree_offset": options.degree_offset,
            ":lng_weight": lng_weight,
            ":limit": options.limit as i64,
        };
        collect_mapped_rows(&mut stmt, params, map_reverse_row)
    }

    pub fn country(&self, country_id: &str) -> Result<Vec<GeocoderReverseResult>, GeocoderError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
              id,
              name,
              admin1_id,
              admin1_name,
              country_id,
              country_name,
              latitude,
              longitude
            FROM geonames
            WHERE country_id = :country_id
            ORDER BY id ASC
            "#,
        )?;
        collect_mapped_rows(
            &mut stmt,
            named_params! { ":country_id": country_id },
            map_reverse_row,
        )
    }

    pub fn country_list(&self) -> Result<Vec<GeocoderCountryListResult>, GeocoderError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
              country_id,
              country_name,
              AVG(latitude) AS latitude_c,
              AVG(longitude) AS longitude_c
            FROM geonames
            GROUP BY country_id, country_name
            ORDER BY country_id ASC
            "#,
        )?;
        collect_mapped_rows(&mut stmt, [], |row| {
            Ok(GeocoderCountryListResult {
                country_id: row.get("country_id")?,
                country: row.get("country_name")?,
                lat: row.get("latitude_c")?,
                lng: row.get("longitude_c")?,
            })
        })
    }

    pub fn country_center(&self, country_id: &str) -> Result<GeocoderPoint, GeocoderError> {
        finalize_country_center(country_center_impl(&self.conn, country_id), country_id)
    }
}

fn query_country_center_row(
    stmt: &mut rusqlite::Statement<'_>,
    country_id: &str,
) -> rusqlite::Result<(Option<f64>, Option<f64>)> {
    stmt.query_row(
        named_params! { ":country_id": country_id },
        map_country_center_row,
    )
}

fn map_country_center_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(Option<f64>, Option<f64>)> {
    Ok((row.get("latitude_c")?, row.get("longitude_c")?))
}

#[inline(never)]
fn finalize_country_center(
    result: Result<Option<GeocoderPoint>, GeocoderError>,
    country_id: &str,
) -> Result<GeocoderPoint, GeocoderError> {
    let maybe_point = match result {
        Ok(maybe_point) => maybe_point,
        Err(err) => return Err(err),
    };
    if let Some(point) = maybe_point {
        return Ok(point);
    }
    Err(GeocoderError::CountryCenterNotFound {
        country_id: country_id.to_owned(),
    })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn country_center_impl(
    conn: &Connection,
    country_id: &str,
) -> Result<Option<GeocoderPoint>, GeocoderError> {
    let mut stmt = conn.prepare(
        r#"
        SELECT
          AVG(latitude) AS latitude_c,
          AVG(longitude) AS longitude_c
        FROM geonames
        WHERE country_id = :country_id
        "#,
    )?;
    let (lat, lng) = query_country_center_row(&mut stmt, country_id)?;
    if let (Some(lat), Some(lng)) = (lat, lng) {
        return Ok(Some(GeocoderPoint { lat, lng }));
    }
    Ok(None)
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn collect_mapped_rows<T, P, F>(
    stmt: &mut rusqlite::Statement<'_>,
    params: P,
    map: F,
) -> Result<Vec<T>, GeocoderError>
where
    P: rusqlite::Params,
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    let rows = stmt.query_map(params, map)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(GeocoderError::from)
}

fn map_reverse_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<GeocoderReverseResult> {
    Ok(GeocoderReverseResult {
        id: row.get("id")?,
        name: row.get("name")?,
        admin1_id: row.get("admin1_id")?,
        admin1_name: row.get("admin1_name")?,
        country_id: row.get("country_id")?,
        country_name: row.get("country_name")?,
        latitude: row.get("latitude")?,
        longitude: row.get("longitude")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn unit_harness_covers_success_paths() {
        let geocoder = open_fixture_geocoder();

        let reverse = geocoder
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
        assert_eq!(reverse.len(), 2);
        assert_eq!(reverse[0].id, 1);

        let country = geocoder.country("US").expect("country query");
        assert_eq!(country.len(), 3);

        let countries = geocoder.country_list().expect("country list query");
        assert_eq!(countries.len(), 2);
        assert_eq!(countries[0].country_id, "BR");

        let center = geocoder.country_center("US").expect("country center query");
        assert!(approx_eq(center.lat, (37.7749 + 34.0522 + 40.7128) / 3.0));
        assert!(approx_eq(
            center.lng,
            (-122.4194 + -118.2437 + -74.0060) / 3.0
        ));
    }

    #[test]
    fn unit_harness_covers_open_bytes_and_weighted_reverse_ordering() {
        let path = build_high_latitude_database();
        let bytes = fs::read(&path).expect("read fixture database bytes");
        let geocoder = Geocoder::open_bytes(&bytes).expect("open byte-backed geocoder");

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
        assert_eq!(results[0].name, "Polar East");
        assert_eq!(results[1].name, "Polar North");
    }

    #[test]
    fn unit_harness_covers_open_path_pathbuf_instantiation() {
        let path = build_fixture_database();
        let geocoder = Geocoder::open_path(path.to_path_buf()).expect("open geocoder from pathbuf");
        let results = geocoder
            .country("US")
            .expect("country query from pathbuf geocoder");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn unit_harness_covers_open_path_pathbuf_ref_instantiation() {
        let temp_path = build_fixture_database();
        let path = temp_path.to_path_buf();
        let geocoder = Geocoder::open_path(&path).expect("open geocoder from pathbuf ref");
        let results = geocoder
            .country("US")
            .expect("country query from pathbuf-ref geocoder");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn unit_harness_covers_open_path_str_instantiation() {
        let path = build_fixture_database();
        let geocoder = Geocoder::open_path(path.to_str().expect("utf-8 fixture path"))
            .expect("open geocoder from string path");
        let results = geocoder
            .country("US")
            .expect("country query from string-path geocoder");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn unit_harness_covers_open_path_path_ref_instantiation() {
        let path = build_fixture_database();
        let path_ref = Path::new(path.to_str().expect("utf-8 fixture path"));
        let geocoder = Geocoder::open_path(path_ref).expect("open geocoder from path ref");
        let results = geocoder
            .country("US")
            .expect("country query from path-ref geocoder");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn unit_harness_covers_open_path_error_path() {
        let root = tempfile::tempdir().expect("temp dir");
        let missing_path = root.path().join("missing").join("fixture.sqlite");
        let open_path_err = Geocoder::open_path(&missing_path)
            .err()
            .expect("missing parent path should fail");
        assert_sqlite_error_contains(open_path_err, "");
    }

    #[test]
    fn unit_harness_covers_missing_schema_errors() {
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

        let country_center_err = geocoder
            .country_center("US")
            .expect_err("country_center should fail without schema");
        assert_sqlite_error_contains(country_center_err, "no such");
    }

    #[test]
    fn unit_harness_covers_row_mapping_errors() {
        let reverse_country = open_reverse_country_row_error_geocoder();
        let reverse_err = reverse_country
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

        let country_err = reverse_country
            .country("US")
            .expect_err("country should fail on invalid row mapping");
        assert_sqlite_error_contains(country_err, "Invalid column type");

        let country_list = open_country_list_row_error_geocoder();
        let country_list_err = country_list
            .country_list()
            .expect_err("country_list should fail on null aggregate row");
        assert_sqlite_error_contains(country_list_err, "Invalid column type");
    }

    #[test]
    fn unit_harness_covers_query_execution_error_paths() {
        let reverse_country = geocoder_with_reverse_country_query_execution_error();
        let reverse_err = reverse_country
            .reverse(
                GeocoderPoint { lat: 1.0, lng: 2.0 },
                Some(GeocoderReverseOptions {
                    limit: 1,
                    degree_offset: 1.0,
                }),
            )
            .expect_err("reverse should fail during query execution");
        assert_sqlite_error_contains(reverse_err, "no such function");

        let country_err = reverse_country
            .country("US")
            .expect_err("country should fail during query execution");
        assert_sqlite_error_contains(country_err, "no such function");

        let country_list_err = geocoder_with_country_list_query_execution_error()
            .country_list()
            .expect_err("country_list should fail during query execution");
        assert_sqlite_error_contains(country_list_err, "no such function");

        let country_center_err = geocoder_with_country_center_query_execution_error()
            .country_center("US")
            .expect_err("country_center should fail during query execution");
        assert_sqlite_error_contains(country_center_err, "no such function");
    }

    #[test]
    fn unit_harness_covers_country_list_field_error_paths() {
        let country_id_err =
            geocoder_with_country_list_sql_row("1", "'United States'", "37.0", "1.0")
                .country_list()
                .expect_err("country_id type mismatch should fail");
        assert_sqlite_error_contains(country_id_err, "Invalid column type");

        let country_name_err = geocoder_with_country_list_sql_row("'US'", "1", "37.0", "1.0")
            .country_list()
            .expect_err("country_name type mismatch should fail");
        assert_sqlite_error_contains(country_name_err, "Invalid column type");

        let longitude_err =
            geocoder_with_country_list_sql_row("'US'", "'United States'", "37.0", "NULL")
                .country_list()
                .expect_err("longitude type mismatch should fail");
        assert_sqlite_error_contains(longitude_err, "Invalid column type");
    }

    #[test]
    fn unit_harness_covers_country_center_row_error_paths() {
        let latitude_err = map_country_center_row_error("'bad'", "1.0");
        assert_sqlite_error_contains(GeocoderError::from(latitude_err), "Invalid column type");

        let longitude_err = map_country_center_row_error("1.0", "'bad'");
        assert_sqlite_error_contains(GeocoderError::from(longitude_err), "Invalid column type");
    }

    #[test]
    fn unit_harness_covers_reverse_row_field_error_paths() {
        for err in [
            map_reverse_row_error(
                "'bad'",
                "'name'",
                "1",
                "'admin'",
                "'US'",
                "'United States'",
                "1.0",
                "2.0",
            ),
            map_reverse_row_error(
                "1",
                "'name'",
                "'bad'",
                "'admin'",
                "'US'",
                "'United States'",
                "1.0",
                "2.0",
            ),
            map_reverse_row_error(
                "1",
                "'name'",
                "1",
                "1",
                "'US'",
                "'United States'",
                "1.0",
                "2.0",
            ),
            map_reverse_row_error(
                "1",
                "'name'",
                "1",
                "'admin'",
                "1",
                "'United States'",
                "1.0",
                "2.0",
            ),
            map_reverse_row_error("1", "'name'", "1", "'admin'", "'US'", "1", "1.0", "2.0"),
            map_reverse_row_error(
                "1",
                "'name'",
                "1",
                "'admin'",
                "'US'",
                "'United States'",
                "'bad'",
                "2.0",
            ),
            map_reverse_row_error(
                "1",
                "'name'",
                "1",
                "'admin'",
                "'US'",
                "'United States'",
                "1.0",
                "'bad'",
            ),
        ] {
            assert_sqlite_error_contains(GeocoderError::from(err), "Invalid column type");
        }
    }

    #[test]
    fn unit_harness_covers_country_center_not_found() {
        let geocoder = open_fixture_geocoder();
        let err = geocoder
            .country_center("ZZ")
            .expect_err("missing country should return not found");
        assert_country_center_not_found(err, "ZZ");
    }

    #[test]
    fn unit_harness_covers_helper_panic_paths() {
        let sqlite_panic = std::panic::catch_unwind(|| {
            assert_sqlite_error_contains(
                GeocoderError::CountryCenterNotFound {
                    country_id: "US".to_owned(),
                },
                "no such",
            );
        });
        assert!(sqlite_panic.is_err());

        let country_center_panic = std::panic::catch_unwind(|| {
            let mismatch_err = GeocoderError::Sqlite(rusqlite::Error::InvalidQuery);
            assert_country_center_not_found(mismatch_err, "US");
        });
        assert!(country_center_panic.is_err());
    }

    fn open_fixture_geocoder() -> Geocoder {
        let path = build_fixture_database();
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

    fn geocoder_with_reverse_country_query_execution_error() -> Geocoder {
        let conn = Connection::open_in_memory().expect("open in-memory query error db");
        conn.execute_batch(
            r#"
            CREATE VIEW geonames AS
              SELECT
                1 AS id,
                missing_reverse_name() AS name,
                1 AS admin1_id,
                'Admin' AS admin1_name,
                'US' AS country_id,
                'United States' AS country_name,
                1.0 AS latitude,
                2.0 AS longitude;
            CREATE TABLE coordinates(
              feature_id INTEGER,
              latitude REAL,
              longitude REAL
            );
            INSERT INTO coordinates (feature_id, latitude, longitude) VALUES (1, 1.0, 2.0);
            "#,
        )
        .expect("create reverse/country execution error schema");
        Geocoder { conn }
    }

    fn geocoder_with_country_list_query_execution_error() -> Geocoder {
        let conn =
            Connection::open_in_memory().expect("open in-memory country_list query error db");
        conn.execute_batch(
            r#"
            CREATE VIEW geonames AS
              SELECT
                'US' AS country_id,
                missing_country_name() AS country_name,
                1.0 AS latitude,
                2.0 AS longitude;
            "#,
        )
        .expect("create country_list execution error schema");
        Geocoder { conn }
    }

    fn geocoder_with_country_center_query_execution_error() -> Geocoder {
        let conn =
            Connection::open_in_memory().expect("open in-memory country_center query error db");
        conn.execute_batch(
            r#"
            CREATE VIEW geonames AS
              SELECT
                'US' AS country_id,
                missing_latitude() AS latitude,
                2.0 AS longitude;
            "#,
        )
        .expect("create country_center execution error schema");
        Geocoder { conn }
    }

    fn geocoder_with_country_list_sql_row(
        country_id_sql: &str,
        country_name_sql: &str,
        latitude_sql: &str,
        longitude_sql: &str,
    ) -> Geocoder {
        let conn =
            Connection::open_in_memory().expect("open in-memory country_list field error db");
        conn.execute_batch(&format!(
            r#"
            CREATE TABLE geonames(
              country_id,
              country_name,
              latitude,
              longitude
            );
            INSERT INTO geonames (country_id, country_name, latitude, longitude)
            VALUES ({country_id_sql}, {country_name_sql}, {latitude_sql}, {longitude_sql});
            "#,
        ))
        .expect("create country_list field error schema");
        Geocoder { conn }
    }

    fn map_country_center_row_error(latitude_sql: &str, longitude_sql: &str) -> rusqlite::Error {
        let conn =
            Connection::open_in_memory().expect("open in-memory country center row error db");
        conn.query_row(
            &format!("SELECT {latitude_sql} AS latitude_c, {longitude_sql} AS longitude_c"),
            [],
            map_country_center_row,
        )
        .expect_err("country center row decode should fail")
    }

    fn map_reverse_row_error(
        id_sql: &str,
        name_sql: &str,
        admin1_id_sql: &str,
        admin1_name_sql: &str,
        country_id_sql: &str,
        country_name_sql: &str,
        latitude_sql: &str,
        longitude_sql: &str,
    ) -> rusqlite::Error {
        let conn = Connection::open_in_memory().expect("open in-memory reverse row error db");
        conn.query_row(
            &format!(
                r#"
                SELECT
                  {id_sql} AS id,
                  {name_sql} AS name,
                  {admin1_id_sql} AS admin1_id,
                  {admin1_name_sql} AS admin1_name,
                  {country_id_sql} AS country_id,
                  {country_name_sql} AS country_name,
                  {latitude_sql} AS latitude,
                  {longitude_sql} AS longitude
                "#,
            ),
            [],
            map_reverse_row,
        )
        .expect_err("reverse row decode should fail")
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
}

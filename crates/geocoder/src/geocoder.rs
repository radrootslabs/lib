use crate::asset::{GeoNamesAssetSpec, validate_geonames_asset_file};
use crate::error::GeocoderError;
use crate::model::{
    GeocoderCountryListResult, GeocoderLocalityCandidate, GeocoderLocalityInput,
    GeocoderLocalityLookup, GeocoderLocalityQuery, GeocoderPoint, GeocoderReverseOptions,
    GeocoderReverseResult,
};
use rusqlite::{Connection, OpenFlags, named_params};
use std::io::Write;
use std::path::Path;

pub struct Geocoder {
    conn: Connection,
    _temp_path: Option<tempfile::TempPath>,
}

impl Geocoder {
    pub fn open_path<P: AsRef<Path>>(path: P) -> Result<Self, GeocoderError> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        Ok(Self {
            conn,
            _temp_path: None,
        })
    }

    pub fn open_bytes(bytes: &[u8]) -> Result<Self, GeocoderError> {
        let mut temp = tempfile::NamedTempFile::new()?;
        temp.as_file_mut().write_all(bytes)?;
        let temp_path = temp.into_temp_path();
        let path: &Path = temp_path.as_ref();
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        Ok(Self {
            conn,
            _temp_path: Some(temp_path),
        })
    }

    pub fn open_verified_geonames_asset<P: AsRef<Path>>(
        path: P,
        spec: &GeoNamesAssetSpec,
    ) -> Result<Self, GeocoderError> {
        validate_geonames_asset_file(path.as_ref(), spec)?;
        Self::open_path(path)
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

    pub fn locality(
        &self,
        query: &GeocoderLocalityQuery,
    ) -> Result<GeocoderLocalityLookup, GeocoderError> {
        match &query.input {
            GeocoderLocalityInput::Structured(structured) => self.locality_by_parts(
                &structured.locality,
                structured.region.as_deref(),
                structured.country.as_deref(),
                query.limit,
            ),
            GeocoderLocalityInput::Query(query_text) => {
                let parsed = parse_locality_query(query_text);
                self.locality_by_parts(
                    &parsed.locality,
                    parsed.region.as_deref(),
                    parsed.country.as_deref(),
                    query.limit,
                )
            }
            GeocoderLocalityInput::FeatureId(id) => self.locality_by_feature_id(*id),
        }
    }

    fn locality_by_parts(
        &self,
        locality: &str,
        region: Option<&str>,
        country: Option<&str>,
        limit: usize,
    ) -> Result<GeocoderLocalityLookup, GeocoderError> {
        let Some(locality) = normalize_optional_name(Some(locality)) else {
            return Ok(GeocoderLocalityLookup::NoMatch);
        };
        let country = normalize_optional_name(country);
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
            WHERE lower(name) = :locality
              AND (
                :country IS NULL
                OR lower(country_id) = :country
                OR lower(country_name) = :country
              )
            ORDER BY
              lower(name) ASC,
              lower(country_id) ASC,
              lower(coalesce(country_name, '')) ASC,
              lower(coalesce(admin1_name, '')) ASC,
              coalesce(admin1_id, -1) ASC,
              id ASC
            "#,
        )?;
        let candidates = collect_mapped_rows(
            &mut stmt,
            named_params! {
                ":locality": locality,
                ":country": country,
            },
            map_locality_candidate_row,
        )?;
        let region = normalize_optional_name(region);
        let candidates = candidates
            .into_iter()
            .filter(|candidate| locality_region_matches(candidate, region.as_deref()))
            .collect::<Vec<_>>();
        Ok(finalize_locality_lookup(candidates, limit))
    }

    fn locality_by_feature_id(&self, id: i64) -> Result<GeocoderLocalityLookup, GeocoderError> {
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
            WHERE id = :id
            LIMIT 1
            "#,
        )?;
        let candidates = collect_mapped_rows(
            &mut stmt,
            named_params! {
                ":id": id,
            },
            map_locality_candidate_row,
        )?;
        Ok(finalize_locality_lookup(candidates, 1))
    }
}

struct ParsedLocalityQuery {
    locality: String,
    region: Option<String>,
    country: Option<String>,
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
    let maybe_point = result?;
    if let Some(point) = maybe_point {
        return Ok(point);
    }
    Err(GeocoderError::CountryCenterNotFound {
        country_id: country_id.to_owned(),
    })
}

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

fn map_locality_candidate_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<GeocoderLocalityCandidate> {
    let name = row.get("name")?;
    let admin1_name = row.get("admin1_name")?;
    let country_name = row.get("country_name")?;
    let candidate = GeocoderLocalityCandidate {
        id: row.get("id")?,
        name,
        admin1_id: row.get("admin1_id")?,
        admin1_name,
        country_id: row.get("country_id")?,
        country_name,
        point: GeocoderPoint {
            lat: row.get("latitude")?,
            lng: row.get("longitude")?,
        },
        display_name: String::new(),
    };
    Ok(GeocoderLocalityCandidate {
        display_name: locality_candidate_display_name(&candidate),
        ..candidate
    })
}

fn parse_locality_query(query: &str) -> ParsedLocalityQuery {
    let parts = query
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    match parts.as_slice() {
        [] => ParsedLocalityQuery {
            locality: String::new(),
            region: None,
            country: None,
        },
        [locality] => ParsedLocalityQuery {
            locality: locality.clone(),
            region: None,
            country: None,
        },
        [locality, region] => ParsedLocalityQuery {
            locality: locality.clone(),
            region: Some(region.clone()),
            country: None,
        },
        parts => {
            let country = parts.last().cloned();
            let region = parts.get(parts.len().saturating_sub(2)).cloned();
            let locality = parts[..parts.len().saturating_sub(2)].join(", ");
            ParsedLocalityQuery {
                locality,
                region,
                country,
            }
        }
    }
}

fn finalize_locality_lookup(
    mut candidates: Vec<GeocoderLocalityCandidate>,
    limit: usize,
) -> GeocoderLocalityLookup {
    match candidates.len() {
        0 => GeocoderLocalityLookup::NoMatch,
        1 => GeocoderLocalityLookup::Unique {
            candidate: candidates.remove(0),
        },
        _ => {
            candidates.truncate(limit.max(1));
            GeocoderLocalityLookup::Ambiguous { candidates }
        }
    }
}

fn locality_region_matches(candidate: &GeocoderLocalityCandidate, region: Option<&str>) -> bool {
    let Some(region) = region else {
        return true;
    };
    let Some(admin1_name) = candidate.admin1_name.as_deref() else {
        return false;
    };
    if normalize_name(admin1_name) == region {
        return true;
    }
    let region_code = normalize_region_code(region);
    region_aliases(&candidate.country_id)
        .iter()
        .any(|(code, name)| {
            normalize_region_code(code) == region_code
                && normalize_name(name) == normalize_name(admin1_name)
        })
}

fn locality_candidate_display_name(candidate: &GeocoderLocalityCandidate) -> String {
    let mut parts = vec![candidate.name.clone()];
    if let Some(admin1_name) = candidate.admin1_name.as_ref() {
        parts.push(admin1_name.clone());
    }
    if let Some(country_name) = candidate.country_name.as_ref() {
        parts.push(country_name.clone());
    } else {
        parts.push(candidate.country_id.clone());
    }
    parts.join(", ")
}

fn normalize_optional_name(input: Option<&str>) -> Option<String> {
    input
        .map(normalize_name)
        .filter(|normalized| !normalized.is_empty())
}

fn normalize_name(input: &str) -> String {
    input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn normalize_region_code(input: &str) -> String {
    input
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_uppercase())
        .collect()
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
            ("IA", "Iowa"),
            ("ID", "Idaho"),
            ("IL", "Illinois"),
            ("IN", "Indiana"),
            ("KS", "Kansas"),
            ("KY", "Kentucky"),
            ("LA", "Louisiana"),
            ("MA", "Massachusetts"),
            ("MD", "Maryland"),
            ("ME", "Maine"),
            ("MI", "Michigan"),
            ("MN", "Minnesota"),
            ("MO", "Missouri"),
            ("MS", "Mississippi"),
            ("MT", "Montana"),
            ("NC", "North Carolina"),
            ("ND", "North Dakota"),
            ("NE", "Nebraska"),
            ("NH", "New Hampshire"),
            ("NJ", "New Jersey"),
            ("NM", "New Mexico"),
            ("NV", "Nevada"),
            ("NY", "New York"),
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
            ("VA", "Virginia"),
            ("VT", "Vermont"),
            ("WA", "Washington"),
            ("WI", "Wisconsin"),
            ("WV", "West Virginia"),
            ("WY", "Wyoming"),
        ],
        _ => &[],
    }
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
    fn unit_harness_covers_forward_locality_lookup_modes() {
        let geocoder = open_forward_fixture_geocoder();

        let british_columbia = geocoder
            .locality(
                &GeocoderLocalityQuery::structured("Fixture Victoria")
                    .with_region("BC")
                    .with_country("CA"),
            )
            .expect("structured locality lookup");
        assert_unique_locality(
            british_columbia,
            3001,
            "Fixture Victoria, British Columbia, Canada",
        );

        let country_name = geocoder
            .locality(
                &GeocoderLocalityQuery::structured("Fixture Victoria")
                    .with_region("British Columbia")
                    .with_country("Canada"),
            )
            .expect("structured country-name locality lookup");
        assert_unique_locality(
            country_name,
            3001,
            "Fixture Victoria, British Columbia, Canada",
        );

        let freeform = geocoder
            .locality(&GeocoderLocalityQuery::query("Fixture Victoria, BC, CA"))
            .expect("freeform locality lookup");
        assert_unique_locality(freeform, 3001, "Fixture Victoria, British Columbia, Canada");

        let narrowed = geocoder
            .locality(
                &GeocoderLocalityQuery::structured("Shared Market")
                    .with_region("Prairie Region")
                    .with_country("CA"),
            )
            .expect("region-narrowed locality lookup");
        assert_unique_locality(narrowed, 3003, "Shared Market, Prairie Region, Canada");

        let selected = geocoder
            .locality(&GeocoderLocalityQuery::feature_id(3004))
            .expect("feature-id locality lookup");
        assert_unique_locality(selected, 3004, "Identifier Grove, British Columbia, Canada");

        let no_match = geocoder
            .locality(
                &GeocoderLocalityQuery::structured("Missing Market")
                    .with_region("BC")
                    .with_country("CA"),
            )
            .expect("no-match locality lookup");
        assert!(matches!(no_match, GeocoderLocalityLookup::NoMatch));
    }

    #[test]
    fn unit_harness_covers_forward_locality_ambiguity_and_limits() {
        let geocoder = open_forward_fixture_geocoder();

        let ambiguous = geocoder
            .locality(
                &GeocoderLocalityQuery::structured("Shared Market")
                    .with_country("CA")
                    .with_limit(1),
            )
            .expect("ambiguous locality lookup");

        let GeocoderLocalityLookup::Ambiguous { candidates } = ambiguous else {
            panic!("expected ambiguous locality lookup");
        };
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, 3002);
        assert_eq!(
            candidates[0].display_name,
            "Shared Market, British Columbia, Canada"
        );
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

    fn open_forward_fixture_geocoder() -> Geocoder {
        let path = build_forward_fixture_database();
        Geocoder::open_path(&path).expect("open forward geocoder")
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
        Geocoder {
            conn,
            _temp_path: None,
        }
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
        Geocoder {
            conn,
            _temp_path: None,
        }
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
        Geocoder {
            conn,
            _temp_path: None,
        }
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
        Geocoder {
            conn,
            _temp_path: None,
        }
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

    fn seed_forward_fixture_database(path: &str) {
        let conn = Connection::open(path).expect("open fixture database");
        seed_schema(&conn);

        insert_country(&conn, "CA", "Canada");
        insert_country(&conn, "US", "United States");

        insert_admin1(&conn, "CA", 2, "British Columbia");
        insert_admin1(&conn, "CA", 3, "Prairie Region");
        insert_admin1(&conn, "US", 4, "River Region");

        insert_feature(
            &conn,
            3001,
            "Fixture Victoria",
            "CA",
            2,
            48.4359,
            -123.35155,
        );
        insert_feature(&conn, 3002, "Shared Market", "CA", 2, 48.7, -123.2);
        insert_feature(&conn, 3003, "Shared Market", "CA", 3, 50.2, -110.4);
        insert_feature(&conn, 3004, "Identifier Grove", "CA", 2, 48.9, -123.4);
        insert_feature(&conn, 3005, "Query Hamlet", "US", 4, 39.25, -77.5);
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

    fn assert_unique_locality(
        lookup: GeocoderLocalityLookup,
        expected_id: i64,
        expected_display_name: &str,
    ) {
        let GeocoderLocalityLookup::Unique { candidate } = lookup else {
            panic!("expected unique locality lookup");
        };
        assert_eq!(candidate.id, expected_id);
        assert_eq!(candidate.display_name, expected_display_name);
    }
}

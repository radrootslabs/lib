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
    pub fn open_path<P: AsRef<Path>>(path: P) -> Result<Self, GeocoderError> {
        let conn = Connection::open(path)?;
        Ok(Self { conn })
    }

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
        let rows = stmt.query_map(
            named_params! {
                ":lat": point.lat,
                ":lng": point.lng,
                ":degree_offset": options.degree_offset,
                ":lng_weight": lng_weight,
                ":limit": options.limit as i64,
            },
            map_reverse_row,
        )?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(GeocoderError::from)
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
        let rows = stmt.query_map(
            named_params! {
                ":country_id": country_id,
            },
            map_reverse_row,
        )?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(GeocoderError::from)
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
        let rows = stmt.query_map([], |row| {
            Ok(GeocoderCountryListResult {
                country_id: row.get("country_id")?,
                country: row.get("country_name")?,
                lat: row.get("latitude_c")?,
                lng: row.get("longitude_c")?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(GeocoderError::from)
    }

    pub fn country_center(&self, country_id: &str) -> Result<GeocoderPoint, GeocoderError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
              AVG(latitude) AS latitude_c,
              AVG(longitude) AS longitude_c
            FROM geonames
            WHERE country_id = :country_id
            "#,
        )?;
        let mut rows = stmt.query(named_params! {
            ":country_id": country_id,
        })?;
        let Some(row) = rows.next()? else {
            return Err(GeocoderError::CountryCenterNotFound {
                country_id: country_id.to_owned(),
            });
        };
        let lat: Option<f64> = row.get("latitude_c")?;
        let lng: Option<f64> = row.get("longitude_c")?;
        match (lat, lng) {
            (Some(lat), Some(lng)) => Ok(GeocoderPoint { lat, lng }),
            _ => Err(GeocoderError::CountryCenterNotFound {
                country_id: country_id.to_owned(),
            }),
        }
    }
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

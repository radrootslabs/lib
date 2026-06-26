#![cfg(feature = "runtime")]

use crate::RadrootsSdkError;
use radroots_events::ids::RadrootsAddressableCoordinate;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;

pub(crate) const SDK_PRIVATE_STORE_SCHEMA_VERSION: i64 = 1;

const PRIVATE_STORE_MIGRATION_UP: &str = r#"
CREATE TABLE IF NOT EXISTS sdk_private_farm_location (
  farm_addr TEXT PRIMARY KEY NOT NULL,
  farm_pubkey TEXT NOT NULL,
  farm_d_tag TEXT NOT NULL,
  latitude REAL NOT NULL CHECK(latitude >= -90.0 AND latitude <= 90.0),
  longitude REAL NOT NULL CHECK(longitude >= -180.0 AND longitude <= 180.0),
  locality_primary TEXT NOT NULL,
  locality_city TEXT,
  locality_region TEXT,
  locality_country TEXT,
  geohash5 TEXT NOT NULL CHECK(length(geohash5) = 5),
  geonames_feature_id INTEGER,
  geonames_country_id TEXT,
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS sdk_private_farm_location_pubkey
  ON sdk_private_farm_location (farm_pubkey, farm_d_tag);
"#;

#[derive(Clone)]
pub(crate) struct SdkPrivateStore {
    pool: SqlitePool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SdkPrivateFarmLocationRecord {
    pub farm_addr: RadrootsAddressableCoordinate,
    pub farm_pubkey: String,
    pub farm_d_tag: String,
    pub latitude: f64,
    pub longitude: f64,
    pub locality_primary: String,
    pub locality_city: Option<String>,
    pub locality_region: Option<String>,
    pub locality_country: Option<String>,
    pub geohash5: String,
    pub geonames_feature_id: Option<i64>,
    pub geonames_country_id: Option<String>,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SdkPrivateStoreStatusSummary {
    pub farm_private_locations: i64,
}

impl SdkPrivateStore {
    pub async fn open_memory() -> Result<Self, RadrootsSdkError> {
        let options =
            SqliteConnectOptions::from_str("sqlite::memory:").map_err(private_store_error)?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(private_store_error)?;
        configure_connection(&pool, false).await?;
        apply_up(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn open_file(path: impl AsRef<Path>) -> Result<Self, RadrootsSdkError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(private_store_error)?;
        configure_connection(&pool, true).await?;
        apply_up(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn pragma_foreign_keys(&self) -> Result<i64, RadrootsSdkError> {
        query_i64(&self.pool, "PRAGMA foreign_keys").await
    }

    pub async fn pragma_busy_timeout(&self) -> Result<i64, RadrootsSdkError> {
        query_i64(&self.pool, "PRAGMA busy_timeout").await
    }

    pub async fn pragma_journal_mode(&self) -> Result<String, RadrootsSdkError> {
        query_string(&self.pool, "PRAGMA journal_mode").await
    }

    pub async fn status_summary(&self) -> Result<SdkPrivateStoreStatusSummary, RadrootsSdkError> {
        Ok(SdkPrivateStoreStatusSummary {
            farm_private_locations: query_i64(
                &self.pool,
                "SELECT COUNT(*) FROM sdk_private_farm_location",
            )
            .await?,
        })
    }

    pub async fn upsert_farm_location(
        &self,
        record: &SdkPrivateFarmLocationRecord,
    ) -> Result<(), RadrootsSdkError> {
        validate_location_record(record)?;
        sqlx::query(
            r#"
            INSERT INTO sdk_private_farm_location (
              farm_addr,
              farm_pubkey,
              farm_d_tag,
              latitude,
              longitude,
              locality_primary,
              locality_city,
              locality_region,
              locality_country,
              geohash5,
              geonames_feature_id,
              geonames_country_id,
              updated_at_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(farm_addr) DO UPDATE SET
              farm_pubkey = excluded.farm_pubkey,
              farm_d_tag = excluded.farm_d_tag,
              latitude = excluded.latitude,
              longitude = excluded.longitude,
              locality_primary = excluded.locality_primary,
              locality_city = excluded.locality_city,
              locality_region = excluded.locality_region,
              locality_country = excluded.locality_country,
              geohash5 = excluded.geohash5,
              geonames_feature_id = excluded.geonames_feature_id,
              geonames_country_id = excluded.geonames_country_id,
              updated_at_ms = excluded.updated_at_ms
            "#,
        )
        .bind(record.farm_addr.as_str())
        .bind(record.farm_pubkey.as_str())
        .bind(record.farm_d_tag.as_str())
        .bind(record.latitude)
        .bind(record.longitude)
        .bind(record.locality_primary.as_str())
        .bind(record.locality_city.as_deref())
        .bind(record.locality_region.as_deref())
        .bind(record.locality_country.as_deref())
        .bind(record.geohash5.as_str())
        .bind(record.geonames_feature_id)
        .bind(record.geonames_country_id.as_deref())
        .bind(record.updated_at_ms)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(private_store_error)
    }

    pub async fn farm_location(
        &self,
        farm_addr: &RadrootsAddressableCoordinate,
    ) -> Result<Option<SdkPrivateFarmLocationRecord>, RadrootsSdkError> {
        let row = sqlx::query(
            r#"
            SELECT
              farm_addr,
              farm_pubkey,
              farm_d_tag,
              latitude,
              longitude,
              locality_primary,
              locality_city,
              locality_region,
              locality_country,
              geohash5,
              geonames_feature_id,
              geonames_country_id,
              updated_at_ms
            FROM sdk_private_farm_location
            WHERE farm_addr = ?1
            "#,
        )
        .bind(farm_addr.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(private_store_error)?;
        row.map(|row| private_farm_location_from_row(farm_addr.clone(), row))
            .transpose()
    }
}

async fn configure_connection(
    pool: &SqlitePool,
    file_backed: bool,
) -> Result<(), RadrootsSdkError> {
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(pool)
        .await
        .map_err(private_store_error)?;
    sqlx::query("PRAGMA busy_timeout = 5000")
        .execute(pool)
        .await
        .map_err(private_store_error)?;
    if file_backed {
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(pool)
            .await
            .map_err(private_store_error)?;
    }
    Ok(())
}

async fn apply_up(pool: &SqlitePool) -> Result<(), RadrootsSdkError> {
    sqlx::raw_sql(PRIVATE_STORE_MIGRATION_UP)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(private_store_error)
}

async fn query_i64(pool: &SqlitePool, sql: &str) -> Result<i64, RadrootsSdkError> {
    let row = sqlx::query(sql)
        .fetch_one(pool)
        .await
        .map_err(private_store_error)?;
    row.try_get(0).map_err(private_store_error)
}

async fn query_string(pool: &SqlitePool, sql: &str) -> Result<String, RadrootsSdkError> {
    let row = sqlx::query(sql)
        .fetch_one(pool)
        .await
        .map_err(private_store_error)?;
    row.try_get(0).map_err(private_store_error)
}

fn private_farm_location_from_row(
    farm_addr: RadrootsAddressableCoordinate,
    row: sqlx::sqlite::SqliteRow,
) -> Result<SdkPrivateFarmLocationRecord, RadrootsSdkError> {
    Ok(SdkPrivateFarmLocationRecord {
        farm_addr,
        farm_pubkey: row.try_get("farm_pubkey").map_err(private_store_error)?,
        farm_d_tag: row.try_get("farm_d_tag").map_err(private_store_error)?,
        latitude: row.try_get("latitude").map_err(private_store_error)?,
        longitude: row.try_get("longitude").map_err(private_store_error)?,
        locality_primary: row
            .try_get("locality_primary")
            .map_err(private_store_error)?,
        locality_city: row.try_get("locality_city").map_err(private_store_error)?,
        locality_region: row
            .try_get("locality_region")
            .map_err(private_store_error)?,
        locality_country: row
            .try_get("locality_country")
            .map_err(private_store_error)?,
        geohash5: row.try_get("geohash5").map_err(private_store_error)?,
        geonames_feature_id: row
            .try_get("geonames_feature_id")
            .map_err(private_store_error)?,
        geonames_country_id: row
            .try_get("geonames_country_id")
            .map_err(private_store_error)?,
        updated_at_ms: row.try_get("updated_at_ms").map_err(private_store_error)?,
    })
}

fn validate_location_record(record: &SdkPrivateFarmLocationRecord) -> Result<(), RadrootsSdkError> {
    if !record.latitude.is_finite()
        || !record.longitude.is_finite()
        || record.latitude < -90.0
        || record.latitude > 90.0
        || record.longitude < -180.0
        || record.longitude > 180.0
    {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "farm exact location coordinates are outside valid latitude/longitude bounds"
                .to_owned(),
        });
    }
    if record.locality_primary.trim().is_empty() {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "farm public locality primary name must not be empty".to_owned(),
        });
    }
    if record.geohash5.len() != 5 {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "farm public locality geohash must be precision 5".to_owned(),
        });
    }
    Ok(())
}

fn private_store_error(error: impl ToString) -> RadrootsSdkError {
    RadrootsSdkError::PrivateStore {
        message: error.to_string(),
    }
}

#[cfg(test)]
#[path = "../tests/unit/private_store_tests.rs"]
mod tests;

#![cfg(feature = "runtime")]

use crate::RadrootsSdkError;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;

pub(crate) const SDK_STUDIO_STORE_SCHEMA_VERSION: i64 = 1;

const STUDIO_STORE_MIGRATION_UP: &str = r#"
CREATE TABLE IF NOT EXISTS sdk_studio_state (
  key TEXT PRIMARY KEY NOT NULL,
  value_json TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL
);
"#;

#[derive(Clone)]
pub(crate) struct SdkStudioStore {
    pool: SqlitePool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SdkStudioStoreStatusSummary {
    pub studio_state_records: i64,
}

impl SdkStudioStore {
    pub async fn open_memory() -> Result<Self, RadrootsSdkError> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").map_err(studio_error)?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(studio_error)?;
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
            .map_err(studio_error)?;
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

    pub async fn status_summary(&self) -> Result<SdkStudioStoreStatusSummary, RadrootsSdkError> {
        Ok(SdkStudioStoreStatusSummary {
            studio_state_records: query_i64(&self.pool, "SELECT COUNT(*) FROM sdk_studio_state")
                .await?,
        })
    }
}

async fn configure_connection(
    pool: &SqlitePool,
    file_backed: bool,
) -> Result<(), RadrootsSdkError> {
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(pool)
        .await
        .map_err(studio_error)?;
    sqlx::query("PRAGMA busy_timeout = 5000")
        .execute(pool)
        .await
        .map_err(studio_error)?;
    if file_backed {
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(pool)
            .await
            .map_err(studio_error)?;
    }
    Ok(())
}

async fn apply_up(pool: &SqlitePool) -> Result<(), RadrootsSdkError> {
    sqlx::raw_sql(STUDIO_STORE_MIGRATION_UP)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(studio_error)
}

async fn query_i64(pool: &SqlitePool, sql: &'static str) -> Result<i64, RadrootsSdkError> {
    let row = sqlx::query(sql)
        .fetch_one(pool)
        .await
        .map_err(studio_error)?;
    row.try_get(0).map_err(studio_error)
}

async fn query_string(pool: &SqlitePool, sql: &'static str) -> Result<String, RadrootsSdkError> {
    let row = sqlx::query(sql)
        .fetch_one(pool)
        .await
        .map_err(studio_error)?;
    row.try_get(0).map_err(studio_error)
}

fn studio_error(error: impl std::fmt::Display) -> RadrootsSdkError {
    RadrootsSdkError::StudioStore {
        message: error.to_string(),
    }
}

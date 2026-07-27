use sqlx::SqlitePool;

pub struct StorageHandle {
    pub pool: SqlitePool,
}

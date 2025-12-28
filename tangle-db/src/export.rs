use radroots_sql_core::{SqlExecutor, error::SqlError, utils};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::backup::{
    DATABASE_BACKUP_VERSION,
    TANGLE_DB_VERSION,
    MigrationBackup,
    SchemaEntry,
    escape_identifier,
    export_migrations,
    load_schema,
};

pub const TANGLE_DB_EXPORT_VERSION: &str = "1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableCount {
    pub name: String,
    pub row_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TangleDbExportManifestRs {
    pub export_version: String,
    pub tangle_db_version: String,
    pub backup_format_version: String,
    pub schema_hash: String,
    pub schema: Vec<SchemaEntry>,
    pub migrations: Vec<MigrationBackup>,
    pub table_counts: Vec<TableCount>,
}

pub fn export_manifest<E: SqlExecutor>(executor: &E) -> Result<TangleDbExportManifestRs, SqlError> {
    let schema = load_schema(executor)?;
    let migrations = export_migrations();
    let table_counts = load_table_counts(executor, &schema)?;
    let schema_hash = schema_hash(&schema)?;
    Ok(TangleDbExportManifestRs {
        export_version: TANGLE_DB_EXPORT_VERSION.to_string(),
        tangle_db_version: TANGLE_DB_VERSION.to_string(),
        backup_format_version: DATABASE_BACKUP_VERSION.to_string(),
        schema_hash,
        schema,
        migrations,
        table_counts,
    })
}

fn load_table_counts<E: SqlExecutor>(
    executor: &E,
    schema: &[SchemaEntry],
) -> Result<Vec<TableCount>, SqlError> {
    #[derive(Deserialize)]
    struct CountRow {
        count: u64,
    }
    let mut counts = Vec::new();
    for entry in schema.iter().filter(|s| s.object_type == "table") {
        let sql = format!(
            "select count(1) as count from {}",
            escape_identifier(&entry.name)
        );
        let json = executor.query_raw(&sql, "[]")?;
        let rows: Vec<CountRow> = utils::parse_json(&json)?;
        let row_count = rows.first().map(|row| row.count).unwrap_or(0);
        counts.push(TableCount {
            name: entry.name.clone(),
            row_count,
        });
    }
    Ok(counts)
}

fn schema_hash(schema: &[SchemaEntry]) -> Result<String, SqlError> {
    let json = serde_json::to_string(schema).map_err(SqlError::from)?;
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

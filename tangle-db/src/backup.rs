use radroots_sql_core::{SqlExecutor, error::SqlError, utils};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};

pub const DATABASE_BACKUP_VERSION: &str = "1.0.0";
pub const TANGLE_DB_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaEntry {
    pub object_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sql: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableData {
    pub name: String,
    pub rows: Vec<Map<String, Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationBackup {
    pub name: String,
    pub up_sql: String,
    pub down_sql: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseBackup {
    pub format_version: String,
    pub tangle_db_version: String,
    pub schema: Vec<SchemaEntry>,
    pub migrations: Vec<MigrationBackup>,
    pub data: Vec<TableData>,
}

pub fn export_database_backup<E: SqlExecutor>(executor: &E) -> Result<DatabaseBackup, SqlError> {
    let schema = load_schema(executor)?;
    let data = read_tables_for_backup(executor, &schema)?;
    let migrations = export_migrations();
    Ok(DatabaseBackup {
        format_version: DATABASE_BACKUP_VERSION.to_string(),
        tangle_db_version: TANGLE_DB_VERSION.to_string(),
        schema,
        migrations,
        data,
    })
}

pub fn export_database_backup_json<E: SqlExecutor>(executor: &E) -> Result<String, SqlError> {
    let backup = export_database_backup(executor)?;
    serde_json::to_string(&backup).map_err(SqlError::from)
}

pub fn restore_database_backup<E: SqlExecutor>(
    executor: &E,
    backup: &DatabaseBackup,
) -> Result<(), SqlError> {
    validate_backup_version(backup)?;
    executor.exec("PRAGMA foreign_keys = OFF;", "[]")?;
    executor.begin()?;
    let result = (|| {
        drop_existing_objects(executor)?;
        create_schema_from_backup(executor, &backup.schema)?;
        insert_rows_from_backup(executor, backup)?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            executor.commit()?;
            let _ = executor.exec("PRAGMA foreign_keys = ON;", "[]")?;
            Ok(())
        }
        Err(err) => {
            let _ = executor.rollback();
            let _ = executor.exec("PRAGMA foreign_keys = ON;", "[]");
            Err(err)
        }
    }
}

pub fn restore_database_backup_json<E: SqlExecutor>(
    executor: &E,
    backup_json: &str,
) -> Result<(), SqlError> {
    let backup: DatabaseBackup = serde_json::from_str(backup_json).map_err(SqlError::from)?;
    restore_database_backup(executor, &backup)
}

fn drop_existing_objects<E: SqlExecutor>(executor: &E) -> Result<(), SqlError> {
    #[derive(Deserialize)]
    struct MasterRow {
        #[serde(rename = "type")]
        object_type: Option<String>,
        name: Option<String>,
    }
    let json = executor.query_raw(
        "select type, name from sqlite_master where name not like 'sqlite_%'",
        "[]",
    )?;
    let rows: Vec<MasterRow> = utils::parse_json(&json)?;

    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows.into_iter() {
        let obj_type = row.object_type.unwrap_or_default();
        let name = match row.name {
            Some(n) => n,
            None => continue,
        };
        groups.entry(obj_type).or_default().push(name);
    }

    for object_type in ["trigger", "view", "index", "table"] {
        if let Some(names) = groups.get(object_type) {
            for name in names {
                let stmt = match object_type {
                    "trigger" => format!("DROP TRIGGER IF EXISTS {};", escape_identifier(name)),
                    "view" => format!("DROP VIEW IF EXISTS {};", escape_identifier(name)),
                    "index" => format!("DROP INDEX IF EXISTS {};", escape_identifier(name)),
                    _ => format!("DROP TABLE IF EXISTS {};", escape_identifier(name)),
                };
                let _ = executor.exec(&stmt, "[]")?;
            }
        }
    }
    Ok(())
}

fn create_schema_from_backup<E: SqlExecutor>(
    executor: &E,
    schema: &[SchemaEntry],
) -> Result<(), SqlError> {
    for entry in schema.iter().filter(|s| s.object_type == "table") {
        if let Some(sql) = &entry.sql {
            executor.exec(sql, "[]")?;
        }
    }
    for entry in schema
        .iter()
        .filter(|s| s.object_type != "table" && s.sql.is_some())
    {
        if let Some(sql) = &entry.sql {
            executor.exec(sql, "[]")?;
        }
    }
    Ok(())
}

fn insert_rows_from_backup<E: SqlExecutor>(
    executor: &E,
    backup: &DatabaseBackup,
) -> Result<(), SqlError> {
    let mut row_sources: HashMap<&str, &Vec<Map<String, Value>>> = HashMap::new();
    for table in &backup.data {
        row_sources.insert(table.name.as_str(), &table.rows);
    }
    for entry in backup.schema.iter().filter(|s| s.object_type == "table") {
        let rows = match row_sources.get(entry.name.as_str()) {
            Some(r) => *r,
            None => continue,
        };
        for row in rows {
            insert_row(executor, &entry.name, row)?;
        }
    }
    Ok(())
}

fn insert_row<E: SqlExecutor>(
    executor: &E,
    table: &str,
    row: &Map<String, Value>,
) -> Result<(), SqlError> {
    if row.is_empty() {
        return Ok(());
    }

    let mut cols: BTreeMap<String, &Value> = BTreeMap::new();
    for (k, v) in row {
        cols.insert(k.clone(), v);
    }

    let column_names: Vec<String> = cols.keys().cloned().collect();
    let placeholders = (0..column_names.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({});",
        escape_identifier(table),
        column_names
            .iter()
            .map(|c| escape_identifier(c))
            .collect::<Vec<_>>()
            .join(","),
        placeholders
    );

    let binds: Vec<Value> = cols.values().map(|v| utils::to_db_bind_value(*v)).collect();
    let params_json = serde_json::to_string(&binds).map_err(SqlError::from)?;
    executor.exec(&sql, &params_json)?;
    Ok(())
}

pub(crate) fn load_schema<E: SqlExecutor>(executor: &E) -> Result<Vec<SchemaEntry>, SqlError> {
    let json = executor.query_raw(
        "select type, name, tbl_name as table_name, sql from sqlite_master where name not like 'sqlite_%' order by type, name",
        "[]",
    )?;
    #[derive(Deserialize)]
    struct RawSchema {
        #[serde(rename = "type")]
        object_type: Option<String>,
        name: Option<String>,
        table_name: Option<String>,
        sql: Option<String>,
    }
    let rows: Vec<RawSchema> = utils::parse_json(&json)?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let name = row.name?;
            let object_type = row.object_type.unwrap_or_default();
            Some(SchemaEntry {
                object_type,
                name,
                table_name: row.table_name,
                sql: row.sql,
            })
        })
        .collect())
}

pub(crate) fn export_migrations() -> Vec<MigrationBackup> {
    crate::migrations::MIGRATIONS
        .iter()
        .map(|m| MigrationBackup {
            name: m.name.to_string(),
            up_sql: m.up_sql.to_string(),
            down_sql: m.down_sql.to_string(),
        })
        .collect()
}

fn read_tables_for_backup<E: SqlExecutor>(
    executor: &E,
    schema: &[SchemaEntry],
) -> Result<Vec<TableData>, SqlError> {
    let mut data = Vec::new();
    for entry in schema.iter().filter(|s| s.object_type == "table") {
        let select_sql = format!("SELECT * FROM {};", escape_identifier(&entry.name));
        let json = executor.query_raw(&select_sql, "[]")?;
        let rows: Vec<Map<String, Value>> = utils::parse_json(&json)?;
        data.push(TableData {
            name: entry.name.clone(),
            rows,
        });
    }
    Ok(data)
}

pub(crate) fn escape_identifier(name: &str) -> String {
    let mut escaped = String::with_capacity(name.len() + 2);
    escaped.push('"');
    for c in name.chars() {
        if c == '"' {
            escaped.push('"');
        }
        escaped.push(c);
    }
    escaped.push('"');
    escaped
}

fn validate_backup_version(backup: &DatabaseBackup) -> Result<(), SqlError> {
    if backup.format_version != DATABASE_BACKUP_VERSION {
        return Err(SqlError::InvalidArgument(format!(
            "unsupported backup format {}, expected {}",
            backup.format_version, DATABASE_BACKUP_VERSION
        )));
    }
    if backup.tangle_db_version != TANGLE_DB_VERSION {
        return Err(SqlError::InvalidArgument(format!(
            "unsupported tangle-db version {}, expected {}",
            backup.tangle_db_version, TANGLE_DB_VERSION
        )));
    }
    Ok(())
}

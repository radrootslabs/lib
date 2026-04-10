use crate::SqlExecutor;
use crate::error::SqlError;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug)]
pub struct Migration {
    pub name: &'static str,
    pub up_sql: &'static str,
    pub down_sql: &'static str,
}

pub fn migrations_run_all_up<E>(executor: &E, migrations: &[Migration]) -> Result<(), SqlError>
where
    E: SqlExecutor,
{
    ensure_table(executor)?;
    for migration in migrations {
        if !is_applied(executor, migration.name)? {
            executor.begin()?;
            let result = (|| -> Result<(), SqlError> {
                let _ = executor.exec(migration.up_sql, "[]")?;
                mark_applied(executor, migration.name)?;
                Ok(())
            })();
            match result {
                Ok(()) => {
                    executor.commit()?;
                }
                Err(err) => {
                    let _ = executor.rollback();
                    return Err(err);
                }
            }
        }
    }
    Ok(())
}

pub fn migrations_run_all_down<E>(executor: &E, migrations: &[Migration]) -> Result<(), SqlError>
where
    E: SqlExecutor,
{
    ensure_table(executor)?;
    executor.begin()?;
    for migration in migrations.iter().rev() {
        let params = json!([migration.name]).to_string();
        let _ = executor.exec("delete from __migrations where name = ?", &params)?;
        let _ = executor.exec(migration.down_sql, "[]")?;
    }
    executor.commit()?;
    Ok(())
}

fn ensure_table<E>(executor: &E) -> Result<(), SqlError>
where
    E: SqlExecutor,
{
    let _ = executor.exec("create table if not exists __migrations(id integer primary key, name text not null unique, applied_at text not null default (datetime('now')))", "[]")?;
    Ok(())
}

fn is_applied<E>(executor: &E, name: &str) -> Result<bool, SqlError>
where
    E: SqlExecutor,
{
    let params = json!([name]).to_string();
    let sql = "select 1 as applied from __migrations where name = ? limit 1";
    let json = executor.query_raw(sql, &params)?;
    if json.trim().is_empty() {
        return Ok(false);
    }
    let rows: Vec<Value> = serde_json::from_str(&json)?;
    Ok(!rows.is_empty())
}

fn mark_applied<E>(executor: &E, name: &str) -> Result<(), SqlError>
where
    E: SqlExecutor,
{
    let params = json!([name]).to_string();
    let sql = "insert or ignore into __migrations(name) values(?)";
    let _ = executor.exec(sql, &params)?;
    Ok(())
}

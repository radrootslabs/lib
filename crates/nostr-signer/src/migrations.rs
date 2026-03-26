#[cfg(feature = "native")]
use radroots_sql_core::SqlExecutor;
#[cfg(feature = "native")]
use radroots_sql_core::error::SqlError;
#[cfg(feature = "native")]
use radroots_sql_core::migrations::{Migration, migrations_run_all_down, migrations_run_all_up};

#[cfg(feature = "native")]
pub static MIGRATIONS: &[Migration] = &[Migration {
    name: "0000_init",
    up_sql: include_str!("../migrations/0000_init.up.sql"),
    down_sql: include_str!("../migrations/0000_init.down.sql"),
}];

#[cfg(feature = "native")]
pub fn run_all_up<E>(executor: &E) -> Result<(), SqlError>
where
    E: SqlExecutor,
{
    migrations_run_all_up(executor, MIGRATIONS)
}

#[cfg(feature = "native")]
pub fn run_all_down<E>(executor: &E) -> Result<(), SqlError>
where
    E: SqlExecutor,
{
    migrations_run_all_down(executor, MIGRATIONS)
}

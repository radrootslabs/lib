#![forbid(unsafe_code)]

use radroots_sql_core::SqlExecutor;
use radroots_sql_core::error::SqlError;
use radroots_sql_core::migrations::{Migration, migrations_run_all_down, migrations_run_all_up};

pub static MIGRATIONS: &[Migration] = &[
    Migration {
        name: "0000_local_events",
        up_sql: include_str!("../migrations/0000_local_events.up.sql"),
        down_sql: include_str!("../migrations/0000_local_events.down.sql"),
    },
    Migration {
        name: "0001_change_tracking",
        up_sql: include_str!("../migrations/0001_change_tracking.up.sql"),
        down_sql: include_str!("../migrations/0001_change_tracking.down.sql"),
    },
];

pub fn run_all_up<E>(executor: &E) -> Result<(), SqlError>
where
    E: SqlExecutor,
{
    migrations_run_all_up(executor, MIGRATIONS)
}

pub fn run_all_down<E>(executor: &E) -> Result<(), SqlError>
where
    E: SqlExecutor,
{
    migrations_run_all_down(executor, MIGRATIONS)
}

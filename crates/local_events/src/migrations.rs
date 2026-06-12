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
    Migration {
        name: "0002_network_source_runtime",
        up_sql: include_str!("../migrations/0002_network_source_runtime.up.sql"),
        down_sql: include_str!("../migrations/0002_network_source_runtime.down.sql"),
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

#[cfg(test)]
mod tests {
    use radroots_sql_core::SqliteExecutor;

    use super::*;

    #[test]
    fn migration_entrypoints_apply_and_reverse_schema() {
        let executor = SqliteExecutor::open_memory().expect("open memory sqlite");

        run_all_up(&executor).expect("migrate up");
        executor
            .query_raw("select name from __migrations order by name", "[]")
            .expect("query migrations");
        run_all_down(&executor).expect("migrate down");
    }
}

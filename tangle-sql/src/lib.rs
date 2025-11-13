pub use radroots_sql_core::error::SqlError;
pub use radroots_sql_core::{ExecOutcome, SqlExecutor};

pub mod migrations;
pub mod tables;
use radroots_types::types::IError;
pub use tables::log_error;

pub struct TangleSql<E: SqlExecutor> {
    executor: E,
}

impl<E: SqlExecutor> TangleSql<E> {
    pub fn new(executor: E) -> Self {
        Self { executor }
    }

    pub fn executor(&self) -> &E {
        &self.executor
    }

    pub fn migrate_up(&self) -> Result<(), SqlError> {
        crate::migrations::run_all_up(self.executor())
    }

    pub fn migrate_down(&self) -> Result<(), SqlError> {
        crate::migrations::run_all_down(self.executor())
    }

    pub fn log_error_create(
        &self,
        opts: &radroots_tangle_schema::log_error::ILogErrorCreate,
    ) -> Result<radroots_tangle_schema::log_error::ILogErrorCreateResolve, IError<SqlError>> {
        tables::log_error::create(self.executor(), opts)
    }

    pub fn log_error_find_many(
        &self,
        opts: &radroots_tangle_schema::log_error::ILogErrorFindMany,
    ) -> Result<radroots_tangle_schema::log_error::ILogErrorFindManyResolve, IError<SqlError>> {
        tables::log_error::find_many(self.executor(), opts)
    }

    pub fn log_error_find_one(
        &self,
        opts: &radroots_tangle_schema::log_error::ILogErrorFindOne,
    ) -> Result<radroots_tangle_schema::log_error::ILogErrorFindOneResolve, IError<SqlError>> {
        tables::log_error::find_one(self.executor(), opts)
    }

    pub fn log_error_update(
        &self,
        opts: &radroots_tangle_schema::log_error::ILogErrorUpdate,
    ) -> Result<radroots_tangle_schema::log_error::ILogErrorUpdateResolve, IError<SqlError>> {
        tables::log_error::update(self.executor(), opts)
    }

    pub fn log_error_delete(
        &self,
        opts: &radroots_tangle_schema::log_error::ILogErrorDelete,
    ) -> Result<radroots_tangle_schema::log_error::ILogErrorDeleteResolve, IError<SqlError>> {
        tables::log_error::delete(self.executor(), opts)
    }
}

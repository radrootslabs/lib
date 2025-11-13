pub use radroots_sql_core::error::SqlError;
pub use radroots_sql_core::{ExecOutcome, SqlExecutor};

pub mod tables;
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

    pub fn insert_log_error(
        &self,
        fields: radroots_tangle_schema::log_error::ILogErrorFields,
    ) -> Result<radroots_tangle_schema::log_error::LogError, SqlError> {
        tables::log_error::insert(self.executor(), fields)
    }

    pub fn find_log_errors(
        &self,
        filter: Option<&radroots_tangle_schema::log_error::ILogErrorFieldsFilter>,
    ) -> Result<Vec<radroots_tangle_schema::log_error::LogError>, SqlError> {
        tables::log_error::find_many(self.executor(), filter)
    }

    pub fn find_log_error(
        &self,
        bind: &radroots_tangle_schema::log_error::LogErrorQueryBindValues,
    ) -> Result<Option<radroots_tangle_schema::log_error::LogError>, SqlError> {
        tables::log_error::find_one(self.executor(), bind)
    }

    pub fn update_log_error(
        &self,
        id: &str,
        fields: radroots_tangle_schema::log_error::ILogErrorFieldsPartial,
    ) -> Result<ExecOutcome, SqlError> {
        tables::log_error::update(self.executor(), id, fields)
    }

    pub fn delete_log_error(
        &self,
        bind: &radroots_tangle_schema::log_error::LogErrorQueryBindValues,
    ) -> Result<ExecOutcome, SqlError> {
        tables::log_error::delete(self.executor(), bind)
    }
}

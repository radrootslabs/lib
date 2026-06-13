#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod error;
pub mod migrations;

#[cfg(all(feature = "web", target_arch = "wasm32"))]
mod export_lock;
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use export_lock::{
    export_lock_active, export_lock_begin, export_lock_end, with_export_lock_bypass,
};

#[cfg(all(feature = "bridge", target_arch = "wasm32"))]
mod executor_wasm;
#[cfg(all(feature = "bridge", target_arch = "wasm32"))]
pub use executor_wasm::WasmSqlExecutor;

#[cfg(all(feature = "native", feature = "std"))]
mod executor_sqlite;
#[cfg(all(feature = "native", feature = "std"))]
pub use executor_sqlite::SqliteExecutor;
#[cfg(all(feature = "native", feature = "std"))]
pub mod sqlite_util;

#[cfg(feature = "embedded")]
mod executor_embedded;
#[cfg(feature = "embedded")]
pub use executor_embedded::EmbeddedSqlExecutor;

#[cfg(feature = "std")]
pub mod utils;

pub use error::SqlError;

use alloc::string::String;

#[derive(Clone, Copy, Debug)]
pub struct ExecOutcome {
    pub changes: i64,
    pub last_insert_id: i64,
}

pub trait SqlExecutor: Send + Sync {
    fn exec(&self, sql: &str, params_json: &str) -> Result<ExecOutcome, SqlError>;
    fn query_raw(&self, sql: &str, params_json: &str) -> Result<String, SqlError>;
    fn begin(&self) -> Result<(), SqlError>;
    fn commit(&self) -> Result<(), SqlError>;
    fn rollback(&self) -> Result<(), SqlError>;
}

impl<T> SqlExecutor for &T
where
    T: SqlExecutor + ?Sized,
{
    fn exec(&self, sql: &str, params_json: &str) -> Result<ExecOutcome, SqlError> {
        (**self).exec(sql, params_json)
    }

    fn query_raw(&self, sql: &str, params_json: &str) -> Result<String, SqlError> {
        (**self).query_raw(sql, params_json)
    }

    fn begin(&self) -> Result<(), SqlError> {
        (**self).begin()
    }

    fn commit(&self) -> Result<(), SqlError> {
        (**self).commit()
    }

    fn rollback(&self) -> Result<(), SqlError> {
        (**self).rollback()
    }
}

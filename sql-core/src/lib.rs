#![cfg_attr(any(feature = "embedded", target_os = "espidf"), no_std)]

#[cfg(any(feature = "embedded", target_os = "espidf"))]
extern crate alloc;

pub mod error;

#[cfg(all(feature = "web", target_arch = "wasm32"))]
mod executor_wasm;
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use executor_wasm::WasmSqlExecutor;

#[cfg(feature = "native")]
mod executor_sqlite;
#[cfg(feature = "native")]
pub use executor_sqlite::SqliteExecutor;

#[cfg(feature = "embedded")]
mod executor_embedded;
#[cfg(feature = "embedded")]
pub use executor_embedded::EmbeddedSqlExecutor;

#[cfg(not(any(feature = "embedded", target_os = "espidf")))]
pub mod utils;

pub use error::SqlError;

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

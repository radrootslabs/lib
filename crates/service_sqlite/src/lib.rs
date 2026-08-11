#![forbid(unsafe_code)]

//! Reusable, service-neutral SQLite mechanics for Radroots services.

mod authority;
mod error;
mod open;
mod status;

pub use authority::WriterAuthority;
pub use error::{
    SafeServiceSqliteError, ServiceSqliteError, ServiceSqliteErrorCode, ServiceSqliteErrorKind,
};
pub use open::{OpenMode, ServiceSqlitePathError, ServiceSqlitePaths};
pub use status::{StorageHealth, StorageIntegrity, StorageStatus};

#![forbid(unsafe_code)]

//! Reusable, service-neutral SQLite mechanics for Radroots services.

mod authority;
mod config;
mod error;
mod initialize;
mod metadata;
mod migration;
mod open;
mod status;

pub use authority::WriterAuthority;
pub use config::{ServiceSqliteConnectionOptions, ServiceSqliteConnectionOptionsError};
pub use error::{
    SafeServiceSqliteError, ServiceSqliteError, ServiceSqliteErrorCode, ServiceSqliteErrorKind,
};
pub use initialize::initialize_database;
pub use metadata::{
    ServiceDatabaseIdentity, ServiceDatabaseMetadata, ServiceSqliteApplicationId,
    ServiceSqliteMetadataValueError,
};
pub use migration::{
    MigrationAppliedAtUnixSeconds, MigrationBuildIdentity, MigrationCatalog, MigrationChecksum,
    MigrationContractError, MigrationDescriptor, MigrationEvidenceError, MigrationKind,
    MigrationName,
};
pub use open::{OpenMode, ServiceSqlitePathError, ServiceSqlitePaths};
pub use status::{StorageHealth, StorageIntegrity, StorageStatus};

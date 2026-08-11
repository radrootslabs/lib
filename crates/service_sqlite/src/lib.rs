#![forbid(unsafe_code)]

//! Reusable, service-neutral SQLite mechanics for Radroots services.

mod authority;
mod config;
mod connection;
mod error;
mod initialize;
mod integrity;
mod metadata;
mod migration;
mod open;
mod status;
mod transaction_control;

pub use authority::WriterAuthority;
pub use config::{ServiceSqliteConnectionOptions, ServiceSqliteConnectionOptionsError};
pub use connection::{
    ServiceSqliteHost, ServiceSqliteTransaction, ServiceSqliteTransactionError,
    ServiceSqliteTransactionErrorKind, ServiceSqliteTransactionFuture,
};
pub use error::{
    SafeServiceSqliteError, ServiceSqliteError, ServiceSqliteErrorCode, ServiceSqliteErrorKind,
};
pub use initialize::initialize_database;
pub use integrity::{
    SchemaCatalog, SchemaCatalogContractError, SchemaDigest, SchemaObject, SchemaObjectKind,
    SchemaVersionCatalog,
};
pub use metadata::{
    ServiceDatabaseIdentity, ServiceDatabaseMetadata, ServiceSqliteApplicationId,
    ServiceSqliteMetadataValueError,
};
pub use migration::{
    MigrationApplicationOutcome, MigrationAppliedAtUnixSeconds, MigrationBuildIdentity,
    MigrationCallback, MigrationCallbackBinding, MigrationCallbackFuture, MigrationCatalog,
    MigrationChecksum, MigrationContractError, MigrationDescriptor, MigrationEvidenceError,
    MigrationKind, MigrationName, MigrationTransactionExecutor,
};
pub use open::{OpenMode, ServiceSqlitePathError, ServiceSqlitePaths};
pub use status::{StorageHealth, StorageIntegrity, StorageStatus};

#![doc = "Radroots Studio persistence adapters."]

pub mod account_namespace;
pub mod accounts;
mod compatibility;
pub mod db;
mod installation;
pub mod journal;
pub mod os_keyring;
pub mod profiles;
mod recovery;
mod repair;

pub use compatibility::{DatabasePreflight, PersistedIdentityIssue, PersistedIdentityIssueKind};
pub use db::{CURRENT_SCHEMA_VERSION, Database};
pub use os_keyring::{CREDENTIAL_SERVICE, OsKeyringSecretStore};
pub use repair::{QuarantineExportReceipt, RepairAuthorization, RepairCandidate};

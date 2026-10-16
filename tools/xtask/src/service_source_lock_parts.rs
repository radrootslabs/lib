//! Shared field models for versioned service source-lock codecs.
//! Each codec owns validation and canonical byte encoding.

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct RawSourceArchiveContract {
    pub(super) binding: String,
    pub(super) format: String,
    pub(super) compression: String,
    pub(super) compression_timestamp: String,
    pub(super) entry_order: String,
    pub(super) path_prefix: String,
    pub(super) file_mode: String,
    pub(super) uid: u32,
    pub(super) gid: u32,
    pub(super) uname: String,
    pub(super) gname: String,
    pub(super) mtime: String,
    pub(super) pax_headers: String,
    pub(super) directory_entries: String,
    pub(super) symlinks: String,
    pub(super) hardlinks: String,
    pub(super) submodules: String,
    pub(super) trailer: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct RawArtifactContract {
    pub(super) path: String,
    pub(super) sha256: String,
    pub(super) binding: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct RawSqliteContract {
    pub(super) high_level_authority: String,
    pub(super) second_pool_connection_query_transaction_migration_authority: String,
    pub(super) incremental_backup_adapter: String,
    pub(super) native_linkage_count: u32,
}

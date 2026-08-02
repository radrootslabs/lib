//! Passive SQLite integrity reporting.

use radroots_storage::{
    Error,
    status::{IntegrityHealth, IntegrityStatus},
};

use crate::SqliteStorage;

impl SqliteStorage {
    /// Returns the last recorded integrity result without running maintenance,
    /// querying SQLite pragmas, or mutating either owned database.
    pub async fn integrity(&self) -> Result<IntegrityStatus, Error> {
        self.lifecycle.integrity()
    }
}

pub(crate) fn unknown() -> Result<IntegrityStatus, Error> {
    IntegrityStatus::new(IntegrityHealth::Unknown, None, 0, 0)
}

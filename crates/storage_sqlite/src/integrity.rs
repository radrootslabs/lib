//! Explicit SQLite integrity validation and passive result reporting.

use radroots_storage::{
    Error,
    status::{IntegrityHealth, IntegrityStatus},
};
use sqlx::SqlitePool;

use crate::SqliteStorage;

impl SqliteStorage {
    /// Returns the last recorded integrity result without running maintenance,
    /// querying SQLite pragmas, or mutating either owned database.
    pub async fn integrity(&self) -> Result<IntegrityStatus, Error> {
        self.lifecycle.integrity()
    }

    /// Explicitly validates physical and referential integrity for both owned
    /// databases and records the result under a caller-supplied timestamp.
    pub async fn check_integrity(&self, checked_at_unix_ms: u64) -> Result<IntegrityStatus, Error> {
        if checked_at_unix_ms == 0 {
            return Err(Error::InvalidIntegrityStatus);
        }
        self.lifecycle.require_open()?;

        let outcomes = [
            check_member(&self.pool).await,
            check_member(&self.private_pool).await,
        ];
        let verified_members = outcomes
            .iter()
            .filter(|outcome| **outcome == MemberOutcome::Verified)
            .count();
        let failed_members = outcomes.len().saturating_sub(verified_members);
        let health = if outcomes.contains(&MemberOutcome::Corrupt) {
            IntegrityHealth::Corrupt
        } else if outcomes.contains(&MemberOutcome::Unavailable) {
            IntegrityHealth::Degraded
        } else {
            IntegrityHealth::Healthy
        };
        let status = IntegrityStatus::new(
            health,
            Some(checked_at_unix_ms),
            u32::try_from(verified_members).map_err(|_| Error::InvalidIntegrityStatus)?,
            u32::try_from(failed_members).map_err(|_| Error::InvalidIntegrityStatus)?,
        )?;
        self.lifecycle.record_integrity(status)
    }
}

pub(crate) fn unknown() -> Result<IntegrityStatus, Error> {
    IntegrityStatus::new(IntegrityHealth::Unknown, None, 0, 0)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MemberOutcome {
    Verified,
    Corrupt,
    Unavailable,
}

async fn check_member(pool: &SqlitePool) -> MemberOutcome {
    let mut connection = match pool.acquire().await {
        Ok(connection) => connection,
        Err(_) => return MemberOutcome::Unavailable,
    };
    check_connection(&mut connection).await
}

pub(crate) async fn check_connection(connection: &mut sqlx::SqliteConnection) -> MemberOutcome {
    let integrity = match sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
        .fetch_all(&mut *connection)
        .await
    {
        Ok(rows) => rows,
        Err(_) => return MemberOutcome::Unavailable,
    };
    if integrity.as_slice() != ["ok"] {
        return MemberOutcome::Corrupt;
    }
    match sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&mut *connection)
        .await
    {
        Ok(rows) if rows.is_empty() => MemberOutcome::Verified,
        Ok(_) => MemberOutcome::Corrupt,
        Err(_) => MemberOutcome::Unavailable,
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod policy_tests {
    use serde::Deserialize;

    const POLICY: &str = include_str!("../../../contracts/storage/integrity_policy_v1.toml");

    #[derive(Deserialize)]
    struct Policy {
        schema_version: u32,
        members: Vec<String>,
        invocation: String,
        checks: Vec<String>,
        timestamp: String,
        initial_passive_health: String,
        passive_status_runs_checks: bool,
        recording: String,
        closed_backend: String,
        healthy: String,
        corrupt: String,
        degraded: String,
    }

    #[test]
    fn implementation_matches_the_governed_integrity_policy() {
        let policy = toml::from_str::<Policy>(POLICY).expect("integrity policy");
        assert_eq!(policy.schema_version, 1);
        assert_eq!(policy.members, ["runtime.sqlite", "private.sqlite"]);
        assert_eq!(policy.invocation, "explicit_only");
        assert_eq!(
            policy.checks,
            ["pragma_integrity_check", "pragma_foreign_key_check"]
        );
        assert_eq!(policy.timestamp, "caller_supplied_positive_unix_ms");
        assert_eq!(policy.initial_passive_health, "unknown");
        assert!(!policy.passive_status_runs_checks);
        assert_eq!(policy.recording, "latest_monotonic_checked_at");
        assert_eq!(policy.closed_backend, "reject");
        assert_eq!(policy.healthy, "all_members_verified");
        assert_eq!(
            policy.corrupt,
            "one_or_more_members_failed_a_completed_check"
        );
        assert_eq!(
            policy.degraded,
            "one_or_more_members_could_not_complete_checks"
        );
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use radroots_storage::{
        Error,
        event::SourceGeneration,
        status::{EventStoreMode, IntegrityHealth},
    };
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::{OpenMode, OpenOptions, Paths, SqliteStorage};

    fn generation(byte: u8) -> SourceGeneration {
        SourceGeneration::new([byte; 32]).expect("source generation")
    }

    async fn create(directory: &std::path::Path) -> SqliteStorage {
        let paths = Paths::from_directory(directory).expect("owned paths");
        SqliteStorage::open(
            OpenOptions::new(paths, OpenMode::Create)
                .with_source_generation(generation(83), 8_300)
                .expect("source generation"),
        )
        .await
        .expect("create storage")
    }

    #[tokio::test]
    async fn explicit_checks_record_healthy_and_corrupt_results_monotonically() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = create(directory.path()).await;

        assert_eq!(
            store.check_integrity(0).await,
            Err(Error::InvalidIntegrityStatus)
        );
        let healthy = store.check_integrity(100).await.expect("healthy check");
        assert_eq!(healthy.health(), IntegrityHealth::Healthy);
        assert_eq!(healthy.checked_at_unix_ms(), Some(100));
        assert_eq!(healthy.verified_members(), 2);
        assert_eq!(healthy.failed_members(), 0);
        assert_eq!(store.integrity().await.expect("passive result"), healthy);

        let mut private = store
            .private_pool
            .acquire()
            .await
            .expect("private connection");
        sqlx::raw_sql(
            "PRAGMA foreign_keys = OFF;
             CREATE TABLE integrity_parent (id INTEGER PRIMARY KEY);
             CREATE TABLE integrity_child (
               parent_id INTEGER NOT NULL REFERENCES integrity_parent(id)
             );
             INSERT INTO integrity_child (parent_id) VALUES (1);",
        )
        .execute(&mut *private)
        .await
        .expect("inject referential corruption");
        drop(private);

        let corrupt = store.check_integrity(200).await.expect("corrupt check");
        assert_eq!(corrupt.health(), IntegrityHealth::Corrupt);
        assert_eq!(corrupt.checked_at_unix_ms(), Some(200));
        assert_eq!(corrupt.verified_members(), 1);
        assert_eq!(corrupt.failed_members(), 1);
        assert_eq!(
            store
                .storage_status()
                .await
                .expect("passive storage status")
                .integrity(),
            corrupt
        );
        assert_eq!(
            store.check_integrity(199).await,
            Err(Error::InvalidIntegrityStatus)
        );
        assert_eq!(store.integrity().await.expect("latest result"), corrupt);

        store.close().await.expect("close storage");
        assert_eq!(
            store.check_integrity(300).await,
            Err(Error::BackendUnavailable)
        );
    }

    #[tokio::test]
    async fn incomplete_member_checks_record_degraded_status() {
        let runtime = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("runtime pool");
        let private = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("private pool");
        let store = SqliteStorage::with_private_pool(
            runtime,
            private.clone(),
            generation(84),
            EventStoreMode::ReadWrite,
        );
        private.close().await;

        let degraded = store.check_integrity(400).await.expect("degraded check");
        assert_eq!(degraded.health(), IntegrityHealth::Degraded);
        assert_eq!(degraded.verified_members(), 1);
        assert_eq!(degraded.failed_members(), 1);
        assert_eq!(store.integrity().await.expect("recorded result"), degraded);
    }
}

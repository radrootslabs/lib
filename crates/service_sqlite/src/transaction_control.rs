//! Private deny-by-default SQLite transaction-control fencing.

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use sqlx::SqliteConnection;

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) struct TransactionControlGate {
    allow_commit: Arc<AtomicBool>,
    allow_runner_rollback: Arc<AtomicBool>,
    rejected_commit: Arc<AtomicBool>,
    rollback_observed: Arc<AtomicBool>,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl TransactionControlGate {
    pub(crate) async fn install(connection: &mut SqliteConnection) -> Result<Self, sqlx::Error> {
        let allow_commit = Arc::new(AtomicBool::new(false));
        let allow_runner_rollback = Arc::new(AtomicBool::new(false));
        let rejected_commit = Arc::new(AtomicBool::new(false));
        let rollback_observed = Arc::new(AtomicBool::new(false));
        let hook_permission = Arc::clone(&allow_commit);
        let rejected_commit_epoch = Arc::clone(&rejected_commit);
        let rollback_permission = Arc::clone(&allow_runner_rollback);
        let rollback_epoch = Arc::clone(&rollback_observed);
        let mut handle = connection.lock_handle().await?;
        handle.set_commit_hook(move || {
            let permitted = hook_permission.load(Ordering::Acquire);
            if !permitted {
                rejected_commit_epoch.store(true, Ordering::Release);
            }
            permitted
        });
        handle.set_rollback_hook(move || {
            if !rollback_permission.load(Ordering::Acquire) {
                rollback_epoch.store(true, Ordering::Release);
            }
        });
        drop(handle);
        Ok(Self {
            allow_commit,
            allow_runner_rollback,
            rejected_commit,
            rollback_observed,
        })
    }

    pub(crate) fn permit_outer_commit(&self) -> TransactionCommitPermit {
        self.allow_commit.store(true, Ordering::Release);
        TransactionCommitPermit {
            allow_commit: Arc::clone(&self.allow_commit),
        }
    }

    pub(crate) fn permit_runner_rollback(&self) -> TransactionRollbackPermit {
        self.allow_runner_rollback.store(true, Ordering::Release);
        TransactionRollbackPermit {
            allow_runner_rollback: Arc::clone(&self.allow_runner_rollback),
        }
    }

    pub(crate) fn control_violation_observed(&self) -> bool {
        self.rejected_commit.load(Ordering::Acquire)
            || self.rollback_observed.load(Ordering::Acquire)
    }

    pub(crate) fn rejected_commit_rolled_back(&self) -> bool {
        self.rejected_commit.load(Ordering::Acquire)
            && self.rollback_observed.load(Ordering::Acquire)
    }

    pub(crate) async fn remove(self, connection: &mut SqliteConnection) -> Result<(), sqlx::Error> {
        self.allow_commit.store(false, Ordering::Release);
        self.allow_runner_rollback.store(false, Ordering::Release);
        let mut handle = connection.lock_handle().await?;
        handle.remove_commit_hook();
        handle.remove_rollback_hook();
        Ok(())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) struct TransactionCommitPermit {
    allow_commit: Arc<AtomicBool>,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for TransactionCommitPermit {
    fn drop(&mut self) {
        self.allow_commit.store(false, Ordering::Release);
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) struct TransactionRollbackPermit {
    allow_runner_rollback: Arc<AtomicBool>,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for TransactionRollbackPermit {
    fn drop(&mut self) {
        self.allow_runner_rollback.store(false, Ordering::Release);
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use sqlx::{Connection, SqliteConnection};

    use super::TransactionControlGate;

    async fn memory_connection() -> SqliteConnection {
        let mut connection = SqliteConnection::connect("sqlite::memory:")
            .await
            .expect("memory SQLite connection");
        sqlx::query("CREATE TABLE gate_probe (value INTEGER NOT NULL)")
            .execute(&mut connection)
            .await
            .expect("gate probe table");
        connection
    }

    #[tokio::test(flavor = "current_thread")]
    async fn denied_commit_and_unpermitted_rollback_are_observed_exactly() {
        let mut connection = memory_connection().await;
        let gate = TransactionControlGate::install(&mut connection)
            .await
            .expect("transaction gate");
        let mut transaction = connection.begin().await.expect("transaction");
        sqlx::query("INSERT INTO gate_probe (value) VALUES (1)")
            .execute(&mut *transaction)
            .await
            .expect("mutate denied transaction");
        transaction
            .commit()
            .await
            .expect_err("commit must be denied");
        assert!(gate.control_violation_observed());
        assert!(gate.rejected_commit_rolled_back());
        gate.remove(&mut connection).await.expect("remove gate");

        let mut connection = memory_connection().await;
        let gate = TransactionControlGate::install(&mut connection)
            .await
            .expect("transaction gate");
        let mut transaction = connection.begin().await.expect("transaction");
        sqlx::query("INSERT INTO gate_probe (value) VALUES (2)")
            .execute(&mut *transaction)
            .await
            .expect("mutate rolled back transaction");
        transaction.rollback().await.expect("SQLite rollback");
        assert!(gate.control_violation_observed());
        assert!(!gate.rejected_commit_rolled_back());
        gate.remove(&mut connection).await.expect("remove gate");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn runner_permits_are_scoped_and_do_not_record_violations() {
        let mut connection = memory_connection().await;
        let gate = TransactionControlGate::install(&mut connection)
            .await
            .expect("transaction gate");

        let mut transaction = connection.begin().await.expect("transaction");
        sqlx::query("INSERT INTO gate_probe (value) VALUES (3)")
            .execute(&mut *transaction)
            .await
            .expect("mutate committed transaction");
        let permit = gate.permit_outer_commit();
        transaction.commit().await.expect("permitted commit");
        drop(permit);
        assert!(!gate.control_violation_observed());
        assert!(!gate.rejected_commit_rolled_back());

        let mut transaction = connection.begin().await.expect("transaction");
        sqlx::query("INSERT INTO gate_probe (value) VALUES (4)")
            .execute(&mut *transaction)
            .await
            .expect("mutate runner rollback transaction");
        let permit = gate.permit_runner_rollback();
        transaction.rollback().await.expect("permitted rollback");
        drop(permit);
        assert!(!gate.control_violation_observed());
        assert!(!gate.rejected_commit_rolled_back());
        gate.remove(&mut connection).await.expect("remove gate");
    }
}

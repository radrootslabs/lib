use radroots_storage::backup::BackupCapabilityError as Error;
use sqlx::{Connection, Sqlite, SqlitePool, pool::PoolConnection};

use crate::SqliteStorage;

/// PoolConnection's asynchronous return pings its worker before releasing the
/// permit. Holding every configured permit therefore also waits for cancelled
/// executor work on connections other than the next snapshot connection.
pub(super) async fn settle(store: &SqliteStorage) -> Result<(), Error> {
    store
        .lifecycle
        .require_open()
        .map_err(|_| Error::Unavailable)?;
    let runtime = settle_pool(&store.pool).await?;
    let protected = settle_pool(&store.private_pool).await?;
    store
        .lifecycle
        .require_open()
        .map_err(|_| Error::Unavailable)?;
    // Retain both sets simultaneously. Dropping a cancelled attempt returns
    // every acquired permit through the same owner, without inventing success.
    drop((runtime, protected));
    Ok(())
}

async fn settle_pool(pool: &SqlitePool) -> Result<Vec<PoolConnection<Sqlite>>, Error> {
    let capacity = pool.options().get_max_connections();
    let mut connections = Vec::with_capacity(capacity as usize);
    for _ in 0..capacity {
        let mut connection = pool.acquire().await.map_err(|_| Error::Unavailable)?;
        connection.ping().await.map_err(|_| Error::Unavailable)?;
        connections.push(connection);
    }
    Ok(connections)
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_storage::backup::StorageReliability;
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };

    async fn fixture() -> (tempfile::TempDir, SqliteStorage) {
        let root = tempfile::tempdir().unwrap();
        let store = SqliteStorage::open(
            crate::OpenOptions::new(
                crate::Paths::from_directory(root.path()).unwrap(),
                crate::OpenMode::Create,
            )
            .with_source_generation(
                radroots_storage::event::SourceGeneration::new([7; 32]).unwrap(),
                100,
            )
            .unwrap(),
        )
        .await
        .unwrap();
        (root, store)
    }

    fn poll<F: Future + ?Sized>(future: std::pin::Pin<&mut F>) -> Poll<F::Output> {
        future.poll(&mut Context::from_waker(Waker::noop()))
    }

    #[tokio::test]
    async fn settling_waits_for_both_full_pools_and_cancelled_attempt_can_retry() {
        let (_root, store) = fixture().await;
        let runtime = settle_pool(&store.pool).await.unwrap();
        let mut protected = settle_pool(&store.private_pool).await.unwrap();
        let mut attempt = store.settle_backup_writes();
        assert!(poll(attempt.as_mut()).is_pending());
        drop(runtime);
        for _ in 0..32 {
            tokio::task::yield_now().await;
            assert!(poll(attempt.as_mut()).is_pending());
        }
        // An idle connection in one pool is not enough. The last admitted
        // protected member must finish before a cross-member inventory begins.
        let last_protected = protected.pop().unwrap();
        drop(protected);
        for _ in 0..32 {
            tokio::task::yield_now().await;
            assert!(poll(attempt.as_mut()).is_pending());
        }
        drop(attempt);
        drop(last_protected);
        store.settle_backup_writes().await.unwrap();
        assert_eq!(
            settle_pool(&store.pool).await.unwrap().len(),
            store.pool.options().get_max_connections() as usize
        );
        store.close().await.unwrap();
        assert_eq!(store.settle_backup_writes().await, Err(Error::Unavailable));
    }

    #[tokio::test]
    async fn settled_owner_observes_committed_and_abandoned_transactions_without_late_changes() {
        let (_root, store) = fixture().await;
        sqlx::query("CREATE TABLE backup_settling_fixture(value INTEGER NOT NULL)")
            .execute(&store.pool)
            .await
            .unwrap();
        let mut pending = store.pool.begin().await.unwrap();
        sqlx::query("INSERT INTO backup_settling_fixture VALUES (1)")
            .execute(&mut *pending)
            .await
            .unwrap();
        let mut attempt = store.settle_backup_writes();
        assert!(poll(attempt.as_mut()).is_pending());
        // Dropping a transaction schedules its rollback. No application future
        // remains to represent that work, but the owner must still settle it.
        drop(pending);
        attempt.await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM backup_settling_fixture")
            .fetch_one(&store.pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        let mut committed = store.pool.begin().await.unwrap();
        sqlx::query("INSERT INTO backup_settling_fixture VALUES (2)")
            .execute(&mut *committed)
            .await
            .unwrap();
        committed.commit().await.unwrap();
        store.settle_backup_writes().await.unwrap();
        for _ in 0..2 {
            let values: Vec<i64> = sqlx::query_scalar("SELECT value FROM backup_settling_fixture")
                .fetch_all(&store.pool)
                .await
                .unwrap();
            assert_eq!(values, [2]);
        }
        store.close().await.unwrap();
    }
}

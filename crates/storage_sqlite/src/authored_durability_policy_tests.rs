use super::*;
use radroots_storage::event::SourceGeneration;
use tempfile::TempDir;

#[tokio::test]
async fn authored_durability_policy_holds_for_every_owned_pool_connection() {
    let temp = TempDir::new().unwrap();
    let store = SqliteStorage::open(
        OpenOptions::new(
            Paths::from_directory(temp.path()).unwrap(),
            OpenMode::Create,
        )
        .with_source_generation(SourceGeneration::new([93; 32]).unwrap(), 9)
        .unwrap(),
    )
    .await
    .unwrap();
    for (pool, database) in [
        (store.pool(), RUNTIME_DATABASE_NAME),
        (store.private_pool(), PRIVATE_DATABASE_NAME),
    ] {
        let mut connections = Vec::new();
        for _ in 0..MAX_CONNECTIONS_PER_DATABASE {
            connections.push(pool.acquire().await.unwrap());
        }
        assert_eq!(connections.len(), 4);
        for connection in &mut connections {
            verify_connection(connection, database, Duration::from_millis(5_000))
                .await
                .unwrap();
            sqlx::query("PRAGMA fullfsync = OFF")
                .execute(&mut **connection)
                .await
                .unwrap();
            assert!(matches!(
                verify_connection(connection, database, Duration::from_millis(5_000)).await,
                Err(Error::ConnectionPolicyMismatch { database: actual }) if actual == database
            ));
            sqlx::query("PRAGMA fullfsync = ON")
                .execute(&mut **connection)
                .await
                .unwrap();
            verify_connection(connection, database, Duration::from_millis(5_000))
                .await
                .unwrap();
        }
    }
    store.close().await.unwrap();
}

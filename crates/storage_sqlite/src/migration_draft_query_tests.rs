use super::tests::{connection, establish_runtime_version, pragma};
use super::*;

const FAILING_V14: &str = concat!(
    include_str!("migration/runtime/0014_authored_draft_query_metadata.up.sql"),
    "\nINSERT INTO missing_fixture_table VALUES (1);"
);

async fn seed(connection: &mut SqliteConnection) -> Vec<Vec<u8>> {
    let snapshots = vec![
        br#"{"payload_schema":"fixture.composer.v1","payload":[49,46]}"#.to_vec(),
        b"{broken historical row".to_vec(),
    ];
    for (index, snapshot) in snapshots.iter().enumerate() {
        sqlx::query("INSERT INTO radroots_runtime_authored_draft_revisions
            (draft_id, revision, author, stage, payload_sha256, created_at_unix_ms, updated_at_unix_ms, snapshot)
            VALUES (?, 1, ?, 0, ?, 10, 10, ?)")
            .bind([index as u8 + 1; 16].as_slice()).bind([7; 32].as_slice()).bind([3; 32].as_slice()).bind(snapshot)
            .execute(&mut *connection).await.unwrap();
    }
    snapshots
}
fn plan(current: u32, fail: bool) -> MigrationPlan {
    let steps = runtime::MIGRATIONS
        .iter()
        .take(current as usize)
        .map(|step| MigrationStep {
            version: step.version(),
            sql: if fail && step.version() == 14 {
                FAILING_V14
            } else {
                runtime::migration_sql(step.version()).unwrap()
            },
            owned_objects: step.owned_objects(),
        })
        .collect();
    MigrationPlan {
        database: RUNTIME_DATABASE,
        application_id: RUNTIME_APPLICATION_ID,
        set_application_id_sql: SET_RUNTIME_APPLICATION_ID,
        minimum_version: runtime::MINIMUM_VERSION,
        current_version: current,
        steps,
    }
}

#[tokio::test]
async fn draft_metadata_upgrade_preserves_source_and_rejects_prior_schema_policy() {
    let mut connection = connection().await;
    establish_runtime_version(&mut connection, 13).await;
    let snapshots = seed(&mut connection).await;
    assert!(matches!(
        migrate_runtime(&mut connection, OpenMode::ReadOnly).await,
        Err(Error::SchemaMigrationRequired {
            current: 14,
            actual: 13,
            ..
        })
    ));
    let report = migrate_runtime(&mut connection, OpenMode::ReadWriteExisting)
        .await
        .unwrap();
    assert_eq!(report.applied(), 1);
    assert_eq!(pragma(&mut connection, "user_version").await, 14);
    let actual: Vec<Vec<u8>> = sqlx::query_scalar(
        "SELECT snapshot FROM radroots_runtime_authored_draft_revisions ORDER BY draft_id",
    )
    .fetch_all(&mut connection)
    .await
    .unwrap();
    assert_eq!(actual, snapshots);
    let schemas: Vec<String> = sqlx::query_scalar(
        "SELECT payload_schema FROM radroots_runtime_authored_draft_revisions ORDER BY draft_id",
    )
    .fetch_all(&mut connection)
    .await
    .unwrap();
    assert_eq!(schemas, ["fixture.composer.v1", ""]);
    assert!(matches!(
        migrate(&mut connection, OpenMode::ReadOnly, &plan(13, false)).await,
        Err(Error::SchemaTooNew {
            supported: 13,
            actual: 14,
            ..
        })
    ));
    assert!(
        sqlx::query(
            "UPDATE radroots_runtime_authored_draft_revisions SET payload_schema = 'changed'"
        )
        .execute(&mut connection)
        .await
        .is_err()
    );
    assert_eq!(
        migrate_runtime(&mut connection, OpenMode::ReadOnly)
            .await
            .unwrap()
            .applied(),
        0
    );
    connection.close().await.unwrap();
}

#[tokio::test]
async fn metadata_migration_failure_rolls_back_columns_index_guards_and_version() {
    let mut connection = connection().await;
    establish_runtime_version(&mut connection, 13).await;
    let snapshots = seed(&mut connection).await;
    assert!(
        migrate(
            &mut connection,
            OpenMode::ReadWriteExisting,
            &plan(14, true)
        )
        .await
        .is_err()
    );
    assert_eq!(pragma(&mut connection, "user_version").await, 13);
    assert!(
        sqlx::query("SELECT payload_schema FROM radroots_runtime_authored_draft_revisions")
            .fetch_all(&mut connection)
            .await
            .is_err()
    );
    let actual: Vec<Vec<u8>> = sqlx::query_scalar(
        "SELECT snapshot FROM radroots_runtime_authored_draft_revisions ORDER BY draft_id",
    )
    .fetch_all(&mut connection)
    .await
    .unwrap();
    assert_eq!(actual, snapshots);
    assert!(
        sqlx::query("UPDATE radroots_runtime_authored_draft_revisions SET stage = 1")
            .execute(&mut connection)
            .await
            .is_err()
    );
    migrate_runtime(&mut connection, OpenMode::ReadWriteExisting)
        .await
        .unwrap();
    connection.close().await.unwrap();
}

use super::{
    tests::{connection, establish_runtime_version, pragma},
    *,
};
use crate::authored::signed_fact_fixture;
use radroots_storage::{
    atomic::AtomicCommitDisposition,
    authored_atomic::{AuthoredAtomicCommand, AuthoredAtomicOutcome, AuthoredAtomicReceipt},
};

const FAILING_V15: &str = concat!(
    include_str!("migration/runtime/0015_authored_signed_facts.up.sql"),
    "\nINSERT INTO missing_fixture_table VALUES (1);"
);

fn plan(current: u32, fail: bool) -> MigrationPlan {
    MigrationPlan {
        database: RUNTIME_DATABASE,
        application_id: RUNTIME_APPLICATION_ID,
        set_application_id_sql: SET_RUNTIME_APPLICATION_ID,
        minimum_version: runtime::MINIMUM_VERSION,
        current_version: current,
        steps: runtime::MIGRATIONS
            .iter()
            .take(current as usize)
            .map(|step| MigrationStep {
                version: step.version(),
                sql: if fail && step.version() == 15 {
                    FAILING_V15
                } else {
                    runtime::migration_sql(step.version()).unwrap()
                },
                owned_objects: step.owned_objects(),
            })
            .collect(),
    }
}

pub(super) async fn seed(connection: &mut SqliteConnection) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let (command, _) = signed_fact_fixture::prepare();
    let AuthoredAtomicCommand::Prepare(prepared) = &command else {
        unreachable!()
    };
    let operation = prepared.operation();
    let artifact = &prepared.artifacts()[0];
    let operation_snapshot = serde_json::to_vec(operation).unwrap();
    let artifact_snapshot = serde_json::to_vec(artifact).unwrap();
    sqlx::query("INSERT INTO radroots_runtime_authored_operations (operation_id, artifact_count, created_at_unix_ms, updated_at_unix_ms, revision, snapshot) VALUES (?, 1, 10, 10, 1, ?)")
        .bind(operation.operation_id().as_bytes().as_slice()).bind(operation_snapshot).execute(&mut *connection).await.unwrap();
    sqlx::query("INSERT INTO radroots_runtime_authored_artifacts (artifact_id, operation_id, ordinal, origin, signing_state, admission_state, plan_wire, created_at_unix_ms, updated_at_unix_ms, revision, snapshot) VALUES (?, ?, 0, 'planned', 'planned', 'pending', ?, 10, 10, 1, ?)")
        .bind(artifact.artifact_id().as_bytes().as_slice()).bind(operation.operation_id().as_bytes().as_slice())
        .bind(artifact.plan().unwrap().wire_json()).bind(&artifact_snapshot).execute(&mut *connection).await.unwrap();
    for delivery in prepared.delivery_plans() {
        sqlx::query("INSERT INTO radroots_runtime_authored_delivery_plans (plan_id, artifact_id, request_digest, state, attempt_count, created_at_unix_ms, updated_at_unix_ms, revision, snapshot) VALUES (?, ?, ?, 'pending', 0, 10, 10, 1, ?)")
            .bind(delivery.plan_id().as_bytes().as_slice()).bind(delivery.artifact_id().as_bytes().as_slice())
            .bind(delivery.request_digest().as_slice()).bind(serde_json::to_vec(delivery).unwrap()).execute(&mut *connection).await.unwrap();
        for (ordinal, target) in delivery.intent().target_set().targets().iter().enumerate() {
            sqlx::query("INSERT INTO radroots_runtime_authored_delivery_targets (plan_id, ordinal, target_fingerprint, target_snapshot) VALUES (?, ?, ?, ?)")
                .bind(delivery.plan_id().as_bytes().as_slice()).bind(i64::try_from(ordinal).unwrap())
                .bind(target.fingerprint().as_str()).bind(serde_json::to_vec(target).unwrap()).execute(&mut *connection).await.unwrap();
        }
    }
    let outcome = AuthoredAtomicOutcome::Prepared {
        operation: operation.clone(),
        artifacts: prepared.artifacts().to_vec(),
        delivery_plans: prepared.delivery_plans().to_vec(),
    };
    let receipt = AuthoredAtomicReceipt::new(
        &command,
        AtomicCommitDisposition::Committed,
        10,
        outcome.clone(),
    )
    .unwrap();
    let receipt_bytes = serde_json::to_vec(&serde_json::json!({"outcome": outcome})).unwrap();
    sqlx::query("INSERT INTO radroots_runtime_authored_atomic_commits (commit_id, commit_digest, phase, target_id, requested_at_unix_ms, committed_at_unix_ms, receipt) VALUES (?, ?, 'prepare', ?, 10, 10, ?)")
        .bind(receipt.commit_id().as_bytes().as_slice()).bind(receipt.digest().as_bytes().as_slice())
        .bind(operation.operation_id().as_bytes().as_slice()).bind(&receipt_bytes).execute(&mut *connection).await.unwrap();
    (
        artifact_snapshot,
        receipt_bytes,
        receipt.commit_id().as_bytes().to_vec(),
    )
}

pub(super) async fn retained(
    connection: &mut SqliteConnection,
    expected: &(Vec<u8>, Vec<u8>, Vec<u8>),
) {
    let artifact: Vec<u8> =
        sqlx::query_scalar("SELECT snapshot FROM radroots_runtime_authored_artifacts")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
    let (receipt, id): (Vec<u8>, Vec<u8>) =
        sqlx::query_as("SELECT receipt, commit_id FROM radroots_runtime_authored_atomic_commits")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
    assert_eq!((artifact, receipt, id), *expected);
}

#[tokio::test]
async fn v15_preserves_v14_rows_and_receipts_and_prior_schema_policy_fails_closed() {
    let mut connection = connection().await;
    establish_runtime_version(&mut connection, 14).await;
    let old = seed(&mut connection).await;
    assert!(matches!(
        migrate(&mut connection, OpenMode::ReadOnly, &plan(15, false)).await,
        Err(Error::SchemaMigrationRequired {
            actual: 14,
            current: 15,
            ..
        })
    ));
    assert_eq!(
        migrate(
            &mut connection,
            OpenMode::ReadWriteExisting,
            &plan(15, false)
        )
        .await
        .unwrap()
        .applied(),
        1
    );
    assert_eq!(pragma(&mut connection, "user_version").await, 15);
    retained(&mut connection, &old).await;
    let stop: Option<String> =
        sqlx::query_scalar("SELECT signing_stop FROM radroots_runtime_authored_artifacts")
            .fetch_one(&mut connection)
            .await
            .unwrap();
    assert_eq!(stop, None);
    for mode in [OpenMode::ReadOnly, OpenMode::ReadWriteExisting] {
        assert!(matches!(
            migrate(&mut connection, mode, &plan(14, false)).await,
            Err(Error::SchemaTooNew {
                actual: 15,
                supported: 14,
                ..
            })
        ));
    }
    assert_eq!(
        migrate(&mut connection, OpenMode::ReadOnly, &plan(15, false))
            .await
            .unwrap()
            .applied(),
        0
    );
    connection.close().await.unwrap();
}

#[tokio::test]
async fn failed_v15_migration_rolls_back_column_guard_version_and_historical_bytes() {
    let mut connection = connection().await;
    establish_runtime_version(&mut connection, 14).await;
    let old = seed(&mut connection).await;
    assert!(
        migrate(
            &mut connection,
            OpenMode::ReadWriteExisting,
            &plan(15, true)
        )
        .await
        .is_err()
    );
    assert_eq!(pragma(&mut connection, "user_version").await, 14);
    retained(&mut connection, &old).await;
    assert!(
        sqlx::query("SELECT signing_stop FROM radroots_runtime_authored_artifacts")
            .fetch_all(&mut connection)
            .await
            .is_err()
    );
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sqlite_schema WHERE name = 'radroots_runtime_authored_artifacts_signed_fact_guard'").fetch_one(&mut connection).await.unwrap(), 0);
    migrate_runtime(&mut connection, OpenMode::ReadWriteExisting)
        .await
        .unwrap();
    retained(&mut connection, &old).await;
    connection.close().await.unwrap();
}

#[test]
fn signed_fact_decision_binds_the_exact_successor_migration() {
    let decision: serde_json::Value = serde_json::from_str(include_str!(
        "../../../contracts/architecture/decisions/authored_signed_facts.v1.json"
    ))
    .unwrap();
    let migration = runtime::MIGRATIONS[14];
    assert_eq!(decision["migration"]["version"], migration.version());
    assert_eq!(decision["migration"]["name"], migration.name());
    assert_eq!(decision["migration"]["sha256"], migration.up_sha256());
}

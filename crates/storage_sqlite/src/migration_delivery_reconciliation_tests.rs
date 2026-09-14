use super::{
    tests::{connection, establish_runtime_version, pragma},
    *,
};
use crate::authored::signed_fact_fixture::{claim, prepare};
use radroots_storage::{
    atomic::AtomicCommitDisposition,
    authored_atomic::{
        AuthoredAtomicCommand, AuthoredAtomicOutcome, AuthoredAtomicReceipt, ClaimAuthoredTarget,
        ClaimAuthoredWork,
    },
};

const FAILING_V17: &str = concat!(
    include_str!("migration/runtime/0017_authored_delivery_reconciliation.up.sql"),
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
                sql: if fail && step.version() == 17 {
                    FAILING_V17
                } else {
                    runtime::migration_sql(step.version()).unwrap()
                },
                owned_objects: step.owned_objects(),
            })
            .collect(),
    }
}

async fn seed(connection: &mut SqliteConnection) {
    super::signed_facts_tests::seed(connection).await;
    let (command, event) = prepare();
    let AuthoredAtomicCommand::Prepare(prepared) = command else {
        unreachable!()
    };
    let mut delivery = prepared.delivery_plans()[0].clone();
    delivery.bind_signed_event(event, 12).unwrap();
    for (token, at) in [(5, 13), (6, 40)] {
        let active = claim(delivery.revision(), token, at);
        delivery.claim(active.clone(), at).unwrap();
        let command = AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(delivery.plan_id()),
            active,
        ));
        let receipt = AuthoredAtomicReceipt::new(
            &command,
            AtomicCommitDisposition::Committed,
            at,
            AuthoredAtomicOutcome::DeliveryPlan(delivery.clone()),
        )
        .unwrap();
        sqlx::query("INSERT INTO radroots_runtime_authored_atomic_commits (commit_id, commit_digest, phase, target_id, requested_at_unix_ms, committed_at_unix_ms, receipt) VALUES (?, ?, 'claim', ?, ?, ?, ?)")
            .bind(receipt.commit_id().as_bytes().as_slice()).bind(receipt.digest().as_bytes().as_slice())
            .bind(delivery.plan_id().as_bytes().as_slice()).bind(at as i64).bind(at as i64)
            .bind(serde_json::to_vec(&serde_json::json!({"outcome": receipt.outcome()})).unwrap())
            .execute(&mut *connection).await.unwrap();
    }
    delivery.request_stop(61).unwrap();
    let mut transaction = connection.begin().await.unwrap();
    crate::authored::persist_plan_v11(&mut transaction, &delivery)
        .await
        .unwrap();
    transaction.commit().await.unwrap();
}

async fn snapshots(connection: &mut SqliteConnection) -> (Vec<u8>, Vec<(Vec<u8>, Vec<u8>)>) {
    let plan = sqlx::query_scalar("SELECT snapshot FROM radroots_runtime_authored_delivery_plans")
        .fetch_one(&mut *connection)
        .await
        .unwrap();
    let receipts = sqlx::query_as("SELECT commit_id, receipt FROM radroots_runtime_authored_atomic_commits ORDER BY commit_id").fetch_all(&mut *connection).await.unwrap();
    (plan, receipts)
}

#[tokio::test]
async fn v17_backfills_stopped_issued_history_without_rewriting_old_receipts() {
    let mut connection = connection().await;
    establish_runtime_version(&mut connection, 16).await;
    seed(&mut connection).await;
    let before = snapshots(&mut connection).await;
    assert!(matches!(
        migrate(&mut connection, OpenMode::ReadOnly, &plan(17, false)).await,
        Err(Error::SchemaMigrationRequired {
            actual: 16,
            current: 17,
            ..
        })
    ));
    assert_eq!(
        migrate(
            &mut connection,
            OpenMode::ReadWriteExisting,
            &plan(17, false)
        )
        .await
        .unwrap()
        .applied(),
        1
    );
    assert_eq!(snapshots(&mut connection).await, before);
    let projected: Vec<Vec<u8>> = sqlx::query_scalar(
        "SELECT claim_id FROM radroots_runtime_authored_delivery_claims ORDER BY claim_id",
    )
    .fetch_all(&mut connection)
    .await
    .unwrap();
    let original: Vec<Vec<u8>> = sqlx::query_scalar("SELECT commit_id FROM radroots_runtime_authored_atomic_commits WHERE phase = 'claim' ORDER BY commit_id").fetch_all(&mut connection).await.unwrap();
    assert_eq!(projected.len(), 2);
    assert_eq!(projected, original);
    for mode in [OpenMode::ReadOnly, OpenMode::ReadWriteExisting] {
        assert!(matches!(
            migrate(&mut connection, mode, &plan(16, false)).await,
            Err(Error::SchemaTooNew {
                actual: 17,
                supported: 16,
                ..
            })
        ));
    }
    assert_eq!(
        migrate(&mut connection, OpenMode::ReadOnly, &plan(17, false))
            .await
            .unwrap()
            .applied(),
        0
    );
    connection.close().await.unwrap();
}

#[tokio::test]
async fn failed_v17_rolls_back_all_objects_and_preserves_original_claim_bytes() {
    let mut connection = connection().await;
    establish_runtime_version(&mut connection, 16).await;
    seed(&mut connection).await;
    let before = snapshots(&mut connection).await;
    assert!(
        migrate(
            &mut connection,
            OpenMode::ReadWriteExisting,
            &plan(17, true)
        )
        .await
        .is_err()
    );
    assert_eq!(pragma(&mut connection, "user_version").await, 16);
    assert_eq!(snapshots(&mut connection).await, before);
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sqlite_schema WHERE name LIKE 'radroots_runtime_authored_delivery_claims%' OR name LIKE 'radroots_runtime_authored_delivery_reconciliations%' OR name = 'radroots_runtime_authored_atomic_target_phase_idx'").fetch_one(&mut connection).await.unwrap(), 0);
    migrate(
        &mut connection,
        OpenMode::ReadWriteExisting,
        &plan(17, false),
    )
    .await
    .unwrap();
    assert_eq!(snapshots(&mut connection).await, before);
    connection.close().await.unwrap();
}

#[tokio::test]
async fn malformed_claim_namespaces_cannot_silently_disappear_during_backfill() {
    for wire in [
        "{}",
        "{!",
        r#"{"outcome":{"delivery_plan":null}}"#,
        r#"{"outcome":{"delivery_plan":{},"artifact":{}}}"#,
    ] {
        let mut connection = connection().await;
        establish_runtime_version(&mut connection, 16).await;
        super::signed_facts_tests::seed(&mut connection).await;
        sqlx::query("INSERT INTO radroots_runtime_authored_atomic_commits (commit_id, commit_digest, phase, target_id, requested_at_unix_ms, committed_at_unix_ms, receipt) VALUES (?, ?, 'claim', ?, 13, 13, ?)")
            .bind([9u8; 16].as_slice()).bind([8u8; 32].as_slice()).bind([3u8; 16].as_slice()).bind(wire.as_bytes()).execute(&mut connection).await.unwrap();
        let before = snapshots(&mut connection).await;
        assert!(
            migrate(
                &mut connection,
                OpenMode::ReadWriteExisting,
                &plan(17, false)
            )
            .await
            .is_err(),
            "{wire}"
        );
        assert_eq!(pragma(&mut connection, "user_version").await, 16);
        assert_eq!(snapshots(&mut connection).await, before);
        connection.close().await.unwrap();
    }
}

#[test]
fn delivery_reconciliation_decision_binds_exact_forward_migration() {
    let decision: serde_json::Value = serde_json::from_str(include_str!(
        "../../../contracts/architecture/decisions/authored_delivery_reconciliation.v1.json"
    ))
    .unwrap();
    let migration = runtime::MIGRATIONS[16];
    assert_eq!(decision["migration"]["version"], migration.version());
    assert_eq!(decision["migration"]["name"], migration.name());
    assert_eq!(decision["migration"]["sha256"], migration.up_sha256());
}

use super::{
    tests::{connection, establish_runtime_version, pragma},
    *,
};

const FAILING_V16: &str = concat!(
    include_str!("migration/runtime/0016_authored_delivery_facts.up.sql"),
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
                sql: if fail && step.version() == 16 {
                    FAILING_V16
                } else {
                    runtime::migration_sql(step.version()).unwrap()
                },
                owned_objects: step.owned_objects(),
            })
            .collect(),
    }
}

#[tokio::test]
async fn delivery_fact_upgrade_preserves_legacy_snapshots_receipts_and_cancelled_stop() {
    let mut connection = connection().await;
    establish_runtime_version(&mut connection, 15).await;
    let old = super::signed_facts_tests::seed(&mut connection).await;
    let bytes: Vec<u8> =
        sqlx::query_scalar("SELECT snapshot FROM radroots_runtime_authored_delivery_plans")
            .fetch_one(&mut connection)
            .await
            .unwrap();
    let mut wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    wire["state"] = serde_json::json!("cancelled");
    wire.as_object_mut().unwrap().remove("delivery_facts");
    wire.as_object_mut()
        .unwrap()
        .remove("stop_requested_at_unix_ms");
    let historical = serde_json::to_vec(&wire).unwrap();
    sqlx::query(
        "UPDATE radroots_runtime_authored_delivery_plans SET state = 'cancelled', snapshot = ?",
    )
    .bind(&historical)
    .execute(&mut connection)
    .await
    .unwrap();
    assert!(matches!(
        migrate_runtime(&mut connection, OpenMode::ReadOnly).await,
        Err(Error::SchemaMigrationRequired {
            actual: 15,
            current: 16,
            ..
        })
    ));
    assert_eq!(
        migrate_runtime(&mut connection, OpenMode::ReadWriteExisting)
            .await
            .unwrap()
            .applied(),
        1
    );
    assert_eq!(pragma(&mut connection, "user_version").await, 16);
    super::signed_facts_tests::retained(&mut connection, &old).await;
    let (actual, stop): (Vec<u8>, Option<i64>) = sqlx::query_as(
        "SELECT snapshot, stop_requested_at_unix_ms FROM radroots_runtime_authored_delivery_plans",
    )
    .fetch_one(&mut connection)
    .await
    .unwrap();
    assert_eq!(actual, historical);
    assert_eq!(stop, Some(10));
    let decoded: radroots_storage::authored_delivery::AuthoredDeliveryPlan =
        serde_json::from_slice(&actual).unwrap();
    assert_eq!(decoded.stop_requested_at_unix_ms(), Some(10));
    assert!(decoded.delivery_facts().is_empty());
    for mode in [OpenMode::ReadOnly, OpenMode::ReadWriteExisting] {
        assert!(matches!(
            migrate(&mut connection, mode, &plan(15, false)).await,
            Err(Error::SchemaTooNew {
                actual: 16,
                supported: 15,
                ..
            })
        ));
    }
    connection.close().await.unwrap();
}

#[tokio::test]
async fn failed_delivery_fact_migration_rolls_back_every_new_schema_object() {
    let mut connection = connection().await;
    establish_runtime_version(&mut connection, 15).await;
    let old = super::signed_facts_tests::seed(&mut connection).await;
    assert!(
        migrate(
            &mut connection,
            OpenMode::ReadWriteExisting,
            &plan(16, true)
        )
        .await
        .is_err()
    );
    assert_eq!(pragma(&mut connection, "user_version").await, 15);
    super::signed_facts_tests::retained(&mut connection, &old).await;
    assert!(
        sqlx::query(
            "SELECT stop_requested_at_unix_ms FROM radroots_runtime_authored_delivery_plans"
        )
        .fetch_all(&mut connection)
        .await
        .is_err()
    );
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sqlite_schema WHERE name LIKE 'radroots_runtime_authored_delivery_facts%' OR name = 'radroots_runtime_authored_delivery_stop_guard'").fetch_one(&mut connection).await.unwrap(), 0);
    migrate_runtime(&mut connection, OpenMode::ReadWriteExisting)
        .await
        .unwrap();
    super::signed_facts_tests::retained(&mut connection, &old).await;
    connection.close().await.unwrap();
}

#[test]
fn delivery_fact_decision_binds_unchanged_historical_migrations_and_exact_successor() {
    let decision: serde_json::Value = serde_json::from_str(include_str!(
        "../../../contracts/architecture/decisions/authored_delivery_facts.v1.json"
    ))
    .unwrap();
    let migration = runtime::MIGRATIONS[15];
    assert_eq!(decision["migration"]["version"], migration.version());
    assert_eq!(decision["migration"]["name"], migration.name());
    assert_eq!(decision["migration"]["sha256"], migration.up_sha256());
}

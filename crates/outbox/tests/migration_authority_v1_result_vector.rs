#![forbid(unsafe_code)]

use radroots_outbox::{
    RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT, RadrootsOutbox, RadrootsOutboxError,
    RadrootsOutboxSchemaStatus,
};
use serde::Deserialize;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::collections::BTreeSet;
use std::str::FromStr;

const VECTOR_BYTES: &[u8] = include_bytes!("fixtures/migration_authority.v1.json");
const BASELINE_UP: &str = include_str!("../migrations/0001_outbox.up.sql");
const SCHEMA_SOURCE: &str = include_str!("../src/schema.rs");
const MIGRATIONS_SOURCE: &str = include_str!("../src/migrations.rs");
const STORE_SOURCE: &str = include_str!("../src/store.rs");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Vector {
    schema_version: u32,
    contract_id: String,
    executor: Executor,
    delegated_suite: DelegatedSuite,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Executor {
    id: String,
    path: String,
    test: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DelegatedSuite {
    lane: String,
    package: String,
    authorities: Vec<Authority>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Authority {
    authority: String,
    authority_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    execution: String,
    expected_outcome: String,
    expected_error: Option<String>,
}

async fn memory_pool() -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::from_str("sqlite::memory:").expect("options"))
        .await
        .expect("pool")
}

async fn execute_case(case: &Case) {
    assert_eq!(case.execution, "direct_executor");
    match case.id.as_str() {
        "fresh_initialization" => {
            let store = RadrootsOutbox::open_memory().await.expect("fresh store");
            assert_eq!(
                store.schema_status().await.expect("status"),
                RadrootsOutboxSchemaStatus::Managed {
                    version: RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT,
                }
            );
            assert_eq!(case.expected_outcome, "managed_v1");
        }
        "exact_unledgered_adoption" => {
            let pool = memory_pool().await;
            sqlx::raw_sql(BASELINE_UP)
                .execute(&pool)
                .await
                .expect("baseline");
            let changes: i64 = sqlx::query_scalar("SELECT total_changes()")
                .fetch_one(&pool)
                .await
                .expect("changes");
            let store = RadrootsOutbox::open_pool(pool, false)
                .await
                .expect("adoption");
            let after: i64 = sqlx::query_scalar("SELECT total_changes()")
                .fetch_one(store.pool())
                .await
                .expect("after changes");
            assert_eq!(after - changes, 1, "only the ledger row is inserted");
            assert_eq!(case.expected_outcome, "managed_v1_without_replay");
        }
        "partial_unledgered_rejected" => {
            let pool = memory_pool().await;
            sqlx::raw_sql(BASELINE_UP)
                .execute(&pool)
                .await
                .expect("baseline");
            sqlx::query("DROP INDEX outbox_event_event_id_idx")
                .execute(&pool)
                .await
                .expect("partial schema");
            assert!(matches!(
                RadrootsOutbox::open_pool(pool, false).await,
                Err(RadrootsOutboxError::UnmanagedSchema { .. })
            ));
            assert_eq!(case.expected_outcome, "rejected_before_mutation");
            assert_eq!(case.expected_error.as_deref(), Some("UnmanagedSchema"));
        }
        "ledger_checksum_tamper_rejected" => {
            let store = RadrootsOutbox::open_memory().await.expect("store");
            sqlx::query(
                "UPDATE radroots_outbox_schema_migrations SET up_sha256 = ? WHERE version = 1",
            )
            .bind("b".repeat(64))
            .execute(store.pool())
            .await
            .expect("tamper");
            assert!(matches!(
                store.schema_status().await,
                Err(RadrootsOutboxError::MigrationHistoryChecksumDrift { .. })
            ));
            assert_eq!(case.expected_outcome, "rejected_before_mutation");
            assert_eq!(
                case.expected_error.as_deref(),
                Some("MigrationHistoryChecksumDrift")
            );
        }
        "newer_history_rejected" => {
            let store = RadrootsOutbox::open_memory().await.expect("store");
            sqlx::query(
                "INSERT INTO radroots_outbox_schema_migrations(version, name, up_sha256, down_sha256, schema_sha256) VALUES (2, 'future', ?, ?, ?)",
            )
            .bind("a".repeat(64))
            .bind("b".repeat(64))
            .bind("c".repeat(64))
            .execute(store.pool())
            .await
            .expect("future row");
            assert!(matches!(
                store.schema_status().await,
                Err(RadrootsOutboxError::SchemaTooNew { database: 2, .. })
            ));
            assert_eq!(case.expected_outcome, "rejected_before_mutation");
            assert_eq!(case.expected_error.as_deref(), Some("SchemaTooNew"));
        }
        "rollback_below_floor_rejected" => {
            let store = RadrootsOutbox::open_memory().await.expect("store");
            assert!(matches!(
                store.rollback_to_schema_version_and_close(0).await,
                Err(RadrootsOutboxError::RollbackBelowVersionFloor {
                    floor: 1,
                    target: 0
                })
            ));
            assert_eq!(case.expected_outcome, "rejected_without_schema_change");
            assert_eq!(
                case.expected_error.as_deref(),
                Some("RollbackBelowVersionFloor")
            );
        }
        "caller_state_preserved" => {
            let pool = memory_pool().await;
            sqlx::raw_sql(
                "CREATE TABLE caller_state(value TEXT NOT NULL); INSERT INTO caller_state VALUES ('preserved');",
            )
            .execute(&pool)
            .await
            .expect("caller state");
            let store = RadrootsOutbox::open_pool(pool, false)
                .await
                .expect("fresh migration");
            let value: String = sqlx::query_scalar("SELECT value FROM caller_state")
                .fetch_one(store.pool())
                .await
                .expect("caller value");
            assert_eq!(value, "preserved");
            assert_eq!(
                case.expected_outcome,
                "managed_v1_with_caller_state_preserved"
            );
        }
        "current_reopen_no_history_write" => {
            let store = RadrootsOutbox::open_memory().await.expect("store");
            let before: i64 = sqlx::query_scalar("SELECT total_changes()")
                .fetch_one(store.pool())
                .await
                .expect("before");
            store
                .migrate_to_current_schema()
                .await
                .expect("current reopen");
            let after: i64 = sqlx::query_scalar("SELECT total_changes()")
                .fetch_one(store.pool())
                .await
                .expect("after");
            assert_eq!(before, after);
            assert_eq!(case.expected_outcome, "managed_v1_without_history_write");
        }
        other => panic!("unknown direct vector case `{other}`"),
    }
    assert_eq!(case.expected_error.is_some(), case.id.contains("rejected"));
}

#[tokio::test]
async fn migration_authority_v1_result_vector() {
    let canonical =
        include_bytes!("../../../contracts/conformance/vectors/outbox/migration_authority.v1.json");
    assert_eq!(VECTOR_BYTES, canonical, "packaged vector mirror drift");
    let vector: Vector = serde_json::from_slice(VECTOR_BYTES).expect("vector JSON");
    assert_eq!(vector.schema_version, 1);
    assert_eq!(vector.contract_id, "radroots_outbox.migration_authority.v1");
    assert_eq!(
        vector.executor.id,
        "radroots_outbox.migration_authority_v1.result_vector_executor.v1"
    );
    assert_eq!(
        vector.executor.path,
        "crates/outbox/tests/migration_authority_v1_result_vector.rs"
    );
    assert_eq!(vector.executor.test, "migration_authority_v1_result_vector");
    assert_eq!(vector.delegated_suite.lane, "nix run .#contract");
    assert_eq!(vector.delegated_suite.package, "radroots_outbox");

    let mut authorities = BTreeSet::new();
    for authority in vector.delegated_suite.authorities {
        assert!(authorities.insert(authority.authority.clone()));
        let source = match authority.authority_path.as_str() {
            "crates/outbox/src/schema.rs" => SCHEMA_SOURCE,
            "crates/outbox/src/migrations.rs" => MIGRATIONS_SOURCE,
            "crates/outbox/src/store.rs" => STORE_SOURCE,
            other => panic!("unknown delegated authority path `{other}`"),
        };
        assert!(source.contains(authority.authority.as_str()));
    }

    let mut case_ids = BTreeSet::new();
    for case in &vector.cases {
        assert!(case_ids.insert(case.id.clone()), "duplicate case id");
        execute_case(case).await;
    }
    assert_eq!(case_ids.len(), 8);
}

#![forbid(unsafe_code)]
#![cfg(feature = "sqlite")]

use radroots_outbox::{
    RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT, RadrootsOutbox, RadrootsOutboxError,
    RadrootsOutboxSchemaStatus,
};
use serde::Deserialize;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const VECTOR_BYTES: &[u8] = include_bytes!("fixtures/migration_authority.v1.json");
const BASELINE_UP: &str = include_str!("../migrations/0001_outbox.up.sql");
const SCHEMA_SOURCE: &str = include_str!("../src/schema.rs");
const MIGRATIONS_SOURCE: &str = include_str!("../src/migrations.rs");

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

async fn direct_pool(path: &Path) -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
        .expect("direct pool")
}

fn database_path(case_id: &str) -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join(format!("{case_id}.sqlite"));
    (directory, path)
}

async fn assert_managed_current(store: &RadrootsOutbox) {
    assert_eq!(
        store.schema_status().await.expect("schema status"),
        RadrootsOutboxSchemaStatus::Managed {
            version: RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT,
        }
    );
}

async fn execute_case(case: &Case) {
    assert_eq!(case.execution, "direct_executor");
    let (_directory, path) = database_path(&case.id);
    match case.id.as_str() {
        "fresh_initialization" => {
            let store = RadrootsOutbox::open_file(&path).await.expect("fresh store");
            assert_managed_current(&store).await;
            assert_eq!(case.expected_outcome, "managed_current");
        }
        "exact_unledgered_adoption" => {
            let pool = direct_pool(&path).await;
            sqlx::raw_sql(BASELINE_UP)
                .execute(&pool)
                .await
                .expect("baseline");
            sqlx::query(
                "INSERT INTO outbox_operations(operation_kind, expected_pubkey, semantic_scope, trade_id, mutation_id, canonical_payload_sha256, idempotency_key, operation_idempotency_digest, status, created_at_ms, updated_at_ms) VALUES ('post', 'author', 'generic_event', NULL, NULL, NULL, NULL, ?, 'queued', 1, 1)",
            )
            .bind("a".repeat(64))
            .execute(&pool)
            .await
            .expect("legacy row");
            pool.close().await;

            let store = RadrootsOutbox::open_file(&path).await.expect("adoption");
            assert_managed_current(&store).await;
            drop(store);
            let pool = direct_pool(&path).await;
            let operations: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_operations")
                .fetch_one(&pool)
                .await
                .expect("legacy row count");
            let history: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM radroots_outbox_schema_migrations")
                    .fetch_one(&pool)
                    .await
                    .expect("history count");
            assert_eq!(operations, 1);
            assert_eq!(history, i64::from(RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT));
            assert_eq!(
                case.expected_outcome,
                "managed_current_without_replaying_0001"
            );
        }
        "partial_unledgered_rejected" => {
            let pool = direct_pool(&path).await;
            sqlx::raw_sql(BASELINE_UP)
                .execute(&pool)
                .await
                .expect("baseline");
            sqlx::query("DROP INDEX outbox_event_event_id_idx")
                .execute(&pool)
                .await
                .expect("partial schema");
            pool.close().await;
            assert!(matches!(
                RadrootsOutbox::open_file(&path).await,
                Err(RadrootsOutboxError::UnmanagedSchema { .. })
            ));
            let pool = direct_pool(&path).await;
            let ledgers: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = 'radroots_outbox_schema_migrations'",
            )
            .fetch_one(&pool)
            .await
            .expect("ledger count");
            assert_eq!(ledgers, 0);
            assert_eq!(case.expected_outcome, "rejected_before_ledger_mutation");
            assert_eq!(case.expected_error.as_deref(), Some("UnmanagedSchema"));
        }
        "ledger_checksum_tamper_rejected" => {
            let store = RadrootsOutbox::open_file(&path).await.expect("store");
            drop(store);
            let pool = direct_pool(&path).await;
            sqlx::query(
                "UPDATE radroots_outbox_schema_migrations SET up_sha256 = ? WHERE version = 1",
            )
            .bind("b".repeat(64))
            .execute(&pool)
            .await
            .expect("tamper");
            pool.close().await;
            assert!(matches!(
                RadrootsOutbox::open_file(&path).await,
                Err(RadrootsOutboxError::MigrationHistoryChecksumDrift { .. })
            ));
            assert_eq!(case.expected_outcome, "rejected_before_schema_mutation");
            assert_eq!(
                case.expected_error.as_deref(),
                Some("MigrationHistoryChecksumDrift")
            );
        }
        "newer_history_rejected" => {
            let store = RadrootsOutbox::open_file(&path).await.expect("store");
            drop(store);
            let pool = direct_pool(&path).await;
            let newer = i64::from(RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT) + 1;
            sqlx::query(
                "INSERT INTO radroots_outbox_schema_migrations(version, name, up_sha256, down_sha256, schema_sha256) VALUES (?, 'future', ?, ?, ?)",
            )
            .bind(newer)
            .bind("a".repeat(64))
            .bind("b".repeat(64))
            .bind("c".repeat(64))
            .execute(&pool)
            .await
            .expect("future row");
            pool.close().await;
            assert!(matches!(
                RadrootsOutbox::open_file(&path).await,
                Err(RadrootsOutboxError::SchemaTooNew { database, .. }) if database == newer
            ));
            assert_eq!(case.expected_outcome, "rejected_before_schema_mutation");
            assert_eq!(case.expected_error.as_deref(), Some("SchemaTooNew"));
        }
        "caller_state_preserved" => {
            let pool = direct_pool(&path).await;
            sqlx::raw_sql(
                "CREATE TABLE caller_state(value TEXT NOT NULL); INSERT INTO caller_state VALUES ('preserved');",
            )
            .execute(&pool)
            .await
            .expect("caller state");
            pool.close().await;
            let store = RadrootsOutbox::open_file(&path)
                .await
                .expect("fresh migration");
            assert_managed_current(&store).await;
            drop(store);
            let pool = direct_pool(&path).await;
            let value: String = sqlx::query_scalar("SELECT value FROM caller_state")
                .fetch_one(&pool)
                .await
                .expect("caller value");
            assert_eq!(value, "preserved");
            assert_eq!(
                case.expected_outcome,
                "managed_current_with_caller_state_preserved"
            );
        }
        "current_reopen_no_history_write" => {
            let store = RadrootsOutbox::open_file(&path).await.expect("store");
            assert_managed_current(&store).await;
            drop(store);
            let before = read_history(&path).await;
            let reopened = RadrootsOutbox::open_file(&path).await.expect("reopen");
            assert_managed_current(&reopened).await;
            drop(reopened);
            assert_eq!(read_history(&path).await, before);
            assert_eq!(
                case.expected_outcome,
                "managed_current_without_history_rewrite"
            );
        }
        other => panic!("unknown direct vector case `{other}`"),
    }
    assert_eq!(
        case.expected_error.is_some(),
        case.expected_outcome.starts_with("rejected")
    );
}

async fn read_history(path: &Path) -> Vec<(i64, String, String, String, String)> {
    let pool = direct_pool(path).await;
    let rows = sqlx::query_as(
        "SELECT version, name, up_sha256, down_sha256, schema_sha256 FROM radroots_outbox_schema_migrations ORDER BY version",
    )
    .fetch_all(&pool)
    .await
    .expect("history rows");
    pool.close().await;
    rows
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
            other => panic!("unknown delegated authority path `{other}`"),
        };
        assert!(source.contains(authority.authority.as_str()));
    }

    let mut case_ids = BTreeSet::new();
    for case in &vector.cases {
        assert!(case_ids.insert(case.id.clone()), "duplicate case id");
        execute_case(case).await;
    }
    assert_eq!(case_ids.len(), 7);
}

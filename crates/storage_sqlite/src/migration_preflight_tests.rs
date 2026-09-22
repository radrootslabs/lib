//! Public-open regressions use only synthetic owned databases.

use super::{tests, *};
use crate::{OpenOptions, Paths, SqliteStorage};
use std::path::Path;

async fn file_connection(path: &Path, create: bool) -> SqliteConnection {
    SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(create),
    )
    .await
    .expect("fixture connection")
}

async fn fixture(runtime_version: u32, private_version: u32) -> (tempfile::TempDir, Paths) {
    let directory = tempfile::tempdir().expect("temporary directory");
    let paths = Paths::from_directory(directory.path()).expect("owned paths");
    let mut runtime = file_connection(paths.runtime(), true).await;
    tests::establish_runtime_version(&mut runtime, runtime_version).await;
    sqlx::query("INSERT INTO radroots_runtime_source_generations (generation, state, created_at_unix_ms) VALUES (?, 'active', 10)")
        .bind([7_u8; 32].as_slice()).execute(&mut runtime).await.expect("retained generation");
    runtime.close().await.expect("close runtime fixture");
    let mut private = file_connection(paths.private(), true).await;
    tests::establish_private_version(&mut private, private_version).await;
    private.close().await.expect("close private fixture");
    (directory, paths)
}

async fn alter(path: &Path, sql: &'static str) {
    let mut connection = file_connection(path, false).await;
    sqlx::raw_sql(sql)
        .execute(&mut connection)
        .await
        .expect("fixture alteration");
    connection.close().await.expect("close fixture alteration");
}

async fn assert_refused_unchanged(paths: Paths, mode: OpenMode, expected: fn(&Error) -> bool) {
    let runtime = std::fs::read(paths.runtime()).expect("runtime bytes");
    let private = std::fs::read(paths.private()).expect("private bytes");
    for _ in 0..2 {
        let error = match SqliteStorage::open(OpenOptions::new(paths.clone(), mode)).await {
            Ok(store) => {
                store.close().await.expect("close unexpected store");
                panic!("incompatible pair opened")
            }
            Err(error) => error,
        };
        assert!(expected(&error), "unexpected error: {error:?}");
        assert!(
            std::fs::read(paths.runtime()).expect("retained runtime") == runtime,
            "runtime bytes changed"
        );
        assert!(
            std::fs::read(paths.private()).expect("retained private") == private,
            "private bytes changed"
        );
    }
}

#[tokio::test]
async fn future_private_schema_preserves_pending_runtime_and_both_original_files() {
    let (_directory, paths) = fixture(16, private::CURRENT_VERSION).await;
    alter(paths.private(), "PRAGMA user_version = 999").await;
    assert_refused_unchanged(paths, OpenMode::ReadWriteExisting, |error| {
        matches!(
            error,
            Error::SchemaTooNew {
                database: PRIVATE_DATABASE,
                actual: 999,
                ..
            }
        )
    })
    .await;
}

#[tokio::test]
async fn foreign_private_namespace_preserves_pending_runtime_and_both_original_files() {
    let (_directory, paths) = fixture(16, private::CURRENT_VERSION).await;
    alter(paths.private(), "PRAGMA application_id = 42").await;
    assert_refused_unchanged(paths, OpenMode::ReadWriteExisting, |error| {
        matches!(
            error,
            Error::SchemaIdentityMismatch {
                database: PRIVATE_DATABASE,
                actual: 42,
                ..
            }
        )
    })
    .await;
}

#[tokio::test]
async fn unexpected_private_catalog_preserves_pending_runtime_and_both_original_files() {
    let (_directory, paths) = fixture(16, private::CURRENT_VERSION).await;
    alter(paths.private(), "CREATE TABLE foreign_state (value TEXT)").await;
    assert_refused_unchanged(paths, OpenMode::ReadWriteExisting, |error| {
        matches!(
            error,
            Error::SchemaCatalogMismatch {
                database: PRIVATE_DATABASE,
                ..
            }
        )
    })
    .await;
}

#[tokio::test]
async fn future_runtime_schema_preserves_pending_private_and_both_original_files() {
    let (_directory, paths) = fixture(runtime::CURRENT_VERSION, 3).await;
    alter(paths.runtime(), "PRAGMA user_version = 999").await;
    assert_refused_unchanged(paths, OpenMode::ReadWriteExisting, |error| {
        matches!(
            error,
            Error::SchemaTooNew {
                database: RUNTIME_DATABASE,
                actual: 999,
                ..
            }
        )
    })
    .await;
}

#[tokio::test]
async fn read_only_pending_pair_preserves_both_original_files() {
    let (_directory, paths) = fixture(16, 3).await;
    assert_refused_unchanged(paths, OpenMode::ReadOnly, |error| {
        matches!(
            error,
            Error::SchemaMigrationRequired {
                database: RUNTIME_DATABASE,
                actual: 16,
                ..
            }
        )
    })
    .await;
}

#[tokio::test]
async fn compatible_pending_pair_migrates_and_reopens_with_same_generation() {
    use radroots_storage::EventStore;
    let (_directory, paths) = fixture(16, 3).await;
    for _ in 0..2 {
        let store =
            SqliteStorage::open(OpenOptions::new(paths.clone(), OpenMode::ReadWriteExisting))
                .await
                .expect("supported pair");
        assert_eq!(
            store
                .status()
                .await
                .expect("status")
                .generation()
                .as_bytes(),
            &[7; 32]
        );
        store.close().await.expect("close supported pair");
    }
    for (path, version) in [
        (paths.runtime(), runtime::CURRENT_VERSION),
        (paths.private(), private::CURRENT_VERSION),
    ] {
        let mut connection = file_connection(path, false).await;
        assert_eq!(
            tests::pragma(&mut connection, "user_version").await,
            i64::from(version)
        );
        connection.close().await.expect("close inspected fixture");
    }
}

#[tokio::test]
async fn unversioned_namespace_collision_preserves_both_original_files() {
    let (_directory, paths) = fixture(16, private::CURRENT_VERSION).await;
    alter(
        paths.private(),
        "PRAGMA user_version = 0; PRAGMA application_id = 0",
    )
    .await;
    assert_refused_unchanged(paths, OpenMode::ReadWriteExisting, |error| {
        matches!(
            error,
            Error::UnrecognizedSchema {
                database: PRIVATE_DATABASE
            }
        )
    })
    .await;
}

#[tokio::test]
async fn corrupt_private_member_preserves_pending_runtime_and_corrupt_evidence() {
    let (_directory, paths) = fixture(16, private::CURRENT_VERSION).await;
    std::fs::write(paths.private(), b"synthetic invalid SQLite evidence").expect("corrupt fixture");
    assert_refused_unchanged(paths, OpenMode::ReadWriteExisting, |error| {
        matches!(
            error,
            Error::DatabaseCorrupt {
                database: PRIVATE_DATABASE
            }
        )
    })
    .await;
}

#[tokio::test]
async fn incompatible_private_member_prevents_creation_of_missing_runtime() {
    use radroots_storage::event::SourceGeneration;
    let (_directory, paths) = fixture(16, private::CURRENT_VERSION).await;
    std::fs::remove_file(paths.runtime()).expect("remove synthetic runtime");
    alter(paths.private(), "PRAGMA user_version = 999").await;
    let before = std::fs::read(paths.private()).expect("private evidence");
    for _ in 0..2 {
        let options = OpenOptions::new(paths.clone(), OpenMode::Create)
            .with_source_generation(SourceGeneration::new([9; 32]).expect("generation"), 10)
            .expect("options");
        assert!(matches!(
            SqliteStorage::open(options).await,
            Err(Error::SchemaTooNew {
                database: PRIVATE_DATABASE,
                actual: 999,
                ..
            })
        ));
        assert!(!paths.runtime().exists());
        assert!(std::fs::read(paths.private()).expect("private retained") == before);
    }
}

#[tokio::test]
async fn ineligible_v10_authored_evidence_preserves_both_original_files() {
    let (_directory, paths) = fixture(10, 3).await;
    alter(
        paths.runtime(),
        "INSERT INTO radroots_runtime_journal_operations (
        instance_id, operation_id, idempotency_key, input_digest, prepared_at_unix_ms,
        revision, stage, cancellation_state, updated_at_unix_ms
    ) VALUES (zeroblob(16), x'ff', 'retained-invalid-journal', zeroblob(32), 10,
        1, 'prepared', 'not_requested', 10)",
    )
    .await;
    assert_refused_unchanged(paths, OpenMode::ReadWriteExisting, |error| {
        matches!(
            error,
            Error::AuthoredMigrationBlocked {
                invalid_or_unsupported: 1,
                ..
            }
        )
    })
    .await;
}

#[tokio::test]
async fn incompatible_generation_preserves_pending_schemas_and_original_files() {
    use radroots_storage::event::SourceGeneration;
    let (_directory, paths) = fixture(16, 3).await;
    let runtime = std::fs::read(paths.runtime()).expect("runtime evidence");
    let private = std::fs::read(paths.private()).expect("private evidence");
    for _ in 0..2 {
        let options = OpenOptions::new(paths.clone(), OpenMode::ReadWriteExisting)
            .with_source_generation(SourceGeneration::new([9; 32]).expect("generation"), 10)
            .expect("options");
        assert!(matches!(
            SqliteStorage::open(options).await,
            Err(Error::SourceGenerationMismatch)
        ));
        assert!(
            std::fs::read(paths.runtime()).expect("retained runtime") == runtime,
            "runtime bytes changed before rejecting generation"
        );
        assert!(
            std::fs::read(paths.private()).expect("retained private") == private,
            "private bytes changed before rejecting generation"
        );
    }
}

#[test]
fn unrelated_metadata_failure_retains_typed_metadata_diagnostic() {
    assert!(matches!(
        schema_metadata_error(&sqlx::Error::RowNotFound, PRIVATE_DATABASE),
        Error::SchemaMetadataUnavailable {
            database: PRIVATE_DATABASE
        }
    ));
}

#[tokio::test]
async fn unfingerprinted_private_v2_envelope_preserves_both_original_files() {
    let (_directory, paths) = fixture(16, 3).await;
    alter(
        paths.private(),
        "INSERT INTO radroots_private_artifacts (
        artifact_id, artifact_kind, schema_id, commitment, protected_size_bytes,
        secret_provider, secret_reference, key_version, envelope_version,
        encrypted_envelope, revision, stage, created_at_unix_ms, updated_at_unix_ms
    ) VALUES (zeroblob(16), 'trade.private_terms', 'trade.private_terms.v1', zeroblob(32), 1,
        'memory', 'synthetic-key', 1, 2, x'03', 1, 'active', 1, 1)",
    )
    .await;
    assert_refused_unchanged(paths, OpenMode::ReadWriteExisting, |error| {
        matches!(
            error,
            Error::SchemaMigrationFailed {
                database: PRIVATE_DATABASE,
                target_version: 4
            }
        )
    })
    .await;
}

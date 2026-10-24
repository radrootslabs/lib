use super::*;
use crate::workspace::operation;
use radroots_domain::{CommandContextData, OperationId};
use radroots_storage::command::{
    AdmissionOutcome, AdmissionRequest, OperationError, OperationJournal,
};
use serde_json::{Value, json};
use sqlx::{Connection, SqliteConnection};

#[tokio::test]
async fn protected_operation_read_refuses_each_inconsistent_payload_and_sql_projection_without_repair()
 {
    let fixture = CryptoFixture::new();
    let path = fixture
        ._temp
        .path()
        .canonicalize()
        .unwrap()
        .join("operation-records");
    let (store, selected, proof) = NativeWorkspace::initialize_local_outbox(
        &path,
        LocalOutboxConfig::new(
            participant(),
            participant(),
            "device",
            UnixSeconds::new(200).unwrap(),
        )
        .unwrap(),
        fixture.crypto(),
        Arc::new(FixedClock(100)),
    )
    .await
    .unwrap();
    let binding = store.authenticate(selected.clone(), &proof).await.unwrap();
    let operation_id = OperationId::from_bytes([11; 32]);
    let context = CommandContextData::new(
        operation_id,
        binding.actor(),
        selected.workspace_id(),
        selected.authority(),
        selected.route_digest(),
        None,
        None,
        UnixSeconds::new(150).unwrap(),
        None,
    )
    .unwrap();
    let wire = serde_json::to_vec(&json!({"type": "inbox.inquire", "context": context,
        "input": {"thread_id": "66".repeat(32), "peer": participant(), "source": null, "text": "synthetic private record"}})).unwrap();
    let request = AdmissionRequest::from_protected_wire(binding, &wire).unwrap();
    let copied = proof.with_secret(|secret| AttachmentProof::from_protected_input(secret).unwrap());
    let retained = match store.admit_command_and_hold(request, copied).await.unwrap() {
        AdmissionOutcome::Committed(snapshot) => snapshot,
        AdmissionOutcome::Unknown(_) => panic!("fixture preparation must confirm its commit"),
    };
    store.close().await.unwrap();
    let mut database = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path.join("execution_v1.sqlite"))
            .create_if_missing(false),
    )
    .await
    .unwrap();
    let original: Vec<u8> = sqlx::query_scalar("SELECT protected FROM operations")
        .fetch_one(&mut database)
        .await
        .unwrap();
    let crypto = fixture.crypto();
    let opened = crypto
        .open_operation(selected.workspace_id(), operation_id, &original)
        .await
        .unwrap();
    let value: Value = opened.expose_secret(|bytes| serde_json::from_slice(bytes).unwrap());
    assert_eq!(value["version"], 3);
    assert_eq!(value["scope"], "new_buyer_outbox");
    // Existing buyer preparations predate explicit scope. Reading one must
    // retain its original identity and bytes, never rewrite it as a new grant.
    let mut legacy = value.clone();
    legacy["version"] = json!(2);
    legacy.as_object_mut().unwrap().remove("scope");
    let legacy = crypto
        .seal_operation(
            selected.workspace_id(),
            operation_id,
            &serde_json::to_vec(&legacy).unwrap(),
        )
        .await
        .unwrap();
    sqlx::query("UPDATE operations SET protected=?")
        .bind(&legacy)
        .execute(&mut database)
        .await
        .unwrap();
    let (recovered, legacy_retained) =
        operation::read(&mut database, &selected, operation_id, &crypto)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(recovered.identity(), retained.identity());
    assert_eq!(
        recovered.request().binding().scope(),
        radroots_storage::workspace::WorkspaceScope::NewBuyerOutbox
    );
    assert_eq!(legacy_retained.bytes, legacy);
    assert_eq!(
        sqlx::query_scalar::<_, Vec<u8>>("SELECT protected FROM operations")
            .fetch_one(&mut database)
            .await
            .unwrap(),
        legacy
    );
    for (pointer, replacement) in [
        ("/version", json!(4)),
        ("/scope", json!("new_seller_outbox")),
        ("/prepared_at", json!("0")),
        ("/prepared_at", json!("99")),
        ("/first_issue_not_after", json!("100")),
        ("/first_issue_not_after", json!("151")),
        ("/revision", json!("0")),
        ("/revision", json!("2")),
        ("/selection/endpoint", json!("local://changed")),
        ("/command/context/operation_id", json!("77".repeat(32))),
    ] {
        let mut changed = value.clone();
        *changed.pointer_mut(pointer).unwrap() = replacement;
        let ciphertext = crypto
            .seal_operation(
                selected.workspace_id(),
                operation_id,
                &serde_json::to_vec(&changed).unwrap(),
            )
            .await
            .unwrap();
        sqlx::query("UPDATE operations SET protected=?")
            .bind(&ciphertext)
            .execute(&mut database)
            .await
            .unwrap();
        assert!(
            matches!(
                operation::read(&mut database, &selected, operation_id, &crypto).await,
                Err(OperationError::RecoveryRequired)
            ),
            "{pointer}"
        );
        let unchanged: Vec<u8> = sqlx::query_scalar("SELECT protected FROM operations")
            .fetch_one(&mut database)
            .await
            .unwrap();
        assert_eq!(unchanged, ciphertext);
    }
    // The following corruption is confined to this owned fixture database.
    // Reads must validate independently of SQLite's original CHECK constraints.
    sqlx::query("PRAGMA ignore_check_constraints=ON")
        .execute(&mut database)
        .await
        .unwrap();
    for assignment in [
        "UPDATE operations SET phase='other'",
        "UPDATE operations SET revision=2",
        "UPDATE operations SET actor=zeroblob(32)",
        "UPDATE operations SET grant_id=zeroblob(32)",
        "UPDATE operations SET command_digest=zeroblob(32)",
        "UPDATE operations SET transaction_id=zeroblob(32)",
        "UPDATE operations SET command_type='inbox.reply'",
        "UPDATE operations SET actor=zeroblob(31)",
        "UPDATE operations SET protected=zeroblob(262145)",
    ] {
        sqlx::query("UPDATE operations SET actor=?,grant_id=?,command_digest=?,transaction_id=?,command_type='inbox.inquire',phase='prepared',revision=1,protected=?")
            .bind(participant().as_bytes().as_slice()).bind(selected.grant_id().as_bytes().as_slice())
            .bind(retained.identity().command_digest().as_bytes().as_slice()).bind(retained.transaction_id().as_bytes().as_slice())
            .bind(&original).execute(&mut database).await.unwrap();
        sqlx::query(assignment)
            .execute(&mut database)
            .await
            .unwrap();
        assert!(
            matches!(
                operation::read(&mut database, &selected, operation_id, &crypto).await,
                Err(OperationError::RecoveryRequired)
            ),
            "{assignment}"
        );
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM operations")
            .fetch_one(&mut database)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }
    let mut transaction = database.begin_with("BEGIN IMMEDIATE").await.unwrap();
    assert!(matches!(
        operation::unchanged(
            &mut transaction,
            &retained,
            &operation::ProtectedOperation {
                bytes: original.clone(),
                creation: None
            }
        )
        .await,
        Err(OperationError::RecoveryRequired)
    ));
    transaction.rollback().await.unwrap();
    let length: i64 = sqlx::query_scalar("SELECT length(protected) FROM operations")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(length, 262145);
    let other =
        PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5")
            .unwrap();
    assert_eq!(
        operation::check_digest(
            &retained,
            ActorRef::new(other, ActorRole::Buyer, None, Some(selected.grant_id())),
            None
        ),
        Err(OperationError::Unauthorized)
    );
    // A retained ciphertext alone cannot confirm unchanged indexed facts.
    let mut different_digest = retained.identity().command_digest().into_bytes();
    different_digest[0] ^= 1;
    sqlx::query("UPDATE operations SET protected=?,command_digest=?")
        .bind(&original)
        .bind(different_digest.as_slice())
        .execute(&mut database)
        .await
        .unwrap();
    let mut transaction = database.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let checked = operation::unchanged(
        &mut transaction,
        &retained,
        &operation::ProtectedOperation {
            bytes: original.clone(),
            creation: None,
        },
    )
    .await;
    transaction.rollback().await.unwrap();
    database.close().await.unwrap();
    assert!(
        matches!(checked, Err(OperationError::RecoveryRequired)),
        "unchanged ciphertext must not hide changed indexed facts"
    );
}

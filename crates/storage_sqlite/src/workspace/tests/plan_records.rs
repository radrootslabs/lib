use super::*;
use crate::workspace::{operation, plan as plan_store};
use nostr::{JsonUtil, nips::nip44};
use radroots_domain::{CommandContextData, EnvelopeEventId, OperationId};
use radroots_protocol::{
    chat::{CapturedDmRoute, ChatReplay, ChatRumor, ChatSeal, ChatSigningPlan},
    command::{CapturedChatCommand, InquireInput},
};
use radroots_storage::command::{
    AdmissionOutcome, AdmissionRequest, OperationJournal, PlanBindingOutcome,
};
use serde_json::{Value, json};
use sqlx::{Connection, SqliteConnection};
use std::sync::atomic::{AtomicU64, Ordering};

struct MutableClock(AtomicU64);
impl WorkspaceClock for MutableClock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        UnixSeconds::new(self.0.load(Ordering::SeqCst)).map_err(|_| WorkspaceError::Unavailable)
    }
}

#[tokio::test]
async fn authenticated_plan_manifest_parts_and_transaction_comparisons_refuse_each_inconsistency() {
    let fixture = CryptoFixture::new();
    let path = fixture
        ._temp
        .path()
        .canonicalize()
        .unwrap()
        .join("plan-records");
    let clock = Arc::new(MutableClock(AtomicU64::new(100)));
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
        clock.clone(),
    )
    .await
    .unwrap();
    let binding = store.authenticate(selected.clone(), &proof).await.unwrap();
    let operation_id = OperationId::from_bytes([51; 32]);
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
    let command = CapturedChatCommand::inquire(
        context,
        InquireInput::new(
            radroots_domain::ThreadId::from_bytes([6; 32]),
            participant(),
            None,
            "protected native plan fixture",
        )
        .unwrap(),
    )
    .unwrap();
    let rumor = ChatRumor::from_command(&command, UnixSeconds::new(100).unwrap()).unwrap();
    let mut scalar = [0; 32];
    scalar[31] = 1;
    let keys = nostr::Keys::new(nostr::SecretKey::from_slice(&scalar).unwrap());
    let ciphertext = nip44::encrypt(
        keys.secret_key(),
        &keys.public_key(),
        rumor.json(),
        nip44::Version::V2,
    )
    .unwrap();
    let seal = ChatSeal::new(
        &rumor,
        participant(),
        UnixSeconds::new(99).unwrap(),
        &ciphertext,
    )
    .unwrap();
    let source = nostr::UnsignedEvent {
        id: None,
        pubkey: keys.public_key(),
        created_at: nostr::Timestamp::from_secs(99),
        kind: nostr::Kind::Custom(10050),
        tags: nostr::Tags::from_list(vec![
            nostr::Tag::parse(["relay", "wss://route.example/"]).unwrap(),
        ]),
        content: "public route metadata ".repeat(1000),
    }
    .sign_with_keys(&keys)
    .unwrap();
    let route = CapturedDmRoute::from_observed_descriptor(
        participant(),
        EnvelopeEventId::from_bytes(*source.id.as_bytes()),
        &source.as_json(),
        &["wss://route.example/".into()],
    )
    .unwrap();
    let plan = ChatSigningPlan::new(
        &command,
        UnixSeconds::new(100).unwrap(),
        vec![seal],
        vec![route],
        ChatReplay::NonReplayable,
    )
    .unwrap();
    let digest = *plan.digest();
    let original_wire = plan.protected_wire().to_vec();
    let rollback_plan = ChatSigningPlan::parse(&command, &original_wire).unwrap();
    let copied =
        || proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes).unwrap());
    assert!(matches!(
        store
            .admit_command_and_hold(
                AdmissionRequest::new(binding.clone(), command).unwrap(),
                copied()
            )
            .await
            .unwrap(),
        AdmissionOutcome::Committed(_)
    ));
    clock.0.store(90, Ordering::SeqCst);
    assert!(matches!(
        store
            .bind_signing_intents(binding.clone(), rollback_plan, copied())
            .await,
        Err(radroots_storage::command::OperationError::Workspace(
            WorkspaceError::Unavailable
        ))
    ));
    let mut unchanged = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path.join("execution_v1.sqlite"))
            .create_if_missing(false)
            .read_only(true),
    )
    .await
    .unwrap();
    let plans: i64 = sqlx::query_scalar("SELECT count(*) FROM signing_plans")
        .fetch_one(&mut unchanged)
        .await
        .unwrap();
    assert_eq!(plans, 0, "clock rollback cannot install a signing plan");
    unchanged.close().await.unwrap();
    clock.0.store(100, Ordering::SeqCst);
    assert!(matches!(
        store
            .bind_signing_intents(binding, plan, copied())
            .await
            .unwrap(),
        PlanBindingOutcome::Committed(_)
    ));
    store.close().await.unwrap();
    let mut database = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path.join("execution_v1.sqlite"))
            .create_if_missing(false)
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::query("PRAGMA ignore_check_constraints=ON")
        .execute(&mut database)
        .await
        .unwrap();
    let crypto = fixture.crypto();
    let original: Vec<u8> = sqlx::query_scalar("SELECT protected FROM signing_plans")
        .fetch_one(&mut database)
        .await
        .unwrap();
    let manifest: Value = crypto
        .open_plan_part(selected.workspace_id(), operation_id, &digest, 0, &original)
        .await
        .unwrap()
        .expose_secret(|bytes| serde_json::from_slice(bytes).unwrap());
    for (field, value) in [
        ("version", json!(2)),
        ("workspace", json!("99".repeat(32))),
        ("operation_id", json!("99".repeat(32))),
        ("command_digest", json!("99".repeat(32))),
        ("preparation_transaction", json!("99".repeat(32))),
        ("plan_digest", json!("99".repeat(32))),
        ("transaction_id", json!("99".repeat(32))),
        ("part_count", json!(24)),
        ("byte_count", json!(1)),
        ("bound_at", json!("0")),
    ] {
        let mut changed = manifest.clone();
        changed[field] = value;
        let protected = crypto
            .seal_plan_part(
                selected.workspace_id(),
                operation_id,
                &digest,
                0,
                &serde_json::to_vec(&changed).unwrap(),
            )
            .await
            .unwrap();
        sqlx::query("UPDATE signing_plans SET protected=?")
            .bind(&protected)
            .execute(&mut database)
            .await
            .unwrap();
        let prepared = operation::read(&mut database, &selected, operation_id, &crypto)
            .await
            .unwrap()
            .unwrap()
            .0;
        let error = plan_store::read(&mut database, &selected, prepared, &crypto)
            .await
            .err()
            .expect("inconsistent authenticated manifest must fail");
        assert_eq!(error.code(), "recovery_required", "{field}");
        let retained: Vec<u8> = sqlx::query_scalar("SELECT protected FROM signing_plans")
            .fetch_one(&mut database)
            .await
            .unwrap();
        assert_eq!(retained, protected);
    }
    sqlx::query("UPDATE signing_plans SET protected=?")
        .bind(&original)
        .execute(&mut database)
        .await
        .unwrap();
    let original_parts: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT part,protected FROM signing_plan_parts ORDER BY part")
            .fetch_all(&mut database)
            .await
            .unwrap();
    assert!(original_parts.len() > 1);
    for fault in 0..7 {
        sqlx::query("DELETE FROM signing_plan_parts")
            .execute(&mut database)
            .await
            .unwrap();
        for (part, protected) in &original_parts {
            sqlx::query("INSERT INTO signing_plan_parts(workspace,operation_id,part,protected) VALUES(?,?,?,?)").bind(selected.workspace_id().as_bytes().as_slice()).bind(operation_id.as_bytes().as_slice()).bind(part).bind(protected).execute(&mut database).await.unwrap();
        }
        match fault {
            0 => {
                sqlx::query("DELETE FROM signing_plan_parts WHERE part=1")
                    .execute(&mut database)
                    .await
                    .unwrap();
            }
            1 => {
                sqlx::query("UPDATE signing_plan_parts SET protected=zeroblob(0)")
                    .execute(&mut database)
                    .await
                    .unwrap();
            }
            2 => {
                sqlx::query(
                    "UPDATE signing_plan_parts SET protected=zeroblob(2097153) WHERE part=1",
                )
                .execute(&mut database)
                .await
                .unwrap();
            }
            3 => {
                sqlx::query("UPDATE signing_plan_parts SET part=0 WHERE part=1")
                    .execute(&mut database)
                    .await
                    .unwrap();
            }
            4 | 5 => {
                let mut bytes = if fault == 4 {
                    vec![0]
                } else {
                    original_wire[..16384].to_vec()
                };
                if fault == 5 {
                    bytes[b"radroots.chat.signing_plan.v1\0".len() + 32 * 3 + 8] = 1;
                }
                let protected = crypto
                    .seal_plan_part(selected.workspace_id(), operation_id, &digest, 1, &bytes)
                    .await
                    .unwrap();
                sqlx::query("UPDATE signing_plan_parts SET protected=? WHERE part=1")
                    .bind(protected)
                    .execute(&mut database)
                    .await
                    .unwrap();
            }
            6 => {
                sqlx::query("UPDATE signing_plans SET byte_count=1")
                    .execute(&mut database)
                    .await
                    .unwrap();
            }
            _ => unreachable!(),
        }
        let prepared = operation::read(&mut database, &selected, operation_id, &crypto)
            .await
            .unwrap()
            .unwrap()
            .0;
        let error = plan_store::read(&mut database, &selected, prepared, &crypto)
            .await
            .err()
            .expect("part inconsistency must fail");
        assert_eq!(error.code(), "recovery_required", "part fault {fault}");
        sqlx::query("UPDATE signing_plans SET byte_count=?")
            .bind(original_wire.len() as i64)
            .execute(&mut database)
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM signing_plan_parts")
        .execute(&mut database)
        .await
        .unwrap();
    for (part, protected) in &original_parts {
        sqlx::query(
            "INSERT INTO signing_plan_parts(workspace,operation_id,part,protected) VALUES(?,?,?,?)",
        )
        .bind(selected.workspace_id().as_bytes().as_slice())
        .bind(operation_id.as_bytes().as_slice())
        .bind(part)
        .bind(protected)
        .execute(&mut database)
        .await
        .unwrap();
    }
    let prepared = operation::read(&mut database, &selected, operation_id, &crypto)
        .await
        .unwrap()
        .unwrap()
        .0;
    let (prepared, value) = plan_store::read(&mut database, &selected, prepared, &crypto)
        .await
        .unwrap();
    let (_, _, _, protected) = value.unwrap();
    assert!(
        plan_store::unchanged(&mut database, &prepared, None)
            .await
            .is_err()
    );
    for fault in 0..4 {
        match fault {
            0 => {
                sqlx::query("UPDATE signing_plans SET transaction_id=zeroblob(32)")
                    .execute(&mut database)
                    .await
                    .unwrap();
            }
            1 => {
                sqlx::query("UPDATE signing_plan_parts SET protected=zeroblob(1) WHERE part=1")
                    .execute(&mut database)
                    .await
                    .unwrap();
            }
            2 => {
                sqlx::query("DELETE FROM signing_plan_parts WHERE part=1")
                    .execute(&mut database)
                    .await
                    .unwrap();
            }
            3 => {
                sqlx::query("DELETE FROM signing_plans")
                    .execute(&mut database)
                    .await
                    .unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            plan_store::unchanged(&mut database, &prepared, Some(&protected))
                .await
                .is_err(),
            "transaction fault {fault}"
        );
        if fault == 0 {
            sqlx::query("UPDATE signing_plans SET transaction_id=?")
                .bind(protected.transaction.as_bytes().as_slice())
                .execute(&mut database)
                .await
                .unwrap();
        }
        if fault == 1 {
            sqlx::query("UPDATE signing_plan_parts SET protected=? WHERE part=1")
                .bind(&original_parts[0].1)
                .execute(&mut database)
                .await
                .unwrap();
        }
        if fault == 2 {
            sqlx::query("INSERT INTO signing_plan_parts(workspace,operation_id,part,protected) VALUES(?,?,1,?)").bind(selected.workspace_id().as_bytes().as_slice()).bind(operation_id.as_bytes().as_slice()).bind(&original_parts[0].1).execute(&mut database).await.unwrap();
        }
    }
    assert!(
        plan_store::unchanged(&mut database, &prepared, None)
            .await
            .is_err()
    );
    // A missing T0 must not become absence or a replacement operation while
    // any original protected plan part remains. This is corrupted retained
    // state, even when the SQL schema itself still passes quick_check.
    let retained_command = prepared.request().command().protected_wire().to_vec();
    sqlx::query("DELETE FROM operations")
        .execute(&mut database)
        .await
        .unwrap();
    let orphaned: Vec<Vec<u8>> =
        sqlx::query_scalar("SELECT protected FROM signing_plan_parts ORDER BY part")
            .fetch_all(&mut database)
            .await
            .unwrap();
    assert!(!orphaned.is_empty());
    database.close().await.unwrap();
    let reopened = NativeWorkspace::open_existing(&path, selected.clone(), fixture.crypto(), clock)
        .await
        .unwrap();
    let binding = reopened
        .authenticate(selected.clone(), &proof)
        .await
        .unwrap();
    // Absence is operation-scoped: unrelated retained orphan parts must not
    // turn a genuinely unknown ID into an error or create a replacement.
    assert!(
        reopened
            .signing_plan(
                binding.clone(),
                OperationId::from_bytes([52; 32]),
                radroots_domain::CommandDigest::from_bytes([0; 32]),
                copied(),
            )
            .await
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        reopened
            .signing_plan(
                binding.clone(),
                operation_id,
                prepared.identity().command_digest(),
                copied(),
            )
            .await,
        Err(radroots_storage::command::OperationError::RecoveryRequired)
    ));
    let lookup = reopened
        .operation(binding.clone(), operation_id, None, copied())
        .await;
    let admission = reopened
        .admit_command_and_hold(
            AdmissionRequest::from_protected_wire(binding, &retained_command).unwrap(),
            copied(),
        )
        .await;
    reopened.close().await.unwrap();
    assert!(
        matches!(
            lookup,
            Err(radroots_storage::command::OperationError::RecoveryRequired)
        ),
        "orphan lookup must refuse, not report absence"
    );
    assert!(
        matches!(
            admission,
            Err(radroots_storage::command::OperationError::RecoveryRequired)
        ),
        "orphan state must not acquire a replacement preparation"
    );
    let mut unchanged = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path.join("execution_v1.sqlite"))
            .create_if_missing(false)
            .read_only(true),
    )
    .await
    .unwrap();
    let still_retained: Vec<Vec<u8>> =
        sqlx::query_scalar("SELECT protected FROM signing_plan_parts ORDER BY part")
            .fetch_all(&mut unchanged)
            .await
            .unwrap();
    assert_eq!(orphaned, still_retained);
    let operations: i64 = sqlx::query_scalar("SELECT count(*) FROM operations")
        .fetch_one(&mut unchanged)
        .await
        .unwrap();
    assert_eq!(operations, 0);
    unchanged.close().await.unwrap();
}

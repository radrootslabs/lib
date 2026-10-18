#![cfg(unix)]

#[path = "support/workspace_plans.rs"]
mod plans;

use radroots_domain::{
    CommandContextData, CommandDigest, CorrelationId, OperationId, PublicKey, UnixSeconds,
};
use radroots_secrets::{
    SecretId, SecretRef,
    envelope::Nonce,
    file::{FileOpenMode, FileProvider},
    id::{BackendKind, KeyVersion},
    provider::AccessPolicy,
    wrapping::SecretMaterial,
};
use radroots_storage::{
    command::{
        AdmissionOutcome, AdmissionRequest, OperationError, OperationJournal, PreparedOperation,
    },
    workspace::{
        AttachmentInput, AttachmentProof, WorkspaceBinding, WorkspaceClock, WorkspaceError,
    },
};
use radroots_storage_sqlite::workspace::{LocalOutboxConfig, NativeWorkspace, WorkspaceCrypto};
use serde_json::json;
use sqlx::{Connection, SqliteConnection};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

struct Pause {
    skip: usize,
    reached: mpsc::SyncSender<()>,
    resume: mpsc::Receiver<()>,
}
struct Clock {
    now: AtomicU64,
    pause: Mutex<Option<Pause>>,
}
impl Clock {
    fn arm(&self, skip: usize) -> (mpsc::Receiver<()>, mpsc::SyncSender<()>) {
        let (reached, receive) = mpsc::sync_channel(1);
        let (resume, released) = mpsc::sync_channel(1);
        *self.pause.lock().unwrap() = Some(Pause {
            skip,
            reached,
            resume: released,
        });
        (receive, resume)
    }
}
impl WorkspaceClock for Clock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        let pause = {
            let mut selected = self.pause.lock().unwrap();
            if let Some(pause) = selected.as_mut() {
                if pause.skip == 0 {
                    selected.take()
                } else {
                    pause.skip -= 1;
                    None
                }
            } else {
                None
            }
        };
        if let Some(pause) = pause {
            pause
                .reached
                .send(())
                .map_err(|_| WorkspaceError::Unavailable)?;
            pause
                .resume
                .recv_timeout(Duration::from_secs(10))
                .map_err(|_| WorkspaceError::Unavailable)?;
        }
        UnixSeconds::new(self.now.load(Ordering::SeqCst)).map_err(|_| WorkspaceError::Unavailable)
    }
}
struct Fixture {
    _root: tempfile::TempDir,
    path: PathBuf,
    provider: Arc<FileProvider>,
    clock: Arc<Clock>,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let base = root.path().canonicalize().unwrap();
        let provider = Arc::new(
            FileProvider::open(
                base.join("vault"),
                FileOpenMode::CreateNew,
                SecretMaterial::from_slice(&[0xa5; 32]).unwrap(),
            )
            .unwrap(),
        );
        provider
            .provision(
                &reference(),
                &SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
                Nonce::new([0x11; 24]),
            )
            .unwrap();
        Self {
            _root: root,
            path: base.join("outbox"),
            provider,
            clock: Arc::new(Clock {
                now: AtomicU64::new(100),
                pause: Mutex::new(None),
            }),
        }
    }
    fn crypto(&self) -> WorkspaceCrypto {
        WorkspaceCrypto::new(
            self.provider.clone(),
            reference(),
            SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
            AccessPolicy::standard(),
        )
        .unwrap()
    }
    async fn create(
        &self,
    ) -> (
        Arc<NativeWorkspace>,
        AttachmentInput,
        AttachmentProof,
        WorkspaceBinding,
    ) {
        self.create_with_expiry(200).await
    }
    async fn create_with_expiry(
        &self,
        expiry: u64,
    ) -> (
        Arc<NativeWorkspace>,
        AttachmentInput,
        AttachmentProof,
        WorkspaceBinding,
    ) {
        let (store, input, proof) = NativeWorkspace::initialize_local_outbox(
            &self.path,
            LocalOutboxConfig::new(
                key(),
                key(),
                "synthetic-device",
                UnixSeconds::new(expiry).unwrap(),
            )
            .unwrap(),
            self.crypto(),
            self.clock.clone(),
        )
        .await
        .unwrap();
        let binding = store.authenticate(input.clone(), &proof).await.unwrap();
        (Arc::new(store), input, proof, binding)
    }
    async fn database(&self) -> SqliteConnection {
        SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new()
                .filename(self.path.join("execution_v1.sqlite"))
                .create_if_missing(false),
        )
        .await
        .unwrap()
    }
    async fn reopen(&self, input: AttachmentInput) -> NativeWorkspace {
        NativeWorkspace::open_existing(&self.path, input, self.crypto(), self.clock.clone())
            .await
            .unwrap()
    }
}
fn reference() -> SecretRef {
    SecretRef::new(
        SecretId::parse("workspace-key").unwrap(),
        BackendKind::File,
        KeyVersion::new(1).unwrap(),
    )
}
fn key() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798").unwrap()
}
fn copy(proof: &AttachmentProof) -> AttachmentProof {
    proof.with_secret(|secret| AttachmentProof::from_protected_input(secret).unwrap())
}
fn request(binding: &WorkspaceBinding, id: u8, text: &str, correlation: &str) -> AdmissionRequest {
    request_until(binding, id, text, correlation, 150)
}
fn request_until(
    binding: &WorkspaceBinding,
    id: u8,
    text: &str,
    correlation: &str,
    not_after: u64,
) -> AdmissionRequest {
    let input = binding.input();
    let context = CommandContextData::new(
        OperationId::from_bytes([id; 32]),
        binding.actor(),
        input.workspace_id(),
        input.authority(),
        input.route_digest(),
        None,
        None,
        UnixSeconds::new(not_after).unwrap(),
        Some(CorrelationId::new(correlation).unwrap()),
    )
    .unwrap();
    let bytes = serde_json::to_vec(&json!({"type": "inbox.inquire", "context": context,
        "input": {"thread_id": "66".repeat(32), "peer": key(), "source": null, "text": text}}))
    .unwrap();
    AdmissionRequest::from_protected_wire(binding.clone(), &bytes).unwrap()
}
fn committed(outcome: AdmissionOutcome) -> PreparedOperation {
    match outcome {
        AdmissionOutcome::Committed(result) => result,
        AdmissionOutcome::Unknown(_) => panic!("unexpected unknown completion"),
    }
}
async fn reached(receiver: mpsc::Receiver<()>) {
    tokio::task::spawn_blocking(move || receiver.recv_timeout(Duration::from_secs(10)))
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn actual_commit_reopen_and_replay_preserve_original_identity_and_private_command() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let captured = request(
        &binding,
        1,
        "private-cabbage-inquiry-🧺",
        "original-observer",
    );
    let digest = captured.command().digest();
    let original = committed(
        store
            .admit_command_and_hold(captured, copy(&proof))
            .await
            .unwrap(),
    );
    let anchor = original.identity();
    assert_eq!(original.revision().get(), 1);
    assert_eq!(
        original.request().command().text(),
        "private-cabbage-inquiry-🧺"
    );
    assert_eq!(original.request().binding().input(), &input);
    assert!(
        store
            .operation(
                binding.clone(),
                OperationId::from_bytes([9; 32]),
                None,
                copy(&proof)
            )
            .await
            .unwrap()
            .is_none()
    );
    store.close().await.unwrap();
    for name in ["execution_v1.sqlite", "execution_v1.sqlite-wal"] {
        if let Ok(bytes) = std::fs::read(fixture.path.join(name)) {
            assert!(
                !bytes
                    .windows(b"private-cabbage-inquiry".len())
                    .any(|value| value == b"private-cabbage-inquiry")
            );
        }
    }
    fixture.clock.now.store(160, Ordering::SeqCst);
    let reopened = fixture.reopen(input).await;
    let retained = reopened
        .operation(
            binding.clone(),
            anchor.operation_id(),
            Some(digest),
            copy(&proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.identity(), anchor);
    let replay = committed(
        reopened
            .admit_command_and_hold(
                request(&binding, 1, "private-cabbage-inquiry-🧺", "new-observer"),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    assert_eq!(replay.identity(), anchor);
    assert_eq!(
        replay
            .request()
            .command()
            .context()
            .correlation_id()
            .unwrap()
            .as_str(),
        "original-observer"
    );
    assert!(matches!(
        reopened
            .admit_command_and_hold(request(&binding, 1, "changed", "new"), copy(&proof))
            .await,
        Err(OperationError::IdempotencyConflict)
    ));
    assert!(matches!(
        reopened
            .operation(
                binding.clone(),
                anchor.operation_id(),
                Some(CommandDigest::from_bytes([0; 32])),
                copy(&proof)
            )
            .await,
        Err(OperationError::IdempotencyConflict)
    ));
    assert!(matches!(
        reopened
            .admit_command_and_hold(
                request(&binding, 2, "new expired command", "new"),
                copy(&proof)
            )
            .await,
        Err(OperationError::DeadlineExceeded)
    ));
    let mut database = fixture.database().await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM operations")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(count, 1);
    database.close().await.unwrap();
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn current_proof_and_revocation_are_checked_for_admission_and_retained_queries() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let wrong = AttachmentProof::from_protected_input(&[0; 32]).unwrap();
    assert!(matches!(
        store
            .admit_command_and_hold(request(&binding, 3, "private", "one"), wrong)
            .await,
        Err(OperationError::Workspace(WorkspaceError::Unauthorized))
    ));
    committed(
        store
            .admit_command_and_hold(request(&binding, 3, "private", "one"), copy(&proof))
            .await
            .unwrap(),
    );
    store.revoke(input, &proof).await.unwrap();
    assert!(matches!(
        store
            .operation(
                binding.clone(),
                OperationId::from_bytes([3; 32]),
                None,
                copy(&proof)
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::GrantRevoked))
    ));
    assert!(matches!(
        store
            .admit_command_and_hold(request(&binding, 3, "private", "retry"), copy(&proof))
            .await,
        Err(OperationError::Workspace(WorkspaceError::GrantRevoked))
    ));
    store.close().await.unwrap();
}

#[tokio::test]
async fn transaction_time_clock_rejects_expiry_after_capture_and_rolls_back() {
    let fixture = Fixture::new();
    let (store, _, proof, binding) = fixture.create().await;
    let captured = request(&binding, 4, "late", "one");
    let (arrived, resume) = fixture.clock.arm(1);
    let owner = store.clone();
    let secret = copy(&proof);
    let task = tokio::spawn(async move { owner.admit_command_and_hold(captured, secret).await });
    reached(arrived).await;
    fixture.clock.now.store(150, Ordering::SeqCst);
    resume.send(()).unwrap();
    assert!(matches!(
        task.await.unwrap(),
        Err(OperationError::DeadlineExceeded)
    ));
    assert!(
        store
            .operation(
                binding,
                OperationId::from_bytes([4; 32]),
                None,
                copy(&proof)
            )
            .await
            .unwrap()
            .is_none()
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn lost_observer_does_not_cancel_admitted_transaction_and_close_drains_it() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let captured = request(&binding, 5, "retained after observer cancellation", "gone");
    let digest = captured.command().digest();
    let (arrived, resume) = fixture.clock.arm(1);
    let owner = store.clone();
    let secret = copy(&proof);
    let task = tokio::spawn(async move { owner.admit_command_and_hold(captured, secret).await });
    reached(arrived).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    resume.send(()).unwrap();
    store.close().await.unwrap();
    let reopened = fixture.reopen(input).await;
    let result = reopened
        .operation(
            binding,
            OperationId::from_bytes([5; 32]),
            Some(digest),
            copy(&proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        result.request().command().text(),
        "retained after observer cancellation"
    );
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn actual_sqlite_statement_failure_confirms_rollback_before_known_failure() {
    let fixture = Fixture::new();
    let (store, _, proof, binding) = fixture.create().await;
    let mut database = fixture.database().await;
    sqlx::raw_sql("CREATE TRIGGER fail_admission BEFORE INSERT ON operations BEGIN SELECT RAISE(ABORT,'isolated fixture'); END")
        .execute(&mut database).await.unwrap();
    assert!(matches!(
        store
            .admit_command_and_hold(request(&binding, 6, "rollback", "one"), copy(&proof))
            .await,
        Err(OperationError::StorageFailure)
    ));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM operations")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(count, 0);
    sqlx::raw_sql("DROP TRIGGER fail_admission")
        .execute(&mut database)
        .await
        .unwrap();
    committed(
        store
            .admit_command_and_hold(request(&binding, 6, "rollback", "two"), copy(&proof))
            .await
            .unwrap(),
    );
    database.close().await.unwrap();
    store.close().await.unwrap();
}

#[tokio::test]
async fn actual_sqlite_commit_failure_stays_unknown_until_safe_close_and_reopen() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let mut database = fixture.database().await;
    sqlx::raw_sql("CREATE TABLE deferred_fault(value INTEGER REFERENCES workspace_grant(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fail_commit AFTER INSERT ON operations BEGIN INSERT INTO deferred_fault VALUES(2); END")
        .execute(&mut database).await.unwrap();
    let captured = request(&binding, 7, "uncertain completion", "one");
    let digest = captured.command().digest();
    let anchor = match store
        .admit_command_and_hold(captured, copy(&proof))
        .await
        .unwrap()
    {
        AdmissionOutcome::Unknown(anchor) => anchor,
        AdmissionOutcome::Committed(_) => panic!("deferred constraint should fail COMMIT"),
    };
    assert_eq!(anchor.operation_id(), OperationId::from_bytes([7; 32]));
    assert_eq!(anchor.command_digest(), digest);
    assert!(anchor.transaction_id().is_some());
    assert!(matches!(
        store
            .operation(
                binding.clone(),
                anchor.operation_id(),
                Some(digest),
                copy(&proof)
            )
            .await,
        Err(OperationError::RecoveryRequired)
    ));
    assert!(matches!(
        store
            .admit_command_and_hold(request(&binding, 8, "blocked", "two"), copy(&proof))
            .await,
        Err(OperationError::RecoveryRequired)
    ));
    store.close().await.unwrap();
    sqlx::raw_sql("DROP TRIGGER fail_commit; DROP TABLE deferred_fault")
        .execute(&mut database)
        .await
        .unwrap();
    database.close().await.unwrap();
    let reopened = fixture.reopen(input).await;
    // No signer or transport has run in this Prepared-only component. This is
    // reconciliation of this original T0, not a general no-external-effect proof.
    assert!(
        reopened
            .operation(binding, anchor.operation_id(), Some(digest), copy(&proof))
            .await
            .unwrap()
            .is_none()
    );
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn protected_grant_is_rechecked_inside_t0_after_the_pretransaction_observation() {
    let fixture = Fixture::new();
    let (store, _, proof, binding) = fixture.create().await;
    let captured = request(&binding, 9, "fenced", "one");
    let mut database = fixture.database().await;
    let original: Vec<u8> = sqlx::query_scalar("SELECT protected FROM workspace_grant")
        .fetch_one(&mut database)
        .await
        .unwrap();
    let (arrived, resume) = fixture.clock.arm(0);
    let owner = store.clone();
    let secret = copy(&proof);
    let task = tokio::spawn(async move { owner.admit_command_and_hold(captured, secret).await });
    reached(arrived).await;
    sqlx::query("UPDATE workspace_grant SET protected=?")
        .bind([0; 8].as_slice())
        .execute(&mut database)
        .await
        .unwrap();
    resume.send(()).unwrap();
    assert!(matches!(
        task.await.unwrap(),
        Err(OperationError::RecoveryRequired)
    ));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM operations")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let retained: Vec<u8> = sqlx::query_scalar("SELECT protected FROM workspace_grant")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(retained, [0; 8]);
    sqlx::query("UPDATE workspace_grant SET protected=?")
        .bind(original)
        .execute(&mut database)
        .await
        .unwrap();
    database.close().await.unwrap();
    store.close().await.unwrap();
}

#[tokio::test]
async fn unqualified_history_and_approval_descriptors_never_become_admitted_commands() {
    let fixture = Fixture::new();
    let (store, _, proof, binding) = fixture.create().await;
    let base = request(&binding, 10, "untrusted selectors", "one");
    let wire: serde_json::Value = serde_json::from_slice(base.command().protected_wire()).unwrap();
    let mut reply = wire.clone();
    reply["type"] = json!("inbox.reply");
    reply["input"].as_object_mut().unwrap().remove("source");
    reply["input"]["parent"] = json!("11".repeat(32));
    let captured = AdmissionRequest::from_protected_wire(
        binding.clone(),
        &serde_json::to_vec(&reply).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        store.admit_command_and_hold(captured, copy(&proof)).await,
        Err(OperationError::MissingHistory)
    ));
    let mut approval = wire.clone();
    approval["context"]["approval"] =
        json!({"intent_digest": "22".repeat(32), "scope": "caller-supplied", "expires_at": "150"});
    let captured = AdmissionRequest::from_protected_wire(
        binding.clone(),
        &serde_json::to_vec(&approval).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        store.admit_command_and_hold(captured, copy(&proof)).await,
        Err(OperationError::UnverifiedApproval)
    ));
    let other = WorkspaceBinding::from_authenticated_host(
        binding.input().clone(),
        binding.actor(),
        "substituted-device",
        binding.grant_revision(),
        binding.expires_at(),
        binding.observed_at(),
    )
    .unwrap();
    assert!(matches!(
        store
            .admit_command_and_hold(request(&other, 10, "private", "one"), copy(&proof))
            .await,
        Err(OperationError::Unauthorized)
    ));
    for (actor, revision, expiry) in [
        (
            radroots_domain::ActorRef::new(
                binding.actor().participant(),
                radroots_domain::ActorRole::Seller,
                None,
                binding.actor().grant_id(),
            ),
            binding.grant_revision(),
            binding.expires_at(),
        ),
        (
            binding.actor(),
            radroots_domain::Sequence::new(2).unwrap(),
            binding.expires_at(),
        ),
        (
            binding.actor(),
            binding.grant_revision(),
            UnixSeconds::new(201).unwrap(),
        ),
    ] {
        let substituted = WorkspaceBinding::from_authenticated_host(
            binding.input().clone(),
            actor,
            binding.principal_id(),
            revision,
            expiry,
            binding.observed_at(),
        )
        .unwrap();
        assert!(matches!(
            store
                .admit_command_and_hold(request(&substituted, 10, "private", "one"), copy(&proof))
                .await,
            Err(OperationError::Unauthorized)
        ));
    }
    let mut expected = wire.clone();
    expected["context"]["expected"] = json!({"order_id": "33".repeat(32), "agreement": null,
        "seller_sequence": "0", "buyer_head": null, "checkpoint": binding.input().checkpoint()});
    let captured = AdmissionRequest::from_protected_wire(
        binding.clone(),
        &serde_json::to_vec(&expected).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        store.admit_command_and_hold(captured, copy(&proof)).await,
        Err(OperationError::MissingHistory)
    ));
    let mut beyond_grant = wire;
    beyond_grant["context"]["not_after"] = json!("201");
    let captured = AdmissionRequest::from_protected_wire(
        binding.clone(),
        &serde_json::to_vec(&beyond_grant).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        store.admit_command_and_hold(captured, copy(&proof)).await,
        Err(OperationError::DeadlineExceeded)
    ));
    let mut database = fixture.database().await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM operations")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(count, 0);
    database.close().await.unwrap();
    store.close().await.unwrap();
}

#[tokio::test]
async fn retained_ciphertext_is_bound_to_the_original_operation_and_never_repaired_by_query() {
    let fixture = Fixture::new();
    let (store, _, proof, binding) = fixture.create().await;
    committed(
        store
            .admit_command_and_hold(
                request(&binding, 11, "first private command", "one"),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    committed(
        store
            .admit_command_and_hold(
                request(&binding, 12, "second private command", "two"),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let mut database = fixture.database().await;
    let protected: Vec<u8> =
        sqlx::query_scalar("SELECT protected FROM operations WHERE operation_id=?")
            .bind([11; 32].as_slice())
            .fetch_one(&mut database)
            .await
            .unwrap();
    sqlx::query("UPDATE operations SET protected=? WHERE operation_id=?")
        .bind(&protected)
        .bind([12; 32].as_slice())
        .execute(&mut database)
        .await
        .unwrap();
    assert!(matches!(
        store
            .operation(
                binding,
                OperationId::from_bytes([12; 32]),
                None,
                copy(&proof)
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::RecoveryRequired))
    ));
    let unchanged: Vec<u8> =
        sqlx::query_scalar("SELECT protected FROM operations WHERE operation_id=?")
            .bind([12; 32].as_slice())
            .fetch_one(&mut database)
            .await
            .unwrap();
    assert_eq!(unchanged, protected);
    database.close().await.unwrap();
    store.close().await.unwrap();
}

#[tokio::test]
async fn predecessor_grant_only_schema_is_preserved_for_explicit_cutover() {
    let fixture = Fixture::new();
    let (store, input, _, _) = fixture.create().await;
    store.close().await.unwrap();
    let mut database = fixture.database().await;
    sqlx::raw_sql(
        "DROP TABLE chat_publication_work; DROP TABLE chat_messages; DROP TABLE seal_evidence_parts; DROP TABLE seal_evidence; DROP TABLE signing_attempts; DROP TABLE signing_plan_parts; DROP TABLE signing_plans; DROP INDEX operation_transaction_identity; DROP TABLE operations; PRAGMA user_version=1",
    )
    .execute(&mut database)
    .await
    .unwrap();
    database.close().await.unwrap();
    let path = fixture.path.join("execution_v1.sqlite");
    let before = std::fs::read(&path).unwrap();
    assert!(matches!(
        NativeWorkspace::open_existing(
            &fixture.path,
            input,
            fixture.crypto(),
            fixture.clock.clone()
        )
        .await,
        Err(WorkspaceError::RecoveryRequired)
    ));
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[tokio::test]
async fn bounded_admission_queue_refuses_overflow_and_drains_every_accepted_request() {
    let fixture = Fixture::new();
    let (store, _, proof, binding) = fixture.create().await;
    let (arrived, resume) = fixture.clock.arm(1);
    let owner = store.clone();
    let first = request(&binding, 64, "first", "one");
    let secret = copy(&proof);
    let blocked = tokio::spawn(async move { owner.admit_command_and_hold(first, secret).await });
    reached(arrived).await;
    let mut queued = Vec::new();
    for id in 1..=32 {
        let mut future =
            store.admit_command_and_hold(request(&binding, id, "queued", "one"), copy(&proof));
        assert!(futures::poll!(future.as_mut()).is_pending());
        queued.push(future);
    }
    assert!(matches!(
        store
            .admit_command_and_hold(request(&binding, 33, "overflow", "one"), copy(&proof))
            .await,
        Err(OperationError::Workspace(WorkspaceError::Unavailable))
    ));
    resume.send(()).unwrap();
    committed(blocked.await.unwrap().unwrap());
    store.close().await.unwrap();
    for future in queued {
        committed(future.await.unwrap());
    }
    let mut database = fixture.database().await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM operations")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(count, 33);
    database.close().await.unwrap();
    assert!(matches!(
        store
            .admit_command_and_hold(request(&binding, 65, "closed", "one"), copy(&proof))
            .await,
        Err(OperationError::Workspace(WorkspaceError::Closed))
    ));
}

#[tokio::test]
async fn retained_first_issue_window_never_restarts_on_reopen_or_replay() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create_with_expiry(2000).await;
    let original = committed(
        store
            .admit_command_and_hold(
                request_until(&binding, 66, "fixed window", "one", 900),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let anchor = original.identity();
    assert_eq!(original.issuance_window().prepared_at().get(), 100);
    assert_eq!(
        original.issuance_window().first_issue_not_after().get(),
        220
    );
    store.close().await.unwrap();
    fixture.clock.now.store(500, Ordering::SeqCst);
    let reopened = fixture.reopen(input).await;
    let recovered = reopened
        .operation(
            binding.clone(),
            anchor.operation_id(),
            Some(anchor.command_digest()),
            copy(&proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered.issuance_window(), original.issuance_window());
    let replay = committed(
        reopened
            .admit_command_and_hold(
                request_until(&binding, 66, "fixed window", "retry", 900),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    assert_eq!(replay.identity(), anchor);
    assert_eq!(replay.issuance_window(), original.issuance_window());
    let fresh = committed(
        reopened
            .admit_command_and_hold(
                request_until(&binding, 67, "new operation", "new", 900),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    assert_eq!(fresh.issuance_window().prepared_at().get(), 500);
    assert_eq!(fresh.issuance_window().first_issue_not_after().get(), 620);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn predecessor_preparation_schema_marker_is_preserved_without_inferring_a_window() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    committed(
        store
            .admit_command_and_hold(request(&binding, 70, "retained", "one"), copy(&proof))
            .await
            .unwrap(),
    );
    store.close().await.unwrap();
    let mut database = fixture.database().await;
    sqlx::query("PRAGMA user_version=2")
        .execute(&mut database)
        .await
        .unwrap();
    database.close().await.unwrap();
    let path = fixture.path.join("execution_v1.sqlite");
    let before = std::fs::read(&path).unwrap();
    assert!(matches!(
        NativeWorkspace::open_existing(
            &fixture.path,
            input,
            fixture.crypto(),
            fixture.clock.clone()
        )
        .await,
        Err(WorkspaceError::RecoveryRequired)
    ));
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[tokio::test]
async fn transaction_checks_first_window_expiry_and_clock_rollback_after_preparation() {
    let fixture = Fixture::new();
    let (store, _, proof, binding) = fixture.create_with_expiry(2000).await;
    for (id, changed_clock) in [(68, 220), (69, 90)] {
        fixture.clock.now.store(100, Ordering::SeqCst);
        let request = request_until(&binding, id, "clock boundary", "one", 900);
        let (arrived, resume) = fixture.clock.arm(1);
        let owner = store.clone();
        let secret = copy(&proof);
        let task = tokio::spawn(async move { owner.admit_command_and_hold(request, secret).await });
        reached(arrived).await;
        fixture.clock.now.store(changed_clock, Ordering::SeqCst);
        resume.send(()).unwrap();
        let result = task.await.unwrap();
        if changed_clock == 220 {
            assert!(matches!(result, Err(OperationError::DeadlineExceeded)));
        } else {
            assert!(matches!(
                result,
                Err(OperationError::Workspace(WorkspaceError::Unavailable))
            ));
        }
        fixture.clock.now.store(100, Ordering::SeqCst);
        assert!(
            store
                .operation(
                    binding.clone(),
                    OperationId::from_bytes([id; 32]),
                    None,
                    copy(&proof)
                )
                .await
                .unwrap()
                .is_none()
        );
    }
    store.close().await.unwrap();
}

#[tokio::test]
async fn new_preparation_refuses_clock_rollback_before_its_first_observation() {
    let fixture = Fixture::new();
    let (store, _, proof, binding) = fixture.create().await;
    fixture.clock.now.store(90, Ordering::SeqCst);
    assert!(matches!(
        store
            .admit_command_and_hold(request(&binding, 71, "clock rollback", "one"), copy(&proof))
            .await,
        Err(OperationError::Workspace(WorkspaceError::Unavailable))
    ));
    fixture.clock.now.store(100, Ordering::SeqCst);
    assert!(
        store
            .operation(binding, OperationId::from_bytes([71; 32]), None, proof)
            .await
            .unwrap()
            .is_none()
    );
    store.close().await.unwrap();
}

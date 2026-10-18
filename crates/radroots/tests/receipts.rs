#![cfg(all(unix, feature = "native"))]

#[path = "support/receipt_plans.rs"]
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

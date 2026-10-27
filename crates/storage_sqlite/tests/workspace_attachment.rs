#![cfg(unix)]

use radroots_domain::{
    ActorRole, AuthorityRef, CheckpointDigest, CheckpointRef, ExecutorId, Generation, GrantId,
    PublicKey, RouteDigest, UnixSeconds, WorkspaceId,
};
use radroots_secrets::{
    SecretId, SecretRef,
    envelope::Nonce,
    file::{FileOpenMode, FileProvider},
    id::{BackendKind, KeyVersion},
    provider::AccessPolicy,
    wrapping::SecretMaterial,
};
use radroots_storage::workspace::{
    AttachmentInput, AttachmentProof, WorkspaceClock, WorkspaceError,
};
use radroots_storage_sqlite::workspace::{LocalOutboxConfig, NativeWorkspace, WorkspaceCrypto};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

struct Clock(AtomicU64);
impl WorkspaceClock for Clock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        UnixSeconds::new(self.0.load(Ordering::SeqCst)).map_err(|_| WorkspaceError::Unavailable)
    }
}
struct Fixture {
    _temp: tempfile::TempDir,
    path: PathBuf,
    provider: Arc<FileProvider>,
    reference: SecretRef,
    clock: Arc<Clock>,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let provider = Arc::new(
            FileProvider::open(
                base.join("vault"),
                FileOpenMode::CreateNew,
                SecretMaterial::from_slice(&[0xA5; 32]).unwrap(),
            )
            .unwrap(),
        );
        let reference = SecretRef::new(
            SecretId::parse("workspace-key").unwrap(),
            BackendKind::File,
            KeyVersion::new(1).unwrap(),
        );
        provider
            .provision(
                &reference,
                &SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
                Nonce::new([0x11; 24]),
            )
            .unwrap();
        Self {
            _temp: temp,
            path: base.join("outbox"),
            provider,
            reference,
            clock: Arc::new(Clock(AtomicU64::new(100))),
        }
    }
    fn crypto(&self) -> WorkspaceCrypto {
        self.crypto_with([0x41; 32])
    }
    fn crypto_with(&self, key: [u8; 32]) -> WorkspaceCrypto {
        let reference = SecretRef::new(
            self.reference.id().clone(),
            self.reference.backend(),
            self.reference.key_version(),
        );
        WorkspaceCrypto::new(
            self.provider.clone(),
            reference,
            SecretMaterial::from_slice(&key).unwrap(),
            AccessPolicy::standard(),
        )
        .unwrap()
    }
    fn config(&self) -> LocalOutboxConfig {
        LocalOutboxConfig::new(
            key(),
            key(),
            "synthetic-device",
            UnixSeconds::new(200).unwrap(),
        )
        .unwrap()
    }
    async fn create(&self) -> (NativeWorkspace, AttachmentInput, AttachmentProof) {
        NativeWorkspace::initialize_local_outbox(
            &self.path,
            self.config(),
            self.crypto(),
            self.clock.clone(),
        )
        .await
        .unwrap()
    }
}
fn key() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798").unwrap()
}

#[tokio::test]
async fn same_size_schema_substitution_is_refused_without_repair() {
    use sqlx::Connection;

    for statement in [
        "DROP INDEX operation_transaction_identity; CREATE UNIQUE INDEX operation_transaction_identity ON operations(workspace,operation_id)",
        "DROP INDEX operation_transaction_identity; CREATE INDEX foreign_index ON operations(workspace,transaction_id)",
        "CREATE TRIGGER foreign_trigger BEFORE INSERT ON operations BEGIN SELECT RAISE(ABORT,'synthetic catalog fault'); END",
    ] {
        let fixture = Fixture::new();
        let (store, input, _) = fixture.create().await;
        store.close().await.unwrap();
        let database = fixture.path.join("execution_v1.sqlite");
        let options = sqlx::sqlite::SqliteConnectOptions::new().filename(&database);
        let mut connection = sqlx::SqliteConnection::connect_with(&options)
            .await
            .unwrap();
        sqlx::raw_sql(statement)
            .execute(&mut connection)
            .await
            .unwrap();
        connection.close().await.unwrap();
        let retained = std::fs::read(&database).unwrap();
        assert!(matches!(
            NativeWorkspace::open_existing(
                &fixture.path,
                input,
                fixture.crypto(),
                fixture.clock.clone(),
            )
            .await,
            Err(WorkspaceError::RecoveryRequired)
        ));
        assert_eq!(std::fs::read(&database).unwrap(), retained);
    }
}
fn second_key() -> PublicKey {
    PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5").unwrap()
}
fn altered(input: &AttachmentInput, field: u8) -> AttachmentInput {
    AttachmentInput::new(
        if field == 0 {
            WorkspaceId::from_bytes([9; 32])
        } else {
            input.workspace_id()
        },
        if field == 1 {
            AuthorityRef::new(
                ExecutorId::from_bytes([9; 32]),
                input.authority().generation(),
            )
        } else if field == 2 {
            AuthorityRef::new(input.authority().executor_id(), Generation::new(2).unwrap())
        } else {
            input.authority()
        },
        if field == 3 {
            second_key()
        } else {
            input.participant()
        },
        if field == 4 {
            "local://other"
        } else {
            input.endpoint()
        },
        if field == 5 {
            second_key()
        } else {
            input.endpoint_identity()
        },
        if field == 6 {
            GrantId::from_bytes([9; 32])
        } else {
            input.grant_id()
        },
        if field == 7 {
            CheckpointRef::new(
                CheckpointDigest::from_bytes([9; 32]),
                input.checkpoint().sequence(),
                input.checkpoint().coverage(),
                vec![],
            )
            .unwrap()
        } else {
            input.checkpoint().clone()
        },
        if field == 8 {
            RouteDigest::from_bytes([9; 32])
        } else {
            input.route_digest()
        },
    )
    .unwrap()
}

#[tokio::test]
async fn actual_protected_outbox_authenticates_exact_selection_and_reopens() {
    let fixture = Fixture::new();
    let (store, input, proof) = fixture.create().await;
    let binding = store.authenticate(input.clone(), &proof).await.unwrap();
    assert_eq!(binding.actor().role(), ActorRole::Buyer);
    assert_eq!(binding.actor().grant_id(), Some(input.grant_id()));
    assert_eq!(binding.principal_id(), "synthetic-device");
    assert_eq!(binding.observed_at().get(), 100);
    assert_eq!(binding.grant_revision().get(), 1);
    assert_eq!(input.checkpoint().sequence().get(), 0);
    assert_eq!(
        input.checkpoint().coverage(),
        radroots_domain::Coverage::Partial
    );
    for field in 0..9 {
        assert!(
            store
                .authenticate(altered(&input, field), &proof)
                .await
                .is_err(),
            "field {field}"
        );
    }
    let wrong = AttachmentProof::from_protected_input(&[0; 32]).unwrap();
    assert_eq!(
        store.authenticate(input.clone(), &wrong).await.unwrap_err(),
        WorkspaceError::Unauthorized
    );
    assert!(matches!(
        NativeWorkspace::open_existing(
            &fixture.path,
            input.clone(),
            fixture.crypto(),
            fixture.clock.clone()
        )
        .await,
        Err(WorkspaceError::WriterActive)
    ));
    let mut files = std::fs::read_dir(&fixture.path)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    files.push(fixture.path.join("execution_v1.sqlite"));
    for path in files {
        if !path.is_file() {
            continue;
        }
        let bytes = std::fs::read(path).unwrap();
        assert!(
            !bytes
                .windows(b"synthetic-device".len())
                .any(|window| window == b"synthetic-device")
        );
        proof.with_secret(|secret| assert!(!bytes.windows(32).any(|window| window == secret)));
    }
    assert_eq!(format!("{proof:?}"), "AttachmentProof(<redacted>)");
    assert_eq!(format!("{store:?}"), "NativeWorkspace(<redacted>)");
    store.close().await.unwrap();
    store.close().await.unwrap();
    assert_eq!(
        store.authenticate(input.clone(), &proof).await.unwrap_err(),
        WorkspaceError::Closed
    );
    let reopened = NativeWorkspace::open_existing(
        &fixture.path,
        input.clone(),
        fixture.crypto(),
        fixture.clock.clone(),
    )
    .await
    .unwrap();
    reopened.authenticate(input.clone(), &proof).await.unwrap();
    reopened.close().await.unwrap();
    assert!(matches!(
        NativeWorkspace::open_existing(
            &fixture.path,
            input,
            fixture.crypto_with([0x42; 32]),
            fixture.clock.clone()
        )
        .await,
        Err(WorkspaceError::Unavailable)
    ));
}

#[tokio::test]
async fn actual_revocation_and_expiry_survive_reopen_without_reenrollment() {
    let fixture = Fixture::new();
    let (store, input, proof) = fixture.create().await;
    fixture.clock.0.store(200, Ordering::SeqCst);
    assert_eq!(
        store.authenticate(input.clone(), &proof).await.unwrap_err(),
        WorkspaceError::GrantExpired
    );
    fixture.clock.0.store(0, Ordering::SeqCst);
    assert_eq!(
        store.authenticate(input.clone(), &proof).await.unwrap_err(),
        WorkspaceError::Unavailable
    );
    fixture.clock.0.store(101, Ordering::SeqCst);
    store.revoke(input.clone(), &proof).await.unwrap();
    assert_eq!(
        store.authenticate(input.clone(), &proof).await.unwrap_err(),
        WorkspaceError::GrantRevoked
    );
    store.close().await.unwrap();
    let reopened = NativeWorkspace::open_existing(
        &fixture.path,
        input.clone(),
        fixture.crypto(),
        fixture.clock.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        reopened.authenticate(input, &proof).await.unwrap_err(),
        WorkspaceError::GrantRevoked
    );
    reopened.close().await.unwrap();
    assert!(
        NativeWorkspace::initialize_local_outbox(
            &fixture.path,
            fixture.config(),
            fixture.crypto(),
            fixture.clock.clone()
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn existing_open_never_creates_or_accepts_wrong_namespace_or_ciphertext() {
    let fixture = Fixture::new();
    let (store, input, proof) = fixture.create().await;
    store.close().await.unwrap();
    assert!(matches!(
        NativeWorkspace::open_existing(
            &fixture.path,
            altered(&input, 0),
            fixture.crypto(),
            fixture.clock.clone()
        )
        .await,
        Err(WorkspaceError::WrongNamespace)
    ));
    let missing = fixture.path.with_file_name("missing");
    assert!(
        NativeWorkspace::open_existing(
            &missing,
            input.clone(),
            fixture.crypto(),
            fixture.clock.clone()
        )
        .await
        .is_err()
    );
    assert!(!missing.exists());
    use sqlx::Connection;
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(fixture.path.join("execution_v1.sqlite"));
    let mut connection = sqlx::SqliteConnection::connect_with(&options)
        .await
        .unwrap();
    sqlx::query("UPDATE workspace_grant SET protected=zeroblob(1024)")
        .execute(&mut connection)
        .await
        .unwrap();
    connection.close().await.unwrap();
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
    assert_eq!(format!("{proof:?}"), "AttachmentProof(<redacted>)");
}

struct PausedClock {
    state: std::sync::Mutex<(bool, bool, bool)>,
    changed: std::sync::Condvar,
}
impl PausedClock {
    fn arm(&self) {
        *self.state.lock().unwrap() = (true, false, false);
    }
    fn entered(&self) {
        let state = self.state.lock().unwrap();
        let (state, timeout) = self
            .changed
            .wait_timeout_while(state, std::time::Duration::from_secs(5), |state| !state.1)
            .unwrap();
        assert!(
            state.1 && !timeout.timed_out(),
            "worker did not reach injected clock"
        );
    }
    fn release(&self) {
        self.state.lock().unwrap().2 = true;
        self.changed.notify_all();
    }
}
impl WorkspaceClock for PausedClock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        let mut state = self.state.lock().unwrap();
        if state.0 {
            state.1 = true;
            self.changed.notify_all();
            while !state.2 {
                state = self.changed.wait(state).unwrap();
            }
            state.0 = false;
        }
        Ok(UnixSeconds::new(100).unwrap())
    }
}
#[tokio::test]
async fn cancelled_authentication_and_close_keep_writer_until_owned_work_drains() {
    let fixture = Fixture::new();
    let clock = Arc::new(PausedClock {
        state: std::sync::Mutex::new((false, false, false)),
        changed: std::sync::Condvar::new(),
    });
    let (store, input, proof) = NativeWorkspace::initialize_local_outbox(
        &fixture.path,
        fixture.config(),
        fixture.crypto(),
        clock.clone(),
    )
    .await
    .unwrap();
    clock.arm();
    let mut auth = Box::pin(store.authenticate(input.clone(), &proof));
    assert!(futures::poll!(auth.as_mut()).is_pending());
    clock.entered();
    let mut closing = Box::pin(store.close());
    assert!(futures::poll!(closing.as_mut()).is_pending());
    drop(auth);
    drop(closing);
    assert!(matches!(
        NativeWorkspace::open_existing(
            &fixture.path,
            input.clone(),
            fixture.crypto(),
            fixture.clock.clone()
        )
        .await,
        Err(WorkspaceError::WriterActive)
    ));
    clock.release();
    store.close().await.unwrap();
    let reopened = NativeWorkspace::open_existing(
        &fixture.path,
        input.clone(),
        fixture.crypto(),
        fixture.clock.clone(),
    )
    .await
    .unwrap();
    reopened.authenticate(input, &proof).await.unwrap();
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn unsafe_links_permissions_and_schema_are_refused_without_repair() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let fixture = Fixture::new();
    let (store, input, _proof) = fixture.create().await;
    store.close().await.unwrap();
    let database = fixture.path.join("execution_v1.sqlite");
    let retained = fixture.path.join("retained.sqlite");
    std::fs::rename(&database, &retained).unwrap();
    symlink(&retained, &database).unwrap();
    assert!(
        NativeWorkspace::open_existing(
            &fixture.path,
            input.clone(),
            fixture.crypto(),
            fixture.clock.clone()
        )
        .await
        .is_err()
    );
    assert!(
        std::fs::symlink_metadata(&database)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    std::fs::remove_file(&database).unwrap();
    std::fs::rename(&retained, &database).unwrap();
    std::fs::set_permissions(&database, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        NativeWorkspace::open_existing(
            &fixture.path,
            input.clone(),
            fixture.crypto(),
            fixture.clock.clone()
        )
        .await
        .is_err()
    );
    assert_eq!(
        std::fs::metadata(&database).unwrap().permissions().mode() & 0o777,
        0o644
    );
    std::fs::set_permissions(&database, std::fs::Permissions::from_mode(0o600)).unwrap();
    use sqlx::Connection;
    let options = sqlx::sqlite::SqliteConnectOptions::new().filename(&database);
    let mut connection = sqlx::SqliteConnection::connect_with(&options)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE unrelated(value TEXT)")
        .execute(&mut connection)
        .await
        .unwrap();
    connection.close().await.unwrap();
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
}

#[path = "support/stock_abort_port.rs"]
mod stock_abort_port;
#[path = "support/stock_authority.rs"]
mod stock_authority;
#[path = "support/stock_envelope_port.rs"]
mod stock_envelope_port;

#[path = "support/pool_creation.rs"]
mod pool_creation;

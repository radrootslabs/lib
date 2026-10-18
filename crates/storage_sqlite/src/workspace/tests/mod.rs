use super::*;
mod operation_records;
mod plan_records;
use radroots_domain::{ActorRef, SourceId};
use radroots_secrets::{
    EncryptedEnvelope, SecretId, SecretRef,
    context::{EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId},
    envelope::{Nonce, SealMaterial, SealRequest},
    file::{FileOpenMode, FileProvider},
    id::{BackendKind, KeyVersion},
    memory::MemoryProvider,
    provider::{AccessPolicy, SecretProvider},
    wrapping::SecretMaterial,
};
use std::{
    fs as host_fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt, symlink},
};

fn participant() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798").unwrap()
}
fn selection() -> AttachmentInput {
    AttachmentInput::new(
        WorkspaceId::from_bytes([1; 32]),
        AuthorityRef::new(ExecutorId::from_bytes([2; 32]), Generation::new(1).unwrap()),
        participant(),
        "local://synthetic",
        participant(),
        GrantId::from_bytes([3; 32]),
        CheckpointRef::new(
            CheckpointDigest::from_bytes([4; 32]),
            Sequence::new(0).unwrap(),
            Coverage::Partial,
            vec![SourceId::from_bytes([5; 32])],
        )
        .unwrap(),
        RouteDigest::from_bytes([6; 32]),
    )
    .unwrap()
}
fn reference(id: &str, backend: BackendKind, version: u32) -> SecretRef {
    SecretRef::new(
        SecretId::parse(id).unwrap(),
        backend,
        KeyVersion::new(version).unwrap(),
    )
}
struct CryptoFixture {
    _temp: tempfile::TempDir,
    provider: Arc<FileProvider>,
}
impl CryptoFixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let provider = Arc::new(
            FileProvider::open(
                root.join("vault"),
                FileOpenMode::CreateNew,
                SecretMaterial::from_slice(&[0xA5; 32]).unwrap(),
            )
            .unwrap(),
        );
        let fixture = Self {
            _temp: temp,
            provider,
        };
        fixture.provision(&reference("grant-key", BackendKind::File, 1), 0x11);
        fixture
    }
    fn provision(&self, reference: &SecretRef, nonce: u8) {
        self.provider
            .provision(
                reference,
                &SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
                Nonce::new([nonce; 24]),
            )
            .unwrap();
    }
    fn crypto(&self) -> WorkspaceCrypto {
        WorkspaceCrypto::new(
            self.provider.clone(),
            reference("grant-key", BackendKind::File, 1),
            SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
            AccessPolicy::standard(),
        )
        .unwrap()
    }
}
async fn envelope(
    provider: &dyn SecretProvider,
    reference: SecretRef,
    plaintext: &[u8],
    nonce: u8,
) -> Vec<u8> {
    let input = selection();
    let context = EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.workspace.grant").unwrap(),
        EnvelopeSubject::parse(
            "workspace_grant",
            format!("{}{}", input.workspace_id(), input.grant_id()),
        )
        .unwrap(),
        PayloadSchemaId::parse("radroots.workspace.grant.v1").unwrap(),
    );
    let plaintext = SecretMaterial::from_slice(plaintext).unwrap();
    EncryptedEnvelope::seal(
        provider,
        SealRequest::new(
            reference,
            context,
            &plaintext,
            SealMaterial::new(
                SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
                Nonce::new([nonce; 24]),
            ),
        ),
    )
    .await
    .unwrap()
    .encode()
    .unwrap()
}

#[tokio::test]
async fn protected_payload_enforces_native_limits_reference_and_context_after_real_crypto() {
    let fixture = CryptoFixture::new();
    let input = selection();
    assert!(matches!(
        WorkspaceCrypto::new(
            fixture.provider.clone(),
            reference("grant-key", BackendKind::File, 1),
            SecretMaterial::from_slice(&[1; 31]).unwrap(),
            AccessPolicy::standard()
        ),
        Err(WorkspaceError::Unavailable)
    ));
    assert!(matches!(
        WorkspaceCrypto::new(
            Arc::new(MemoryProvider::new()),
            reference("grant-key", BackendKind::Memory, 1),
            SecretMaterial::from_slice(&[1; 32]).unwrap(),
            AccessPolicy::standard()
        ),
        Err(WorkspaceError::Unavailable)
    ));
    let crypto = fixture.crypto();
    let boundary = vec![b'x'; crypto::PRIVATE_PAYLOAD_MAX_BYTES];
    let sealed = crypto
        .seal(input.workspace_id(), input.grant_id(), &boundary)
        .await
        .unwrap();
    crypto
        .open(input.workspace_id(), input.grant_id(), &sealed)
        .await
        .unwrap()
        .expose_secret(|value| assert_eq!(value, boundary));
    assert!(matches!(
        crypto
            .open(WorkspaceId::from_bytes([9; 32]), input.grant_id(), &sealed)
            .await,
        Err(WorkspaceError::RecoveryRequired)
    ));
    assert!(matches!(
        crypto
            .open(input.workspace_id(), GrantId::from_bytes([9; 32]), &sealed)
            .await,
        Err(WorkspaceError::RecoveryRequired)
    ));
    let oversized = vec![b'x'; crypto::PRIVATE_PAYLOAD_MAX_BYTES + 1];
    assert!(matches!(
        crypto
            .seal(input.workspace_id(), input.grant_id(), &oversized)
            .await,
        Err(WorkspaceError::InvalidInput)
    ));
    assert!(matches!(
        crypto
            .open(
                input.workspace_id(),
                input.grant_id(),
                &vec![0; crypto::PROTECTED_MAX_BYTES + 1]
            )
            .await,
        Err(WorkspaceError::RecoveryRequired)
    ));
    let sealed = envelope(
        fixture.provider.as_ref(),
        reference("grant-key", BackendKind::File, 1),
        &oversized,
        0x61,
    )
    .await;
    assert!(matches!(
        crypto
            .open(input.workspace_id(), input.grant_id(), &sealed)
            .await,
        Err(WorkspaceError::RecoveryRequired)
    ));
    for (id, version, nonce) in [("other-key", 1, 0x12), ("grant-key", 2, 0x13)] {
        fixture.provision(&reference(id, BackendKind::File, version), nonce);
        let sealed = envelope(
            fixture.provider.as_ref(),
            reference(id, BackendKind::File, version),
            b"private",
            nonce + 0x60,
        )
        .await;
        assert!(matches!(
            crypto
                .open(input.workspace_id(), input.grant_id(), &sealed)
                .await,
            Err(WorkspaceError::RecoveryRequired)
        ));
    }
    let memory = MemoryProvider::new();
    memory
        .provision(
            &reference("grant-key", BackendKind::Memory, 1),
            SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
        )
        .unwrap();
    let sealed = envelope(
        &memory,
        reference("grant-key", BackendKind::Memory, 1),
        b"private",
        0x64,
    )
    .await;
    assert!(matches!(
        crypto
            .open(input.workspace_id(), input.grant_id(), &sealed)
            .await,
        Err(WorkspaceError::RecoveryRequired)
    ));
}

#[test]
fn persisted_grant_refuses_inconsistent_scope_actor_and_revisions() {
    use serde_json::{Value, json};
    let input = selection();
    let proof = AttachmentProof::from_protected_input(&[7; 32]).unwrap();
    let record = record::GrantRecord::new(
        input.clone(),
        ActorRef::new(
            participant(),
            ActorRole::Buyer,
            None,
            Some(input.grant_id()),
        ),
        "synthetic-device".into(),
        UnixSeconds::new(200).unwrap(),
        &proof,
    )
    .unwrap();
    let encoded = record.encode().unwrap();
    let original: Value = serde_json::from_slice(&encoded).unwrap();
    let second_key = "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
    for (pointer, value) in [
        ("/version", json!(2)),
        ("/scope", json!("seller_stock")),
        ("/actor/role", json!("seller")),
        ("/actor/organization", json!(participant().to_hex())),
        ("/actor/participant", json!(second_key)),
        ("/actor/grant_id", Value::Null),
        ("/principal", json!("")),
        ("/principal", json!("x".repeat(129))),
        ("/principal", json!("bad\nlabel")),
        ("/revision", json!("0")),
        ("/expires_at", json!("0")),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert!(
            matches!(
                record::GrantRecord::decode(&serde_json::to_vec(&changed).unwrap()),
                Err(WorkspaceError::RecoveryRequired)
            ),
            "{pointer}"
        );
    }
    let mut unknown = original.clone();
    unknown["unknown"] = json!(true);
    assert!(matches!(
        record::GrantRecord::decode(&serde_json::to_vec(&unknown).unwrap()),
        Err(WorkspaceError::RecoveryRequired)
    ));
    let mut exhausted = original;
    exhausted["revision"] = json!(i64::MAX.to_string());
    let mut exhausted =
        record::GrantRecord::decode(&serde_json::to_vec(&exhausted).unwrap()).unwrap();
    assert_eq!(exhausted.revoke(), Err(WorkspaceError::RecoveryRequired));
    let record = record::GrantRecord::decode(&encoded).unwrap();
    assert_eq!(
        record
            .authenticate(&input, &proof, UnixSeconds::new(100).unwrap())
            .unwrap()
            .grant_revision()
            .get(),
        1
    );
}

struct FixedClock(u64);
impl WorkspaceClock for FixedClock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        UnixSeconds::new(self.0).map_err(|_| WorkspaceError::Unavailable)
    }
}
#[tokio::test]
async fn enrollment_requires_bounded_principal_current_time_and_usable_key() {
    for principal in [String::new(), "x".repeat(129), "bad\nlabel".into()] {
        assert!(matches!(
            LocalOutboxConfig::new(
                participant(),
                participant(),
                principal,
                UnixSeconds::new(200).unwrap()
            ),
            Err(WorkspaceError::InvalidInput)
        ));
    }
    assert!(matches!(
        LocalOutboxConfig::new(
            participant(),
            participant(),
            "device",
            UnixSeconds::new(0).unwrap()
        ),
        Err(WorkspaceError::InvalidInput)
    ));
    let fixture = CryptoFixture::new();
    for (now, expiry, expected) in [
        (0, 200, WorkspaceError::Unavailable),
        (100, 100, WorkspaceError::GrantExpired),
    ] {
        let path = fixture
            ._temp
            .path()
            .canonicalize()
            .unwrap()
            .join(format!("refused-{now}"));
        let result = NativeWorkspace::initialize_local_outbox(
            &path,
            LocalOutboxConfig::new(
                participant(),
                participant(),
                "device",
                UnixSeconds::new(expiry).unwrap(),
            )
            .unwrap(),
            fixture.crypto(),
            Arc::new(FixedClock(now)),
        )
        .await;
        assert!(matches!(result, Err(error) if error == expected));
        assert!(!path.exists());
    }
    let wrong = WorkspaceCrypto::new(
        fixture.provider.clone(),
        reference("grant-key", BackendKind::File, 1),
        SecretMaterial::from_slice(&[0x42; 32]).unwrap(),
        AccessPolicy::standard(),
    )
    .unwrap();
    let path = fixture
        ._temp
        .path()
        .canonicalize()
        .unwrap()
        .join("wrong-key");
    assert!(matches!(
        NativeWorkspace::initialize_local_outbox(
            &path,
            LocalOutboxConfig::new(
                participant(),
                participant(),
                "device",
                UnixSeconds::new(200).unwrap()
            )
            .unwrap(),
            wrong,
            Arc::new(FixedClock(100))
        )
        .await,
        Err(WorkspaceError::Unavailable)
    ));
    // Failed initialization remains visible. A second call cannot overwrite it.
    assert!(matches!(
        NativeWorkspace::initialize_local_outbox(
            &path,
            LocalOutboxConfig::new(
                participant(),
                participant(),
                "device",
                UnixSeconds::new(200).unwrap()
            )
            .unwrap(),
            fixture.crypto(),
            Arc::new(FixedClock(100))
        )
        .await,
        Err(WorkspaceError::NamespaceUnavailable)
    ));
}

fn set_mode(path: &std::path::Path, mode: u32) {
    host_fs::set_permissions(path, host_fs::Permissions::from_mode(mode)).unwrap();
}
#[test]
fn namespace_rechecks_permissions_links_replacement_and_sidecars() {
    for fault in 0..8 {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let path = base.join("outbox");
        let namespace = fs::Namespace::acquire(&path, true).unwrap();
        let database = namespace.database_path();
        match fault {
            0 => set_mode(&path, 0o750),
            1 => set_mode(&database, 0o640),
            2 => {
                host_fs::hard_link(&database, base.join("linked-database")).unwrap();
            }
            3 => {
                host_fs::rename(&database, base.join("old-database")).unwrap();
                host_fs::write(&database, b"replacement").unwrap();
                set_mode(&database, 0o600);
            }
            4 => {
                let lock = path.join(".execution_v1.lock");
                host_fs::rename(&lock, base.join("old-lock")).unwrap();
                host_fs::write(&lock, b"").unwrap();
                set_mode(&lock, 0o600);
            }
            5 => {
                host_fs::rename(&path, base.join("old-root")).unwrap();
                host_fs::DirBuilder::new()
                    .mode(0o700)
                    .create(&path)
                    .unwrap();
            }
            6 => {
                let sidecar = path.join("execution_v1.sqlite-wal");
                host_fs::write(&sidecar, b"").unwrap();
                set_mode(&sidecar, 0o644);
            }
            7 => {
                symlink(&database, path.join("execution_v1.sqlite-wal")).unwrap();
            }
            _ => unreachable!(),
        }
        assert_eq!(
            namespace.validate(),
            Err(WorkspaceError::NamespaceUnavailable),
            "fault {fault}"
        );
        drop(namespace);
    }
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let path = base.join("outbox");
    let namespace = fs::Namespace::acquire(&path, true).unwrap();
    for suffix in ["-wal", "-shm", "-journal"] {
        let path = path.join(format!("execution_v1.sqlite{suffix}"));
        host_fs::write(&path, b"").unwrap();
        set_mode(&path, 0o600);
    }
    namespace.validate().unwrap();
    host_fs::write(path.join("unexpected"), b"retain").unwrap();
    assert_eq!(namespace.validate(), Err(WorkspaceError::RecoveryRequired));
    assert_eq!(host_fs::read(path.join("unexpected")).unwrap(), b"retain");
}

#[test]
fn namespace_refuses_unsafe_paths_empty_database_and_nonempty_lock_without_repair() {
    assert!(matches!(
        fs::Namespace::acquire(std::path::Path::new("relative"), false),
        Err(WorkspaceError::NamespaceUnavailable)
    ));
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    assert!(matches!(
        fs::Namespace::acquire(&base.join("child/../outbox"), true),
        Err(WorkspaceError::NamespaceUnavailable)
    ));
    let mutable = base.join("mutable");
    host_fs::create_dir(&mutable).unwrap();
    set_mode(&mutable, 0o777);
    assert!(matches!(
        fs::Namespace::acquire(&mutable.join("outbox"), true),
        Err(WorkspaceError::NamespaceUnavailable)
    ));
    let regular = base.join("regular");
    host_fs::write(&regular, b"retain").unwrap();
    assert!(matches!(
        fs::Namespace::acquire(&regular, false),
        Err(WorkspaceError::NamespaceUnavailable)
    ));
    let target = base.join("target");
    host_fs::create_dir(&target).unwrap();
    symlink(&target, base.join("alias")).unwrap();
    assert!(matches!(
        fs::Namespace::acquire(&base.join("alias/outbox"), true),
        Err(WorkspaceError::NamespaceUnavailable)
    ));
    for nonempty_lock in [false, true] {
        let path = base.join(format!("existing-{nonempty_lock}"));
        let namespace = fs::Namespace::acquire(&path, true).unwrap();
        drop(namespace);
        if nonempty_lock {
            host_fs::write(path.join(".execution_v1.lock"), b"retain").unwrap();
        }
        assert!(matches!(
            fs::Namespace::acquire(&path, false),
            Err(WorkspaceError::RecoveryRequired)
        ));
        if nonempty_lock {
            assert_eq!(
                host_fs::read(path.join(".execution_v1.lock")).unwrap(),
                b"retain"
            );
        }
    }
}

#[tokio::test]
async fn existing_database_rejects_corrupt_version_missing_grant_and_oversized_rows_without_repair()
{
    use sqlx::Connection;
    let fixture = CryptoFixture::new();
    let base = fixture._temp.path().canonicalize().unwrap();
    for fault in 0..7 {
        let path = base.join(format!("corrupt-{fault}"));
        let (store, input, proof) = NativeWorkspace::initialize_local_outbox(
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
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path.join("execution_v1.sqlite"))
            .create_if_missing(false);
        let mut connection = sqlx::SqliteConnection::connect_with(&options)
            .await
            .unwrap();
        sqlx::query("PRAGMA ignore_check_constraints=ON")
            .execute(&mut connection)
            .await
            .unwrap();
        let statement = match fault {
            0 => "PRAGMA user_version=5",
            1 => "DELETE FROM workspace_grant",
            2 => "UPDATE workspace_grant SET grant_id=zeroblob(31)",
            3 => "UPDATE workspace_grant SET protected=zeroblob(0)",
            4 => "UPDATE workspace_grant SET protected=zeroblob(262145)",
            5 => "UPDATE workspace_grant SET protected=zeroblob(262144)",
            6 => "UPDATE workspace_grant SET workspace=zeroblob(31)",
            _ => unreachable!(),
        };
        sqlx::query(statement)
            .execute(&mut connection)
            .await
            .unwrap();
        connection.close().await.unwrap();
        if fault != 0 {
            assert!(
                matches!(
                    store.authenticate(input.clone(), &proof).await,
                    Err(WorkspaceError::RecoveryRequired)
                ),
                "live fault {fault}"
            );
        }
        store.close().await.unwrap();
        let result = NativeWorkspace::open_existing(
            &path,
            input,
            fixture.crypto(),
            Arc::new(FixedClock(100)),
        )
        .await;
        assert!(
            matches!(&result, Err(WorkspaceError::RecoveryRequired)),
            "fault {fault}: {result:?}"
        );
        let mut connection = sqlx::SqliteConnection::connect_with(&options.clone().read_only(true))
            .await
            .unwrap();
        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM workspace_grant")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert_eq!(version, if fault == 0 { 5 } else { 4 });
        assert_eq!(rows, if fault == 1 { 0 } else { 1 });
        if (3..=5).contains(&fault) {
            let size: i64 = sqlx::query_scalar("SELECT length(protected) FROM workspace_grant")
                .fetch_one(&mut connection)
                .await
                .unwrap();
            assert_eq!(size, [0, 262145, 262144][fault - 3]);
        }
        connection.close().await.unwrap();
    }
}

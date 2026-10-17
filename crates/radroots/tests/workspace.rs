#![cfg(all(feature = "native", unix))]
use radroots::{Client, NativeConfig, OperationId, StartMode};
use radroots_domain::{
    Approval, ApprovalIntentDigest, CorrelationId, ExpectedOrder, OrderId, PublicKey, RecordId,
    Sequence, UnixSeconds,
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
    event::SourceGeneration,
    workspace::{AttachmentProof, WorkspaceClock, WorkspaceError},
};
use radroots_storage_sqlite::{
    OpenMode, OpenOptions, Paths,
    workspace::{LocalOutboxConfig, NativeWorkspace, WorkspaceCrypto},
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
struct Clock(AtomicU64);
impl WorkspaceClock for Clock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        UnixSeconds::new(self.0.load(Ordering::SeqCst)).map_err(|_| WorkspaceError::Unavailable)
    }
}
fn key() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798").unwrap()
}
#[tokio::test]
async fn ordinary_client_authenticates_context_and_closes_actual_workspace() {
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
    let reference = || {
        SecretRef::new(
            SecretId::parse("workspace-key").unwrap(),
            BackendKind::File,
            KeyVersion::new(1).unwrap(),
        )
    };
    provider
        .provision(
            &reference(),
            &SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
            Nonce::new([0x11; 24]),
        )
        .unwrap();
    let crypto = || {
        WorkspaceCrypto::new(
            provider.clone(),
            reference(),
            SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
            AccessPolicy::standard(),
        )
        .unwrap()
    };
    let clock = Arc::new(Clock(AtomicU64::new(100)));
    let config = LocalOutboxConfig::new(
        key(),
        key(),
        "synthetic-device",
        UnixSeconds::new(200).unwrap(),
    )
    .unwrap();
    let (workspace, input, proof) = NativeWorkspace::initialize_local_outbox(
        &root.join("outbox"),
        config,
        crypto(),
        clock.clone(),
    )
    .await
    .unwrap();
    let workspace_copy = workspace.clone();
    let options = OpenOptions::new(
        Paths::from_directory(root.join("cache")).unwrap(),
        OpenMode::Create,
    )
    .with_source_generation(SourceGeneration::new([17; 32]).unwrap(), 1)
    .unwrap();
    // The existing public cache adapter requires its explicit directory to exist.
    std::fs::create_dir(root.join("cache")).unwrap();
    let client = Client::open(NativeConfig::public_read(options).with_workspace(workspace))
        .await
        .unwrap();
    let wrong = AttachmentProof::from_protected_input(&[0; 32]).unwrap();
    assert_eq!(
        client
            .attach(input.clone(), wrong)
            .await
            .unwrap_err()
            .code(),
        radroots_execution::Code::Unauthorized
    );
    let duplicate = proof
        .with_secret(|secret| AttachmentProof::from_protected_input(secret))
        .unwrap();
    let other_proof = proof
        .with_secret(|secret| AttachmentProof::from_protected_input(secret))
        .unwrap();
    let session = client.attach(input.clone(), proof).await.unwrap();
    assert_eq!(session.binding().actor().participant(), key());
    let op = OperationId::from_bytes([7; 32]);
    let approval = Approval::new(
        ApprovalIntentDigest::from_bytes([8; 32]),
        "known-kind",
        UnixSeconds::new(160).unwrap(),
    )
    .unwrap();
    let expected = ExpectedOrder::new(
        OrderId::from_bytes([9; 32]),
        Some(RecordId::from_bytes([10; 32])),
        Sequence::new(11).unwrap(),
        Some(RecordId::from_bytes([12; 32])),
        input.checkpoint().clone(),
    );
    let context = session
        .command_context_with_correlation(
            op,
            Some(expected.clone()),
            Some(approval.clone()),
            UnixSeconds::new(150).unwrap(),
            Some(CorrelationId::new("diagnostic").unwrap()),
        )
        .unwrap();
    let data = context.data();
    assert_eq!(data.operation_id(), op);
    assert_eq!(data.actor(), session.binding().actor());
    assert_eq!(data.workspace_id(), input.workspace_id());
    assert_eq!(data.authority(), input.authority());
    assert_eq!(data.route_digest(), input.route_digest());
    assert_eq!(data.approval(), Some(&approval));
    assert_eq!(data.expected(), Some(&expected));
    assert_eq!(data.not_after().get(), 150);
    assert_eq!(data.correlation_id().unwrap().as_str(), "diagnostic");
    context.revalidate().await.unwrap();
    assert_eq!(format!("{context:?}"), "CommandContext(<redacted>)");
    assert_eq!(format!("{session:?}"), "WorkspaceSession(<redacted>)");
    assert!(
        session
            .command_context(op, None, None, UnixSeconds::new(100).unwrap())
            .is_err()
    );
    assert!(
        session
            .command_context(op, None, None, UnixSeconds::new(201).unwrap())
            .is_err()
    );
    clock.0.store(150, Ordering::SeqCst);
    assert_eq!(
        context.revalidate().await.unwrap_err().code(),
        radroots_execution::Code::Unauthorized
    );
    clock.0.store(101, Ordering::SeqCst);
    let other = client.attach(input.clone(), other_proof).await.unwrap();
    let other_context = other
        .command_context(op, None, None, UnixSeconds::new(150).unwrap())
        .unwrap();
    other.close();
    assert_eq!(
        other_context.revalidate().await.unwrap_err().code(),
        radroots_execution::Code::CapabilityUnavailable
    );
    assert!(
        other
            .command_context(op, None, None, UnixSeconds::new(150).unwrap())
            .is_err()
    );
    context.revalidate().await.unwrap();
    workspace_copy
        .revoke(input.clone(), &duplicate)
        .await
        .unwrap();
    assert_eq!(
        context.revalidate().await.unwrap_err().code(),
        radroots_execution::Code::GrantRevoked
    );
    client.start(StartMode::LocalOnly).await.unwrap();
    client.close().await.unwrap();
    assert!(
        session
            .command_context(op, None, None, UnixSeconds::new(150).unwrap())
            .is_err()
    );
    assert_eq!(
        context.revalidate().await.unwrap_err().code(),
        radroots_execution::Code::CapabilityUnavailable
    );
    assert!(
        client
            .attach(
                input.clone(),
                AttachmentProof::from_protected_input(&[0; 32]).unwrap()
            )
            .await
            .is_err()
    );
    workspace_copy.close().await.unwrap();
    let reopened = NativeWorkspace::open_existing(&root.join("outbox"), input, crypto(), clock)
        .await
        .unwrap();
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn preview_cannot_authenticate_a_caller_supplied_workspace_descriptor() {
    let client = Client::preview();
    let input = radroots_storage::workspace::AttachmentInput::new(
        radroots_domain::WorkspaceId::from_bytes([1; 32]),
        radroots_domain::AuthorityRef::new(
            radroots_domain::ExecutorId::from_bytes([2; 32]),
            radroots_domain::Generation::new(1).unwrap(),
        ),
        key(),
        "local://workspace/untrusted",
        key(),
        radroots_domain::GrantId::from_bytes([3; 32]),
        radroots_domain::CheckpointRef::new(
            radroots_domain::CheckpointDigest::from_bytes([4; 32]),
            Sequence::new(0).unwrap(),
            radroots_domain::Coverage::Partial,
            vec![],
        )
        .unwrap(),
        radroots_domain::RouteDigest::from_bytes([5; 32]),
    )
    .unwrap();
    let proof = AttachmentProof::from_protected_input(&[0; 32]).unwrap();
    assert_eq!(
        client.attach(input, proof).await.unwrap_err().code(),
        radroots_execution::Code::MissingAuthority
    );
    client.close().await.unwrap();
}

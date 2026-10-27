//! This checkpoint exports descriptors, not a stock-envelope backend.
use super::*;
use nostr::JsonUtil;
use radroots_domain::{
    CommandDigest, EnvelopeEventId, OperationId, Sequence, SigningAttemptId, TransactionId,
};
use radroots_storage::{
    command::{CommitAttempt, OperationError, PrivateEnvelopeCandidate},
    inventory::{
        InventoryCheckpointRef, InventoryStore, StockEnvelopeReceipt, StockEnvelopeRequest,
        StockEnvelopeSource, StockEvidenceReceipt,
    },
    workspace::WorkspaceBinding,
};
use radroots_storage_sqlite::workspace::{SellerStockConfig, StockEnrollment};
use sha2::{Digest, Sha256};
use sqlx::Connection;

fn copy(proof: &AttachmentProof) -> AttachmentProof {
    proof
        .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
        .unwrap()
}
fn minimum(input: &AttachmentInput, revision: u64) -> InventoryCheckpointRef {
    InventoryCheckpointRef::from_host_metadata(
        input.authority(),
        Sequence::new(revision).unwrap(),
        (revision != 0).then_some(CheckpointDigest::from_bytes([12; 32])),
    )
    .unwrap()
}
fn request(binding: &WorkspaceBinding) -> StockEnvelopeRequest {
    StockEnvelopeRequest::new(
        binding.clone(),
        minimum(binding.input(), 0),
        OperationId::from_bytes([80; 32]),
        CommandDigest::from_bytes([81; 32]),
        key(),
        None,
    )
    .unwrap()
}
fn carrier() -> String {
    // A genuine signed carrier with opaque synthetic content. Neither this
    // signature nor the source model verifies a stock seal or mints authority.
    let signer = nostr::Keys::new(nostr::SecretKey::from_slice(&[3; 32]).unwrap());
    nostr::UnsignedEvent {
        id: None,
        pubkey: signer.public_key(),
        created_at: nostr::Timestamp::from_secs(100),
        kind: nostr::Kind::Custom(1059),
        tags: nostr::Tags::from_list(vec![nostr::Tag::public_key(
            nostr::PublicKey::from_slice(key().as_bytes()).unwrap(),
        )]),
        content: String::from("synthetic original carrier"),
    }
    .sign_with_keys(&signer)
    .unwrap()
    .as_json()
}
async fn stock(f: &Fixture) -> (NativeWorkspace, StockEnrollment, WorkspaceBinding) {
    let enrollment = StockEnrollment::prepare(
        SellerStockConfig::new(
            key(),
            key(),
            "stock-envelope-boundary",
            UnixSeconds::new(200).unwrap(),
        )
        .unwrap(),
        f.clock.as_ref(),
    )
    .unwrap();
    let store =
        NativeWorkspace::initialize_seller_stock(&f.path, &enrollment, f.crypto(), f.clock.clone())
            .await
            .unwrap();
    let binding = store
        .authenticate(enrollment.input().clone(), enrollment.proof())
        .await
        .unwrap();
    (store, enrollment, binding)
}

#[tokio::test]
async fn envelope_ports_explicitly_refuse_on_current_stock_and_ordinary_outbox_without_writes() {
    let f = Fixture::new();
    let (store, enrollment, binding) = stock(&f).await;
    let outbox_fixture = Fixture::new();
    let (outbox, _, _) = outbox_fixture.create().await;
    let candidate = PrivateEnvelopeCandidate::from_obtained(key(), &carrier(), &[3; 32]).unwrap();
    for port in [
        &store as &dyn InventoryStore,
        &outbox as &dyn InventoryStore,
    ] {
        assert!(matches!(
            port.capture_stock_envelope(request(&binding), copy(enrollment.proof()), &candidate)
                .await,
            Err(OperationError::UnsupportedOperation)
        ));
        assert!(matches!(
            port.stock_envelope(request(&binding), copy(enrollment.proof()))
                .await,
            Err(OperationError::UnsupportedOperation)
        ));
    }
    // Current catalog and authoritative history remain unchanged by refusal.
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(f.path.join("stock_execution_v1.sqlite"))
            .read_only(true),
    )
    .await
    .unwrap();
    let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    let entries: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_operations")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    assert_eq!(catalog, 15);
    assert_eq!(entries, 0);
    connection.close().await.unwrap();
    assert!(
        !outbox_fixture
            .path
            .join("stock_execution_v1.sqlite")
            .exists()
    );
    store
        .authenticate(enrollment.input().clone(), enrollment.proof())
        .await
        .unwrap();
    outbox.close().await.unwrap();
    store.close().await.unwrap();
}

#[tokio::test]
async fn reported_envelope_source_preserves_exact_carrier_and_refuses_tampered_or_mixed_reports() {
    let f = Fixture::new();
    let (store, _, binding) = stock(&f).await;
    let selected = request(&binding);
    let attempt = CommitAttempt::new(
        selected.attempt().workspace_id(),
        selected.operation_id(),
        selected.command_digest(),
        Some(TransactionId::from_bytes([82; 32])),
    );
    let evidence = StockEvidenceReceipt::from_committed_store(
        attempt,
        SigningAttemptId::from_bytes([83; 32]),
        key(),
        [84; 32],
        None,
    )
    .unwrap();
    let raw = carrier();
    let event = nostr::Event::from_json(&raw).unwrap();
    let id = EnvelopeEventId::from_bytes(event.id.to_bytes());
    let digest = Sha256::digest(raw.as_bytes()).into();
    let report = |id, digest| {
        StockEnvelopeReceipt::from_committed_store(
            &selected,
            attempt,
            evidence,
            TransactionId::from_bytes([85; 32]),
            id,
            digest,
            minimum(binding.input(), 1),
        )
        .unwrap()
    };
    // These public report constructors confer no persistence/stock permission.
    let receipt = report(id, digest);
    let source = StockEnvelopeSource::from_retained_wire(receipt, &raw).unwrap();
    source.with_json(|retained| assert_eq!(retained.as_bytes(), raw.as_bytes()));
    assert_eq!(source.receipt(), receipt);
    assert_eq!(format!("{source:?}"), "StockEnvelopeSource(<redacted>)");
    for (receipt, wire) in [
        (
            report(EnvelopeEventId::from_bytes([99; 32]), digest),
            raw.clone(),
        ),
        (report(id, [99; 32]), raw.clone()),
        (receipt, format!("{raw} ")),
        (receipt, "x".repeat(65537)),
        (receipt, "é".repeat(32769)),
    ] {
        assert!(matches!(
            StockEnvelopeSource::from_retained_wire(receipt, &wire),
            Err(OperationError::InvalidEvidence)
        ));
    }
    for field in ["id", "sig"] {
        let mut altered: serde_json::Value = serde_json::from_str(&raw).unwrap();
        altered[field] = "0".repeat(if field == "id" { 64 } else { 128 }).into();
        assert!(StockEnvelopeSource::from_retained_wire(receipt, &altered.to_string()).is_err());
    }
    let original = raw.clone();
    drop(raw);
    source.with_json(|retained| assert_eq!(retained, original));
    store.close().await.unwrap();
}

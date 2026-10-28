//! Shape checks remain independent of concrete persistence and permission.
#[path = "inventory_delivery.rs"]
mod delivery;
use super::*;
use radroots_domain::{
    CommandDigest, EnvelopeEventId, OperationId, SigningAttemptId, TransactionId,
};
use radroots_storage::{
    command::{CommitAttempt, OperationError},
    inventory::{
        InventoryCheckpointRef, StockEnvelopeReceipt, StockEnvelopeRequest, StockEvidenceReceipt,
    },
    workspace::WorkspaceScope,
};

fn binding(scope: WorkspaceScope) -> WorkspaceBinding {
    WorkspaceBinding::from_authenticated_host(
        input(),
        scope,
        ActorRef::new(key(), ActorRole::Seller, None, Some(input().grant_id())),
        "synthetic",
        Sequence::new(1).unwrap(),
        UnixSeconds::new(200).unwrap(),
        UnixSeconds::new(100).unwrap(),
    )
    .unwrap()
}
fn checkpoint(revision: u64) -> InventoryCheckpointRef {
    InventoryCheckpointRef::from_host_metadata(
        input().authority(),
        Sequence::new(revision).unwrap(),
        (revision != 0).then_some(CheckpointDigest::from_bytes([13; 32])),
    )
    .unwrap()
}
fn attempt(workspace: u8, operation: u8, digest: u8, committed: bool) -> CommitAttempt {
    CommitAttempt::new(
        WorkspaceId::from_bytes([workspace; 32]),
        OperationId::from_bytes([operation; 32]),
        CommandDigest::from_bytes([digest; 32]),
        committed.then_some(TransactionId::from_bytes([11; 32])),
    )
}
fn evidence(attempt: CommitAttempt, recipient: PublicKey) -> StockEvidenceReceipt {
    StockEvidenceReceipt::from_committed_store(
        attempt,
        SigningAttemptId::from_bytes([9; 32]),
        recipient,
        [10; 32],
        None,
    )
    .unwrap()
}

#[test]
fn envelope_selector_refuses_foreign_authority_and_non_stock_scope_without_minting_a_transaction() {
    let select = |scope, minimum| {
        StockEnvelopeRequest::new(
            binding(scope),
            minimum,
            OperationId::from_bytes([7; 32]),
            CommandDigest::from_bytes([8; 32]),
            key(),
            None,
        )
    };
    assert!(matches!(
        select(WorkspaceScope::NewSellerOutbox, checkpoint(0)),
        Err(OperationError::Unauthorized)
    ));
    let foreign = InventoryCheckpointRef::from_host_metadata(
        AuthorityRef::new(
            ExecutorId::from_bytes([99; 32]),
            Generation::new(1).unwrap(),
        ),
        Sequence::new(0).unwrap(),
        None,
    )
    .unwrap();
    assert!(select(WorkspaceScope::SellerStockOwner, foreign).is_err());
    let request = select(WorkspaceScope::SellerStockOwner, checkpoint(0)).unwrap();
    assert_eq!(request.minimum_checkpoint(), checkpoint(0));
    assert_eq!(request.attempt(), attempt(1, 7, 8, false));
    assert!(request.observed_at_unix_ms().is_none());
    assert_eq!(format!("{request:?}"), "StockEnvelopeRequest(<redacted>)");
}

#[test]
fn envelope_report_rejects_mixed_originals_and_preserves_nullable_capture_observation() {
    let original = attempt(1, 7, 8, true);
    let source = evidence(original, key());
    let peer =
        PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5")
            .unwrap();
    for observed in [None, core::num::NonZeroU64::new(150000)] {
        let request = StockEnvelopeRequest::new(
            binding(WorkspaceScope::SellerStockOwner),
            checkpoint(0),
            original.operation_id(),
            original.command_digest(),
            key(),
            observed,
        )
        .unwrap();
        let report = |anchor, obtained, progress| {
            StockEnvelopeReceipt::from_committed_store(
                &request,
                anchor,
                obtained,
                TransactionId::from_bytes([12; 32]),
                EnvelopeEventId::from_bytes([14; 32]),
                [15; 32],
                progress,
            )
        };
        for anchor in [
            attempt(1, 7, 8, false),
            attempt(99, 7, 8, true),
            attempt(1, 99, 8, true),
            attempt(1, 7, 99, true),
        ] {
            assert_eq!(
                report(anchor, source, checkpoint(1)).unwrap_err(),
                OperationError::RecoveryRequired
            );
        }
        for obtained in [
            evidence(attempt(99, 7, 8, true), key()),
            evidence(attempt(1, 99, 8, true), key()),
            evidence(attempt(1, 7, 99, true), key()),
            evidence(original, peer),
        ] {
            assert_eq!(
                report(original, obtained, checkpoint(1)).unwrap_err(),
                OperationError::RecoveryRequired
            );
        }
        assert!(report(original, source, checkpoint(0)).is_err());
        let foreign = InventoryCheckpointRef::from_host_metadata(
            AuthorityRef::new(
                ExecutorId::from_bytes([99; 32]),
                Generation::new(1).unwrap(),
            ),
            checkpoint(1).revision(),
            checkpoint(1).head(),
        )
        .unwrap();
        assert!(report(original, source, foreign).is_err());
        let retained = report(original, source, checkpoint(1)).unwrap();
        assert_eq!(retained.attempt(), original);
        assert_eq!(retained.evidence(), source);
        assert_eq!(retained.recipient(), key());
        assert_eq!(
            retained.installation_transaction_id(),
            TransactionId::from_bytes([12; 32])
        );
        assert_eq!(retained.event_id(), EnvelopeEventId::from_bytes([14; 32]));
        assert_eq!(retained.digest(), &[15; 32]);
        assert_eq!(retained.checkpoint(), checkpoint(1));
        assert_eq!(retained.observed_at_unix_ms(), observed);
        assert_eq!(format!("{retained:?}"), "StockEnvelopeReceipt(<redacted>)");
    }
}

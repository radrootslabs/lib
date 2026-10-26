//! These are bounded descriptors and report-shape checks, not store proofs.
use super::*;
use radroots_domain::{CommandDigest, OperationId, SigningAttemptId, TransactionId};
use radroots_storage::{
    command::{CommitAttempt, OperationError},
    inventory::{StockEvidenceReceipt, StockEvidenceRequest},
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
fn request(scope: WorkspaceScope, raw: &str) -> Result<StockEvidenceRequest, OperationError> {
    StockEvidenceRequest::from_obtained_wire(
        binding(scope),
        OperationId::from_bytes([7; 32]),
        CommandDigest::from_bytes([8; 32]),
        SigningAttemptId::from_bytes([9; 32]),
        key(),
        [10; 32],
        raw,
        None,
    )
}
#[test]
fn input_bounds_original_identity_and_redaction_hold_without_authenticating_a_claim() {
    for raw in ["".to_owned(), "x".repeat(65537), "é".repeat(32769)] {
        assert!(matches!(
            request(WorkspaceScope::SellerStockOwner, &raw),
            Err(OperationError::InvalidEvidence)
        ));
    }
    assert!(request(WorkspaceScope::NewSellerOutbox, "x").is_err());
    let raw = "x".repeat(65536);
    let evidence = request(WorkspaceScope::SellerStockOwner, &raw).unwrap();
    let retained = evidence.retained_copy();
    drop(evidence);
    retained.with_json(|value| assert_eq!(value, raw));
    assert!(!format!("{retained:?}").contains(&raw));
    assert_eq!(retained.attempt().workspace_id(), input().workspace_id());
    assert_eq!(retained.attempt().operation_id(), retained.operation_id());
    assert_eq!(
        retained.attempt().command_digest(),
        retained.command_digest()
    );
    assert!(retained.attempt().transaction_id().is_none());
    assert_eq!(
        retained.signing_attempt_id(),
        SigningAttemptId::from_bytes([9; 32])
    );
    assert_eq!(retained.recipient(), key());
    assert_eq!(retained.intent_digest(), &[10; 32]);
    assert!(retained.observed_at_unix_ms().is_none());
}
#[test]
fn append_report_requires_commit_identity_and_preserves_nullable_observation() {
    let attempt = |transaction| {
        CommitAttempt::new(
            input().workspace_id(),
            OperationId::from_bytes([7; 32]),
            CommandDigest::from_bytes([8; 32]),
            transaction,
        )
    };
    let claim = SigningAttemptId::from_bytes([9; 32]);
    assert_eq!(
        StockEvidenceReceipt::from_committed_store(attempt(None), claim, key(), [10; 32], None)
            .unwrap_err(),
        OperationError::RecoveryRequired
    );
    for observed in [None, core::num::NonZeroU64::new(150000)] {
        let original = attempt(Some(TransactionId::from_bytes([11; 32])));
        let report =
            StockEvidenceReceipt::from_committed_store(original, claim, key(), [10; 32], observed)
                .unwrap();
        assert_eq!(report.attempt(), original);
        assert_eq!(report.signing_attempt_id(), claim);
        assert_eq!(report.recipient(), key());
        assert_eq!(report.digest(), &[10; 32]);
        assert_eq!(report.observed_at_unix_ms(), observed);
    }
}

use super::*;
use radroots_domain::{
    CommandDigest, OperationId, OrderId, PoolId, TransactionId,
    listing::MeasurementUnit,
    stock::{PhysicalBalances, PoolBalances, PoolSnapshot, PoolUnit, StockTicks},
};
use radroots_storage::{
    command::{CommitAttempt, OperationError},
    inventory::{InventoryCheckpointRef, StockAbortReceipt},
    workspace::seller_stock::StockAuthorizationRef,
};

fn checkpoint(
    namespace: &AttachmentInput,
    revision: u64,
    authorization: bool,
) -> InventoryCheckpointRef {
    InventoryCheckpointRef::from_host_metadata(
        namespace.authority(),
        Sequence::new(revision).unwrap(),
        (revision != 0).then_some(CheckpointDigest::from_bytes([7; 32])),
    )
    .unwrap()
    .with_optional_authorization(authorization.then(|| {
        StockAuthorizationRef::from_host_metadata(
            Sequence::new(1).unwrap(),
            CheckpointDigest::from_bytes([8; 32]),
        )
        .unwrap()
    }))
}
fn attempt(namespace: &AttachmentInput, committed: bool) -> CommitAttempt {
    CommitAttempt::new(
        namespace.workspace_id(),
        OperationId::from_bytes([70; 32]),
        CommandDigest::from_bytes([71; 32]),
        committed.then_some(TransactionId::from_bytes([72; 32])),
    )
}
fn report(
    namespace: &AttachmentInput,
    attempt: CommitAttempt,
    checkpoint: InventoryCheckpointRef,
    pools: Vec<PoolSnapshot>,
) -> Result<StockAbortReceipt, OperationError> {
    StockAbortReceipt::from_committed_store(
        namespace,
        attempt,
        OrderId::from_bytes([73; 32]),
        UnixSeconds::new(111).unwrap(),
        checkpoint,
        pools,
    )
}
fn pool(id: u8, revision: u64) -> PoolSnapshot {
    PoolSnapshot::new(
        PoolId::from_bytes([id; 32]),
        PoolUnit::new(MeasurementUnit::Each, 0).unwrap(),
        PoolBalances::new(StockTicks::ZERO, PhysicalBalances::ZERO).unwrap(),
        Sequence::new(revision).unwrap(),
    )
}

#[test]
fn shared_cleanup_metadata_refuses_missing_commit_progress_authorization_and_foreign_namespace() {
    let namespace = input();
    let valid = checkpoint(&namespace, 1, true);
    for (anchor, progress) in [
        (attempt(&namespace, false), valid),
        (attempt(&namespace, true), checkpoint(&namespace, 0, true)),
        (attempt(&namespace, true), checkpoint(&namespace, 1, false)),
        (
            CommitAttempt::new(
                WorkspaceId::from_bytes([99; 32]),
                OperationId::from_bytes([70; 32]),
                CommandDigest::from_bytes([71; 32]),
                Some(TransactionId::from_bytes([72; 32])),
            ),
            valid,
        ),
        (
            attempt(&namespace, true),
            InventoryCheckpointRef::from_host_metadata(
                AuthorityRef::new(
                    ExecutorId::from_bytes([99; 32]),
                    Generation::new(1).unwrap(),
                ),
                valid.revision(),
                valid.head(),
            )
            .unwrap()
            .with_optional_authorization(valid.authorization()),
        ),
    ] {
        assert_eq!(
            report(&namespace, anchor, progress, vec![]).unwrap_err(),
            OperationError::RecoveryRequired
        );
    }
    let value = report(&namespace, attempt(&namespace, true), valid, vec![]).unwrap();
    assert_eq!(value.attempt(), attempt(&namespace, true));
    assert_eq!(value.checkpoint(), valid);
    assert!(value.released_pools().is_empty());
    assert_eq!(format!("{value:?}"), "StockAbortReceipt(<redacted>)");
}

#[test]
fn shared_cleanup_pool_reports_are_bounded_canonical_and_keep_the_original_after_images() {
    let namespace = input();
    let selected = |pools| {
        report(
            &namespace,
            attempt(&namespace, true),
            checkpoint(&namespace, 1, true),
            pools,
        )
    };
    for invalid in [
        vec![pool(1, 3), pool(1, 3)],
        vec![pool(2, 3), pool(1, 3)],
        vec![pool(1, 0)],
        vec![pool(1, 1)],
        vec![pool(1, 2)],
        (0..65).map(|id| pool(id, 3)).collect(),
    ] {
        assert_eq!(
            selected(invalid).unwrap_err(),
            OperationError::RecoveryRequired
        );
    }
    let expected: Vec<_> = (0..64).map(|id| pool(id, 3)).collect();
    let value = selected(expected.clone()).unwrap();
    assert!(value.released_pools() == expected);
    assert!(value.into_released_pools() == expected);
}

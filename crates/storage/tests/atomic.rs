use futures_executor::block_on;
use radroots_protocol::runtime::v1::OperationId;
use radroots_storage::{
    Error,
    atomic::{
        AtomicCommit, AtomicCommitDigest, AtomicCommitDisposition, AtomicCommitId,
        AtomicCommitOutcome, AtomicCommitReceipt, AtomicStorage, AtomicWorkflow,
    },
    journal::{IdempotencyDigest, IdempotencyKey, OperationInstanceId, PrepareOperation},
};
use radroots_transport::BoxFuture;
use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

struct ReferenceAtomic {
    receipts: Mutex<BTreeMap<AtomicCommitId, AtomicCommitReceipt>>,
    fail_before_commit: AtomicBool,
}

impl ReferenceAtomic {
    fn new() -> Self {
        Self {
            receipts: Mutex::new(BTreeMap::new()),
            fail_before_commit: AtomicBool::new(false),
        }
    }
}

impl AtomicStorage for ReferenceAtomic {
    fn commit(&self, request: AtomicCommit) -> BoxFuture<'_, Result<AtomicCommitReceipt, Error>> {
        Box::pin(async move {
            let mut receipts = self.receipts.lock().expect("atomic test lock");
            if let Some(existing) = receipts.get(&request.commit_id()) {
                if existing.digest() != request.digest()
                    || existing.outcome().kind() != request.workflow().kind()
                {
                    return Err(Error::AtomicCommitConflict);
                }
                return AtomicCommitReceipt::new(
                    &request,
                    AtomicCommitDisposition::Replay,
                    existing.committed_at_unix_ms(),
                    existing.outcome().clone(),
                );
            }
            if self.fail_before_commit.swap(false, Ordering::SeqCst) {
                return Err(Error::AtomicCommitFailed);
            }
            let outcome = match request.workflow().clone() {
                AtomicWorkflow::Prepared(operation) => AtomicCommitOutcome::Prepared {
                    journal: operation.into_record()?,
                },
                _ => return Err(Error::AtomicCommitFailed),
            };
            let receipt = AtomicCommitReceipt::new(
                &request,
                AtomicCommitDisposition::Committed,
                request.requested_at_unix_ms(),
                outcome,
            )?;
            receipts.insert(request.commit_id(), receipt.clone());
            Ok(receipt)
        })
    }

    fn receipt(
        &self,
        commit_id: AtomicCommitId,
    ) -> BoxFuture<'_, Result<Option<AtomicCommitReceipt>, Error>> {
        Box::pin(async move {
            Ok(self
                .receipts
                .lock()
                .expect("atomic test lock")
                .get(&commit_id)
                .cloned())
        })
    }
}

fn request(commit_byte: u8, digest_byte: u8) -> AtomicCommit {
    let prepare = PrepareOperation::new(
        OperationInstanceId::new([9; 16]).expect("operation instance"),
        OperationId::TradePrivateArtifactSeal,
        IdempotencyKey::parse("atomic-test-key").expect("idempotency key"),
        IdempotencyDigest::new([3; 32]),
        100,
    )
    .expect("prepare operation");
    AtomicCommit::new(
        AtomicCommitId::new([commit_byte; 16]).expect("commit id"),
        AtomicCommitDigest::new([digest_byte; 32]),
        100,
        AtomicWorkflow::Prepared(prepare),
    )
    .expect("atomic commit")
}

#[test]
fn failure_before_commit_leaves_no_partial_receipt() {
    let store = ReferenceAtomic::new();
    let request = request(1, 2);
    store.fail_before_commit.store(true, Ordering::SeqCst);
    assert_eq!(
        block_on(store.commit(request.clone())),
        Err(Error::AtomicCommitFailed)
    );
    assert_eq!(
        block_on(store.receipt(request.commit_id())).expect("receipt query"),
        None
    );

    let committed = block_on(store.commit(request.clone())).expect("commit");
    assert_eq!(committed.disposition(), AtomicCommitDisposition::Committed);
    assert_eq!(committed.commit_id(), request.commit_id());
    assert_eq!(committed.digest(), request.digest());
    assert_eq!(committed.committed_at_unix_ms(), 100);
    assert_eq!(request.commit_id().as_bytes(), &[1; 16]);
    assert_eq!(request.digest().as_bytes(), &[2; 32]);
    assert_eq!(request.requested_at_unix_ms(), 100);
    assert_eq!(
        request.workflow().kind(),
        radroots_storage::atomic::AtomicWorkflowKind::Prepared
    );
    assert!(
        block_on(store.receipt(request.commit_id()))
            .expect("receipt query")
            .is_some()
    );
}

#[test]
fn exact_commit_replays_and_digest_reuse_conflicts() {
    let store = ReferenceAtomic::new();
    let original_request = request(1, 2);
    let original = block_on(store.commit(original_request.clone())).expect("commit");
    let replay = block_on(store.commit(original_request)).expect("replay");
    assert_eq!(replay.disposition(), AtomicCommitDisposition::Replay);
    assert_eq!(replay.outcome(), original.outcome());
    assert_eq!(
        block_on(store.commit(request(1, 4))),
        Err(Error::AtomicCommitConflict)
    );
}

#[test]
fn atomic_contract_is_dyn_compatible_and_rejects_invalid_identity_and_time() {
    fn accepts_dyn(_: &dyn AtomicStorage) {}
    let store = ReferenceAtomic::new();
    accepts_dyn(&store);
    assert_eq!(
        AtomicCommitId::new([0; 16]),
        Err(Error::InvalidAtomicCommitId)
    );
    let valid = request(1, 2);
    assert_eq!(
        AtomicCommit::new(
            valid.commit_id(),
            valid.digest(),
            0,
            valid.workflow().clone()
        ),
        Err(Error::InvalidAtomicCommitTimestamp)
    );

    let outcome = match valid.workflow().clone() {
        AtomicWorkflow::Prepared(operation) => AtomicCommitOutcome::Prepared {
            journal: operation.into_record().expect("journal record"),
        },
        _ => unreachable!(),
    };
    assert_eq!(
        AtomicCommitReceipt::new(
            &valid,
            AtomicCommitDisposition::Committed,
            99,
            outcome.clone(),
        ),
        Err(Error::AtomicWorkflowMismatch)
    );
    assert_eq!(
        AtomicCommitReceipt::from_durable_parts(
            valid.commit_id(),
            valid.digest(),
            AtomicCommitDisposition::Committed,
            0,
            100,
            radroots_storage::atomic::AtomicWorkflowKind::Prepared,
            outcome.clone(),
        ),
        Err(Error::AtomicWorkflowMismatch)
    );
    assert_eq!(
        AtomicCommitReceipt::from_durable_parts(
            valid.commit_id(),
            valid.digest(),
            AtomicCommitDisposition::Committed,
            100,
            99,
            radroots_storage::atomic::AtomicWorkflowKind::Prepared,
            outcome.clone(),
        ),
        Err(Error::AtomicWorkflowMismatch)
    );
    assert_eq!(
        AtomicCommitReceipt::from_durable_parts(
            valid.commit_id(),
            valid.digest(),
            AtomicCommitDisposition::Committed,
            100,
            100,
            radroots_storage::atomic::AtomicWorkflowKind::Signed,
            outcome.clone(),
        ),
        Err(Error::AtomicWorkflowMismatch)
    );
    let reconstructed = AtomicCommitReceipt::from_durable_parts(
        valid.commit_id(),
        valid.digest(),
        AtomicCommitDisposition::Replay,
        100,
        101,
        radroots_storage::atomic::AtomicWorkflowKind::Prepared,
        outcome.clone(),
    )
    .expect("durable receipt");
    assert_eq!(reconstructed.commit_id(), valid.commit_id());
    assert_eq!(reconstructed.digest(), valid.digest());
    assert_eq!(reconstructed.disposition(), AtomicCommitDisposition::Replay);
    assert_eq!(reconstructed.committed_at_unix_ms(), 101);
    assert_eq!(reconstructed.outcome(), &outcome);
}

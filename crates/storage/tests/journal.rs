use futures_executor::block_on;
use radroots_event::EventId;
use radroots_protocol::runtime::v1::OperationId;
use radroots_storage::{
    Error,
    journal::{
        CancellationState, IdempotencyDigest, IdempotencyKey, Journal, JournalRevision,
        JournalStage, JournalState, JournalTransition, OperationInstanceId, OperationRecord,
        PrepareDisposition, PrepareOperation, PrepareReceipt, RECOVERABLE_QUERY_LIMIT_MAX,
        RecoveryPoint, RecoveryReason, RecoveryRecord,
    },
};
use radroots_transport::BoxFuture;
use std::sync::Mutex;

struct MemoryJournal(Mutex<Vec<OperationRecord>>);

impl MemoryJournal {
    fn new() -> Self {
        Self(Mutex::new(Vec::new()))
    }
}

impl Journal for MemoryJournal {
    fn prepare(&self, operation: PrepareOperation) -> BoxFuture<'_, Result<PrepareReceipt, Error>> {
        Box::pin(async move {
            let mut records = self.0.lock().expect("test journal lock");
            if let Some(record) = records
                .iter()
                .find(|record| record.idempotency_key() == operation.idempotency_key())
            {
                if record.operation_id() != operation.operation_id()
                    || record.input_digest() != operation.input_digest()
                    || record.instance_id() != operation.instance_id()
                {
                    return Err(Error::IdempotencyConflict);
                }
                return Ok(PrepareReceipt::new(
                    PrepareDisposition::Replay,
                    record.clone(),
                ));
            }
            if records
                .iter()
                .any(|record| record.instance_id() == operation.instance_id())
            {
                return Err(Error::OperationIdentityMismatch);
            }
            let record = operation.into_record()?;
            records.push(record.clone());
            Ok(PrepareReceipt::new(PrepareDisposition::Created, record))
        })
    }

    fn operation(
        &self,
        instance_id: OperationInstanceId,
    ) -> BoxFuture<'_, Result<Option<OperationRecord>, Error>> {
        Box::pin(async move {
            Ok(self
                .0
                .lock()
                .expect("test journal lock")
                .iter()
                .find(|record| record.instance_id() == instance_id)
                .cloned())
        })
    }

    fn by_idempotency_key(
        &self,
        operation_id: OperationId,
        idempotency_key: IdempotencyKey,
    ) -> BoxFuture<'_, Result<Option<OperationRecord>, Error>> {
        Box::pin(async move {
            Ok(self
                .0
                .lock()
                .expect("test journal lock")
                .iter()
                .find(|record| {
                    record.operation_id() == operation_id
                        && record.idempotency_key() == &idempotency_key
                })
                .cloned())
        })
    }

    fn transition(
        &self,
        transition: JournalTransition,
    ) -> BoxFuture<'_, Result<OperationRecord, Error>> {
        Box::pin(async move {
            let mut records = self.0.lock().expect("test journal lock");
            let record = records
                .iter_mut()
                .find(|record| record.instance_id() == transition.instance_id())
                .ok_or(Error::OperationNotFound)?;
            let next = record.transition(&transition)?;
            *record = next.clone();
            Ok(next)
        })
    }

    fn recoverable(&self, limit: u16) -> BoxFuture<'_, Result<Vec<OperationRecord>, Error>> {
        Box::pin(async move {
            if limit == 0 || limit > RECOVERABLE_QUERY_LIMIT_MAX {
                return Err(Error::InvalidJournalQueryLimit);
            }
            Ok(self
                .0
                .lock()
                .expect("test journal lock")
                .iter()
                .filter(|record| record.state().stage() == JournalStage::Recoverable)
                .take(usize::from(limit))
                .cloned()
                .collect())
        })
    }
}

fn instance(byte: u8) -> OperationInstanceId {
    OperationInstanceId::new([byte; 16]).expect("operation instance")
}

fn key(byte: u8) -> IdempotencyKey {
    IdempotencyKey::parse(format!("sync-push-{byte:02x}")).expect("idempotency key")
}

fn event_id(byte: &str) -> EventId {
    EventId::parse(byte.repeat(64)).expect("event id")
}

fn prepare(instance_id: OperationInstanceId, digest: u8, at: u64) -> PrepareOperation {
    PrepareOperation::new(
        instance_id,
        OperationId::SyncPush,
        key(instance_id.as_bytes()[0]),
        IdempotencyDigest::new([digest; 32]),
        at,
    )
    .expect("prepare operation")
}

#[test]
fn prepare_replays_exact_input_and_rejects_conflicts() {
    let journal = MemoryJournal::new();
    let dynamic: &dyn Journal = &journal;
    let operation = prepare(instance(1), 2, 100);
    let created = block_on(dynamic.prepare(operation.clone())).expect("created");
    assert_eq!(created.disposition(), PrepareDisposition::Created);
    assert_eq!(created.record().revision(), JournalRevision::INITIAL);

    let replay = block_on(journal.prepare(operation)).expect("replay");
    assert_eq!(replay.disposition(), PrepareDisposition::Replay);
    assert_eq!(replay.record(), created.record());
    assert_eq!(
        block_on(journal.prepare(prepare(instance(1), 3, 100))),
        Err(Error::IdempotencyConflict)
    );
    let conflicting_kind = PrepareOperation::new(
        instance(1),
        OperationId::FarmPublish,
        key(1),
        IdempotencyDigest::new([2; 32]),
        100,
    )
    .expect("conflicting operation kind");
    assert_eq!(
        block_on(journal.prepare(conflicting_kind)),
        Err(Error::IdempotencyConflict)
    );

    let debug = format!("{:?}", key(1));
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("sync-push-01"));
}

#[test]
fn lifecycle_is_optimistic_monotonic_and_commit_bound() {
    let journal = MemoryJournal::new();
    let instance_id = instance(4);
    let event_id = event_id("a");
    let prepared = block_on(journal.prepare(prepare(instance_id, 5, 100)))
        .expect("prepare")
        .record()
        .clone();
    let signed = block_on(journal.transition(JournalTransition::signed(
        instance_id,
        prepared.revision(),
        event_id,
    )))
    .expect("signed");
    assert_eq!(signed.state().stage(), JournalStage::Signed);
    assert_eq!(
        block_on(journal.transition(JournalTransition::signed(
            instance_id,
            prepared.revision(),
            event_id,
        ))),
        Err(Error::JournalRevisionConflict)
    );

    let committed = block_on(journal.transition(JournalTransition::committed(
        instance_id,
        signed.revision(),
        event_id,
        150,
    )))
    .expect("committed");
    assert_eq!(committed.state().stage(), JournalStage::Committed);
    assert_eq!(
        block_on(
            journal.transition(JournalTransition::recoverable(
                instance_id,
                committed.revision(),
                RecoveryRecord::new(
                    RecoveryPoint::Signed { event_id },
                    RecoveryReason::TransportUnavailable,
                    1,
                    None,
                )
                .expect("recovery"),
            ))
        ),
        Err(Error::JournalOperationCommitted)
    );
}

#[test]
fn cancellation_before_commit_recovers_and_after_commit_preserves_commit() {
    let journal = MemoryJournal::new();
    let before_id = instance(6);
    let prepared = block_on(journal.prepare(prepare(before_id, 7, 100)))
        .expect("prepare before")
        .record()
        .clone();
    let cancelled = block_on(journal.transition(JournalTransition::cancelled(
        before_id,
        prepared.revision(),
        110,
    )))
    .expect("cancel before commit");
    assert_eq!(cancelled.state().stage(), JournalStage::Recoverable);
    assert_eq!(
        cancelled.cancellation(),
        CancellationState::CancelledBeforeCommit
    );
    assert_eq!(
        block_on(journal.recoverable(10)).expect("recovery").len(),
        1
    );

    let resumed =
        block_on(journal.transition(JournalTransition::resume(before_id, cancelled.revision())))
            .expect("resume");
    assert_eq!(resumed.state(), &JournalState::Prepared);
    assert_eq!(resumed.cancellation(), CancellationState::NotRequested);

    let after_id = instance(8);
    let event_id = event_id("b");
    let prepared = block_on(journal.prepare(prepare(after_id, 9, 200)))
        .expect("prepare after")
        .record()
        .clone();
    let signed = block_on(journal.transition(JournalTransition::signed(
        after_id,
        prepared.revision(),
        event_id,
    )))
    .expect("signed after");
    let committed = block_on(journal.transition(JournalTransition::committed(
        after_id,
        signed.revision(),
        event_id,
        220,
    )))
    .expect("committed after");
    let observed = block_on(journal.transition(JournalTransition::cancelled(
        after_id,
        committed.revision(),
        230,
    )))
    .expect("cancel observed after commit");
    assert_eq!(observed.state().stage(), JournalStage::Committed);
    assert_eq!(
        observed.cancellation(),
        CancellationState::ObservedAfterCommit
    );
}

#[test]
fn invalid_records_and_inputs_fail_closed() {
    assert_eq!(
        OperationInstanceId::new([0; 16]),
        Err(Error::InvalidOperationInstanceId)
    );
    assert_eq!(
        IdempotencyKey::parse(" bad-key"),
        Err(Error::InvalidIdempotencyKey)
    );
    assert_eq!(
        RecoveryRecord::new(
            RecoveryPoint::Prepared,
            RecoveryReason::Interrupted,
            0,
            None,
        ),
        Err(Error::InvalidRecoveryAttempt)
    );
}

#[test]
fn journal_value_and_state_validation_matrix_is_complete() {
    let instance_id = instance(1);
    assert_eq!(instance_id.as_bytes(), &[1; 16]);
    for invalid in ["", " leading", "trailing ", "bad\nkey"] {
        assert_eq!(
            IdempotencyKey::parse(invalid),
            Err(Error::InvalidIdempotencyKey)
        );
    }
    assert_eq!(
        IdempotencyKey::parse("x".repeat(radroots_storage::journal::IDEMPOTENCY_KEY_MAX_BYTES + 1)),
        Err(Error::InvalidIdempotencyKey)
    );
    let idempotency_key = key(1);
    assert_eq!(idempotency_key.as_str(), "sync-push-01");
    let digest = IdempotencyDigest::new([2; 32]);
    assert_eq!(digest.as_bytes(), &[2; 32]);
    assert_eq!(JournalRevision::new(0), Err(Error::InvalidJournalRevision));
    assert_eq!(JournalRevision::new(2).unwrap().get(), 2);
    assert_eq!(
        RecoveryRecord::new(
            RecoveryPoint::Prepared,
            RecoveryReason::Interrupted,
            1,
            Some(0),
        ),
        Err(Error::InvalidRecoveryDeadline)
    );
    let recovery = RecoveryRecord::new(
        RecoveryPoint::Signed {
            event_id: event_id("a"),
        },
        RecoveryReason::TransportUnavailable,
        2,
        Some(150),
    )
    .unwrap();
    assert!(matches!(recovery.point(), RecoveryPoint::Signed { .. }));
    assert_eq!(recovery.reason(), RecoveryReason::TransportUnavailable);
    assert_eq!(recovery.attempt(), 2);
    assert_eq!(recovery.retry_not_before_unix_ms(), Some(150));
    for state in [
        JournalState::Prepared,
        JournalState::Signed {
            event_id: event_id("a"),
        },
        JournalState::Recoverable(recovery.clone()),
        JournalState::Committed {
            event_id: event_id("a"),
            committed_at_unix_ms: 150,
        },
    ] {
        let expected = match state {
            JournalState::Prepared => JournalStage::Prepared,
            JournalState::Signed { .. } => JournalStage::Signed,
            JournalState::Recoverable(_) => JournalStage::Recoverable,
            JournalState::Committed { .. } => JournalStage::Committed,
        };
        assert_eq!(state.stage(), expected);
    }

    let build = |state, cancellation| {
        OperationRecord::from_parts(
            instance_id,
            OperationId::SyncPush,
            idempotency_key.clone(),
            digest,
            100,
            JournalRevision::INITIAL,
            state,
            cancellation,
        )
    };
    assert_eq!(
        OperationRecord::from_parts(
            instance_id,
            OperationId::SyncPush,
            idempotency_key.clone(),
            digest,
            0,
            JournalRevision::INITIAL,
            JournalState::Prepared,
            CancellationState::NotRequested,
        ),
        Err(Error::InvalidOperationTimestamp)
    );
    for result in [
        build(
            JournalState::Committed {
                event_id: event_id("a"),
                committed_at_unix_ms: 0,
            },
            CancellationState::NotRequested,
        ),
        build(
            JournalState::Committed {
                event_id: event_id("a"),
                committed_at_unix_ms: 99,
            },
            CancellationState::NotRequested,
        ),
        build(
            JournalState::Recoverable(
                RecoveryRecord::new(
                    RecoveryPoint::Prepared,
                    RecoveryReason::Interrupted,
                    1,
                    Some(99),
                )
                .unwrap(),
            ),
            CancellationState::NotRequested,
        ),
        build(
            JournalState::Committed {
                event_id: event_id("a"),
                committed_at_unix_ms: 100,
            },
            CancellationState::CancelledBeforeCommit,
        ),
        build(
            JournalState::Recoverable(recovery.clone()),
            CancellationState::ObservedAfterCommit,
        ),
        build(
            JournalState::Recoverable(recovery.clone()),
            CancellationState::CancelledBeforeCommit,
        ),
        build(
            JournalState::Prepared,
            CancellationState::ObservedAfterCommit,
        ),
        build(
            JournalState::Signed {
                event_id: event_id("a"),
            },
            CancellationState::CancelledBeforeCommit,
        ),
    ] {
        assert_eq!(result, Err(Error::CorruptJournalRecord));
    }

    let operation = prepare(instance_id, 2, 100);
    assert_eq!(operation.instance_id(), instance_id);
    assert_eq!(operation.operation_id(), OperationId::SyncPush);
    assert_eq!(operation.idempotency_key(), &idempotency_key);
    assert_eq!(operation.input_digest(), digest);
    let record = operation.into_record().unwrap();
    assert_eq!(record.instance_id(), instance_id);
    assert_eq!(record.operation_id(), OperationId::SyncPush);
    assert_eq!(record.idempotency_key(), &idempotency_key);
    assert_eq!(record.input_digest(), digest);
    assert_eq!(record.prepared_at_unix_ms(), 100);
    assert_eq!(record.revision(), JournalRevision::INITIAL);
    assert_eq!(record.cancellation(), CancellationState::NotRequested);
    let receipt = PrepareReceipt::new(PrepareDisposition::Created, record.clone());
    assert_eq!(receipt.disposition(), PrepareDisposition::Created);
    assert_eq!(receipt.record(), &record);

    assert_eq!(
        record.transition(&JournalTransition::signed(
            instance(2),
            record.revision(),
            event_id("a"),
        )),
        Err(Error::OperationIdentityMismatch)
    );
    assert_eq!(
        record.transition(&JournalTransition::committed(
            instance_id,
            record.revision(),
            event_id("a"),
            100,
        )),
        Err(Error::InvalidJournalTransition)
    );
    assert_eq!(
        record.transition(&JournalTransition::cancelled(
            instance_id,
            record.revision(),
            99,
        )),
        Err(Error::InvalidJournalTransition)
    );
    let signed = record
        .transition(&JournalTransition::signed(
            instance_id,
            record.revision(),
            event_id("a"),
        ))
        .unwrap();
    assert_eq!(
        signed.transition(&JournalTransition::committed(
            instance_id,
            signed.revision(),
            event_id("b"),
            101,
        )),
        Err(Error::InvalidJournalTransition)
    );
    let recoverable = signed
        .transition(&JournalTransition::recoverable(
            instance_id,
            signed.revision(),
            recovery,
        ))
        .unwrap();
    let resumed = recoverable
        .transition(&JournalTransition::resume(
            instance_id,
            recoverable.revision(),
        ))
        .unwrap();
    assert!(matches!(resumed.state(), JournalState::Signed { .. }));
    assert_eq!(
        JournalTransition::resume(instance_id, resumed.revision()).instance_id(),
        instance_id
    );
}

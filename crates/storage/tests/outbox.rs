use futures_executor::block_on;
use radroots_event::{SignedEvent, wire::Nip01EventWire};
use radroots_storage::{
    Error, Outbox,
    journal::OperationInstanceId,
    outbox::{
        ClaimOutboxItems, ClaimedOutboxItem, DeliveryAttempt, DeliveryAttemptEvidence,
        DeliveryPlanDigest, EnqueueDisposition, EnqueueOutboxItem, EnqueueReceipt, LeaseId,
        LeaseOwner, OutboxItemId, OutboxLease, OutboxRecord, OutboxStage, OutboxStatus,
        SatisfactionResult,
    },
};
use radroots_transport::{
    BoxFuture, DeliveryReceipt, DeliveryRequest, Target, TargetSet,
    outcome::DeliveryOutcome,
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    sink::{DeliveryPayload, DeliveryTargetReceipt},
};
use std::{collections::BTreeMap, sync::Mutex};

struct MemoryOutbox {
    records: Mutex<BTreeMap<OutboxItemId, OutboxRecord>>,
}

impl MemoryOutbox {
    fn new() -> Self {
        Self {
            records: Mutex::new(BTreeMap::new()),
        }
    }
}

impl Outbox for MemoryOutbox {
    fn enqueue(&self, item: EnqueueOutboxItem) -> BoxFuture<'_, Result<EnqueueReceipt, Error>> {
        Box::pin(async move {
            let mut records = self.records.lock().expect("test outbox lock");
            if let Some(existing) = records.get(&item.item_id()) {
                let exact = existing.operation_instance_id() == item.operation_instance_id()
                    && existing.plan_digest() == item.plan_digest()
                    && existing.request() == item.request();
                return if exact {
                    Ok(EnqueueReceipt::new(
                        EnqueueDisposition::Replay,
                        existing.clone(),
                    ))
                } else {
                    Err(Error::OutboxPlanConflict)
                };
            }
            let record = item.into_record();
            records.insert(record.item_id(), record.clone());
            Ok(EnqueueReceipt::new(EnqueueDisposition::Created, record))
        })
    }

    fn item(&self, item_id: OutboxItemId) -> BoxFuture<'_, Result<Option<OutboxRecord>, Error>> {
        Box::pin(async move {
            Ok(self
                .records
                .lock()
                .expect("test outbox lock")
                .get(&item_id)
                .cloned())
        })
    }

    fn claim(
        &self,
        request: ClaimOutboxItems,
    ) -> BoxFuture<'_, Result<Vec<ClaimedOutboxItem>, Error>> {
        Box::pin(async move {
            let mut records = self.records.lock().expect("test outbox lock");
            let mut claimed = Vec::new();
            for record in records.values_mut() {
                if claimed.len() >= usize::from(request.limit()) || record.stage().is_terminal() {
                    continue;
                }
                if matches!(record.retry_not_before_unix_ms(), Some(value) if value > request.now_unix_ms())
                {
                    continue;
                }
                if record
                    .lease()
                    .is_some_and(|lease| lease.is_active_at(request.now_unix_ms()))
                {
                    continue;
                }
                let lease = OutboxLease::new(
                    request.lease_id_for(record.item_id()),
                    request.owner().clone(),
                    request.now_unix_ms(),
                    request.lease_expires_at_unix_ms(),
                )?;
                record.claim(lease.clone())?;
                claimed.push(ClaimedOutboxItem::new(record.clone(), lease));
            }
            Ok(claimed)
        })
    }

    fn record_attempt(
        &self,
        evidence: DeliveryAttemptEvidence,
    ) -> BoxFuture<'_, Result<OutboxRecord, Error>> {
        Box::pin(async move {
            let mut records = self.records.lock().expect("test outbox lock");
            let record = records
                .get_mut(&evidence.item_id())
                .ok_or(Error::OutboxItemNotFound)?;
            record.record_attempt(evidence)?;
            Ok(record.clone())
        })
    }

    fn release(
        &self,
        item_id: OutboxItemId,
        lease_id: LeaseId,
        expected_revision: radroots_storage::outbox::OutboxRevision,
        released_at_unix_ms: u64,
        retry_not_before_unix_ms: Option<u64>,
    ) -> BoxFuture<'_, Result<OutboxRecord, Error>> {
        Box::pin(async move {
            let mut records = self.records.lock().expect("test outbox lock");
            let record = records.get_mut(&item_id).ok_or(Error::OutboxItemNotFound)?;
            record.release(
                lease_id,
                expected_revision,
                released_at_unix_ms,
                retry_not_before_unix_ms,
            )?;
            Ok(record.clone())
        })
    }

    fn status(&self) -> BoxFuture<'_, Result<OutboxStatus, Error>> {
        Box::pin(async move {
            let mut status = OutboxStatus {
                pending: 0,
                leased: 0,
                retryable: 0,
                satisfied: 0,
                exhausted: 0,
            };
            for record in self.records.lock().expect("test outbox lock").values() {
                match record.stage() {
                    OutboxStage::Pending => status.pending += 1,
                    OutboxStage::Leased => status.leased += 1,
                    OutboxStage::Retryable => status.retryable += 1,
                    OutboxStage::Satisfied => status.satisfied += 1,
                    OutboxStage::Exhausted => status.exhausted += 1,
                }
            }
            Ok(status)
        })
    }
}

fn signed_event() -> SignedEvent {
    let mut wire = Nip01EventWire {
        id: "0".repeat(64),
        pubkey: "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df".to_owned(),
        created_at: 1_800_000_100,
        kind: 0,
        tags: vec![],
        content: "{\"display_name\":\"Moss Street Farm\",\"bot\":false}".to_owned(),
        sig: "42".repeat(64),
        extra: Default::default(),
    };
    wire.id = wire
        .computed_event_id()
        .expect("canonical event id")
        .to_hex();
    let raw_json = serde_json::to_string(&wire).expect("event JSON");
    SignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
}

fn targets() -> Vec<Target> {
    vec![
        Target::nostr_relay("wss://one.example").expect("first target"),
        Target::nostr_relay("wss://two.example").expect("second target"),
    ]
}

fn request() -> DeliveryRequest {
    DeliveryRequest::new(
        "outbox-test-request",
        DeliveryPayload::new(signed_event()),
        TargetSet::new(targets()).expect("target set"),
        SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
        10_000,
    )
    .expect("delivery request")
}

fn enqueue(item_byte: u8, digest_byte: u8) -> EnqueueOutboxItem {
    EnqueueOutboxItem::new(
        OutboxItemId::new([item_byte; 16]).expect("item id"),
        OperationInstanceId::new([9; 16]).expect("operation instance"),
        DeliveryPlanDigest::new([digest_byte; 32]),
        request(),
        10,
    )
    .expect("enqueue request")
}

fn claim(store: &dyn Outbox, now: u64, expiry: u64, seed: u8) -> ClaimedOutboxItem {
    let request = ClaimOutboxItems::new(
        LeaseOwner::parse("worker-a").expect("owner"),
        LeaseId::new([seed; 16]).expect("lease seed"),
        now,
        expiry,
        1,
    )
    .expect("claim request");
    block_on(store.claim(request))
        .expect("claim")
        .pop()
        .expect("claimed item")
}

fn receipt(request: &DeliveryRequest, outcomes: [DeliveryOutcome; 2]) -> DeliveryReceipt {
    let receipts = request
        .target_set()
        .targets()
        .iter()
        .cloned()
        .zip(outcomes)
        .map(|(target, outcome)| DeliveryTargetReceipt::attempted(target, outcome))
        .collect();
    DeliveryReceipt::for_request(request, receipts).expect("delivery receipt")
}

#[test]
fn enqueue_is_idempotent_and_rejects_a_conflicting_plan() {
    let store = MemoryOutbox::new();
    let first = enqueue(1, 2);
    let created = block_on(store.enqueue(first.clone())).expect("created");
    assert_eq!(created.disposition(), EnqueueDisposition::Created);
    let replay = block_on(store.enqueue(first)).expect("exact replay");
    assert_eq!(replay.disposition(), EnqueueDisposition::Replay);

    let conflict = enqueue(1, 3);
    assert_eq!(
        block_on(store.enqueue(conflict)),
        Err(Error::OutboxPlanConflict)
    );
}

#[test]
fn leases_exclude_concurrent_claims_expire_and_defer_retries() {
    let store = MemoryOutbox::new();
    block_on(store.enqueue(enqueue(1, 2))).expect("enqueue");
    let first = claim(&store, 100, 200, 3);

    let concurrent = ClaimOutboxItems::new(
        LeaseOwner::parse("worker-b").expect("owner"),
        LeaseId::new([4; 16]).expect("seed"),
        150,
        250,
        1,
    )
    .expect("claim request");
    assert!(block_on(store.claim(concurrent)).expect("claim").is_empty());

    let stale = DeliveryAttemptEvidence::new(
        first.record().item_id(),
        first.lease().id(),
        first.record().revision(),
        DeliveryAttempt::FIRST,
        receipt(
            first.record().request(),
            [DeliveryOutcome::accepted(), DeliveryOutcome::unavailable()],
        ),
        200,
    )
    .expect("evidence");
    assert_eq!(
        block_on(store.record_attempt(stale)),
        Err(Error::OutboxLeaseExpired)
    );

    let reclaimed = claim(&store, 200, 300, 5);
    let released = block_on(store.release(
        reclaimed.record().item_id(),
        reclaimed.lease().id(),
        reclaimed.record().revision(),
        210,
        Some(250),
    ))
    .expect("release");
    assert_eq!(released.stage(), OutboxStage::Pending);
    assert_eq!(released.retry_not_before_unix_ms(), Some(250));
}

#[test]
fn partial_retryable_evidence_advances_to_satisfaction() {
    let store = MemoryOutbox::new();
    block_on(store.enqueue(enqueue(1, 2))).expect("enqueue");
    let first = claim(&store, 100, 200, 3);
    let first_result = block_on(
        store.record_attempt(
            DeliveryAttemptEvidence::new(
                first.record().item_id(),
                first.lease().id(),
                first.record().revision(),
                DeliveryAttempt::FIRST,
                receipt(
                    first.record().request(),
                    [DeliveryOutcome::accepted(), DeliveryOutcome::unavailable()],
                ),
                150,
            )
            .expect("attempt evidence"),
        ),
    )
    .expect("record partial attempt");
    assert_eq!(first_result.stage(), OutboxStage::Retryable);
    assert_eq!(first_result.satisfaction(), SatisfactionResult::Pending);
    assert_eq!(first_result.evidence().len(), 2);
    assert!(first_result.evidence()[1].outcome().is_retryable());

    let second = claim(&store, 250, 350, 4);
    let second_result = block_on(
        store.record_attempt(
            DeliveryAttemptEvidence::new(
                second.record().item_id(),
                second.lease().id(),
                second.record().revision(),
                DeliveryAttempt::new(2).expect("second attempt"),
                receipt(
                    second.record().request(),
                    [DeliveryOutcome::accepted(), DeliveryOutcome::delivered()],
                ),
                300,
            )
            .expect("attempt evidence"),
        ),
    )
    .expect("record successful attempt");
    assert_eq!(second_result.stage(), OutboxStage::Satisfied);
    assert_eq!(second_result.satisfaction(), SatisfactionResult::Satisfied);
    assert_eq!(second_result.evidence().len(), 4);
    let second_target = &second_result.request().target_set().targets()[1];
    assert_eq!(
        second_result
            .latest_target_evidence(second_target.fingerprint())
            .expect("latest target evidence")
            .attempt()
            .get(),
        2
    );
    assert_eq!(block_on(store.status()).expect("status").satisfied, 1);
}

#[test]
fn terminal_outcomes_exhaust_the_plan_and_models_are_bounded() {
    let store = MemoryOutbox::new();
    block_on(store.enqueue(enqueue(1, 2))).expect("enqueue");
    let claimed = claim(&store, 100, 200, 3);
    let exhausted = block_on(
        store.record_attempt(
            DeliveryAttemptEvidence::new(
                claimed.record().item_id(),
                claimed.lease().id(),
                claimed.record().revision(),
                DeliveryAttempt::FIRST,
                receipt(
                    claimed.record().request(),
                    [DeliveryOutcome::rejected(), DeliveryOutcome::rejected()],
                ),
                150,
            )
            .expect("attempt evidence"),
        ),
    )
    .expect("record terminal attempt");
    assert_eq!(exhausted.stage(), OutboxStage::Exhausted);
    assert_eq!(exhausted.satisfaction(), SatisfactionResult::Exhausted);
    assert_eq!(block_on(store.status()).expect("status").total(), Some(1));

    assert_eq!(OutboxItemId::new([0; 16]), Err(Error::InvalidOutboxItemId));
    assert_eq!(DeliveryAttempt::new(0), Err(Error::InvalidDeliveryAttempt));
    assert_eq!(
        ClaimOutboxItems::new(
            LeaseOwner::parse("worker").expect("owner"),
            LeaseId::new([1; 16]).expect("seed"),
            1,
            2,
            0,
        ),
        Err(Error::InvalidOutboxClaimLimit)
    );
    assert!(
        format!("{:?}", LeaseOwner::parse("secret-worker").expect("owner")).contains("[REDACTED]")
    );
}

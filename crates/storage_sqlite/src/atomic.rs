use crate::{SqliteStorage, journal, outbox, projection};
use radroots_storage::{
    Error,
    atomic::{
        AtomicCommit, AtomicCommitDigest, AtomicCommitDisposition, AtomicCommitId,
        AtomicCommitOutcome, AtomicCommitReceipt, AtomicStorage, AtomicWorkflow,
        AtomicWorkflowKind,
    },
    event::{
        AdmissionDisposition, AdmissionReceipt, AdmissionStage, BoxFuture, EventPosition,
        EventSequence, SourceGeneration,
    },
    journal::JournalTransition,
};
use sqlx::{Row, Sqlite};

const RECEIPT_FORMAT_VERSION: u8 = 1;
const RECEIPT_MAX_BYTES: usize = 4 * 1024 * 1024;

impl AtomicStorage for SqliteStorage {
    fn commit(&self, request: AtomicCommit) -> BoxFuture<'_, Result<AtomicCommitReceipt, Error>> {
        Box::pin(async move {
            self.require_atomic_writer()?;
            let mut transaction = self
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;

            let result = commit_transaction(self, &mut transaction, &request).await;
            match result {
                Ok(receipt) => {
                    transaction.commit().await.map_err(map_backend)?;
                    Ok(receipt)
                }
                Err(primary) => {
                    let rollback = transaction.rollback().await;
                    Err(preserve_primary(primary, rollback))
                }
            }
        })
    }

    fn receipt(
        &self,
        commit_id: AtomicCommitId,
    ) -> BoxFuture<'_, Result<Option<AtomicCommitReceipt>, Error>> {
        Box::pin(async move {
            sqlx::query(
                "SELECT commit_id, commit_digest, workflow_kind, requested_at_unix_ms,
                        committed_at_unix_ms, receipt
                 FROM radroots_runtime_atomic_commits WHERE commit_id = ?",
            )
            .bind(commit_id.as_bytes().as_slice())
            .fetch_optional(self.pool())
            .await
            .map_err(map_backend)?
            .as_ref()
            .map(decode_receipt_row)
            .transpose()
        })
    }
}

impl SqliteStorage {
    fn require_atomic_writer(&self) -> Result<(), Error> {
        if self.event_mode() == radroots_storage::status::EventStoreMode::ReadOnly {
            return Err(Error::BackendUnavailable);
        }
        Ok(())
    }
}

async fn commit_transaction(
    storage: &SqliteStorage,
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    request: &AtomicCommit,
) -> Result<AtomicCommitReceipt, Error> {
    if let Some(row) = sqlx::query(
        "SELECT commit_id, commit_digest, workflow_kind, requested_at_unix_ms,
                committed_at_unix_ms, receipt
         FROM radroots_runtime_atomic_commits WHERE commit_id = ?",
    )
    .bind(request.commit_id().as_bytes().as_slice())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_backend)?
    {
        let committed = decode_receipt_row(&row)?;
        if committed.digest() != request.digest()
            || committed.outcome().kind() != request.workflow().kind()
        {
            return Err(Error::AtomicCommitConflict);
        }
        return AtomicCommitReceipt::new(
            request,
            AtomicCommitDisposition::Replay,
            committed.committed_at_unix_ms(),
            committed.outcome().clone(),
        );
    }

    let outcome = execute_workflow(storage, transaction, request.workflow().clone()).await?;
    let committed_at_unix_ms = request.requested_at_unix_ms();
    let receipt = AtomicCommitReceipt::new(
        request,
        AtomicCommitDisposition::Committed,
        committed_at_unix_ms,
        outcome,
    )?;
    let snapshot = encode_outcome(receipt.outcome())?;
    sqlx::query(
        "INSERT INTO radroots_runtime_atomic_commits (
           commit_id, commit_digest, workflow_kind, requested_at_unix_ms,
           committed_at_unix_ms, receipt
         ) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(request.commit_id().as_bytes().as_slice())
    .bind(request.digest().as_bytes().as_slice())
    .bind(workflow_name(request.workflow().kind()))
    .bind(i64_from_u64(request.requested_at_unix_ms())?)
    .bind(i64_from_u64(committed_at_unix_ms)?)
    .bind(snapshot)
    .execute(&mut **transaction)
    .await
    .map_err(map_backend)?;
    Ok(receipt)
}

async fn execute_workflow(
    storage: &SqliteStorage,
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    workflow: AtomicWorkflow,
) -> Result<AtomicCommitOutcome, Error> {
    match workflow {
        AtomicWorkflow::Prepared(operation) => Ok(AtomicCommitOutcome::Prepared {
            journal: journal::prepare_transaction(transaction, operation)
                .await?
                .record()
                .clone(),
        }),
        AtomicWorkflow::Signed(signed) => {
            let event_id = *signed.event().id();
            let journal = journal::transition_transaction(
                transaction,
                JournalTransition::signed(
                    signed.instance_id(),
                    signed.expected_revision(),
                    event_id,
                ),
            )
            .await?;
            Ok(AtomicCommitOutcome::Signed { journal, event_id })
        }
        AtomicWorkflow::Enqueued(enqueued) => {
            let admission = storage
                .admit_transaction(transaction, enqueued.admission().clone())
                .await?;
            let outbox = outbox::enqueue_transaction(transaction, enqueued.outbox().clone())
                .await?
                .record()
                .clone();
            let journal = journal::transition_transaction(
                transaction,
                JournalTransition::committed(
                    enqueued.instance_id(),
                    enqueued.expected_revision(),
                    *enqueued.admission().event_id(),
                    enqueued.committed_at_unix_ms(),
                ),
            )
            .await?;
            Ok(AtomicCommitOutcome::Enqueued {
                journal,
                admission,
                outbox: Box::new(outbox),
            })
        }
        AtomicWorkflow::Delivered(evidence) => Ok(AtomicCommitOutcome::Delivered {
            outbox: Box::new(outbox::record_attempt_transaction(transaction, *evidence).await?),
        }),
        AtomicWorkflow::Ingested(ingested) => {
            let admission = storage
                .admit_transaction(transaction, ingested.admission().clone())
                .await?;
            let projection = match ingested.projection().cloned() {
                Some(checkpoint) => Some(Box::new(
                    projection::checkpoint_transaction(transaction, checkpoint).await?,
                )),
                None => None,
            };
            Ok(AtomicCommitOutcome::Ingested {
                admission,
                projection,
            })
        }
    }
}

fn decode_receipt_row(row: &sqlx::sqlite::SqliteRow) -> Result<AtomicCommitReceipt, Error> {
    let commit_id = AtomicCommitId::new(array(
        row.try_get::<Vec<u8>, _>("commit_id")
            .map_err(map_corrupt)?,
    )?)
    .map_err(|_| Error::AtomicCommitFailed)?;
    let digest = AtomicCommitDigest::new(array(
        row.try_get::<Vec<u8>, _>("commit_digest")
            .map_err(map_corrupt)?,
    )?);
    let workflow_kind = workflow_kind(
        row.try_get::<String, _>("workflow_kind")
            .map_err(map_corrupt)?
            .as_str(),
    )?;
    let requested_at_unix_ms =
        u64_from_i64(row.try_get("requested_at_unix_ms").map_err(map_corrupt)?)?;
    let committed_at_unix_ms =
        u64_from_i64(row.try_get("committed_at_unix_ms").map_err(map_corrupt)?)?;
    let bytes = row.try_get::<Vec<u8>, _>("receipt").map_err(map_corrupt)?;
    let outcome = decode_outcome(bytes.as_slice())?;
    AtomicCommitReceipt::from_durable_parts(
        commit_id,
        digest,
        AtomicCommitDisposition::Committed,
        requested_at_unix_ms,
        committed_at_unix_ms,
        workflow_kind,
        outcome,
    )
    .map_err(|_| Error::AtomicCommitFailed)
}

fn encode_outcome(outcome: &AtomicCommitOutcome) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::with_capacity(512);
    bytes.push(RECEIPT_FORMAT_VERSION);
    match outcome {
        AtomicCommitOutcome::Prepared { journal } => {
            bytes.push(0);
            put_blob(
                &mut bytes,
                journal::encode_record_snapshot(journal)?.as_slice(),
            )?;
        }
        AtomicCommitOutcome::Signed { journal, event_id } => {
            bytes.push(1);
            put_blob(
                &mut bytes,
                journal::encode_record_snapshot(journal)?.as_slice(),
            )?;
            bytes.extend_from_slice(event_id.as_bytes());
        }
        AtomicCommitOutcome::Enqueued {
            journal,
            admission,
            outbox,
        } => {
            bytes.push(2);
            put_blob(
                &mut bytes,
                journal::encode_record_snapshot(journal)?.as_slice(),
            )?;
            encode_admission(&mut bytes, admission);
            put_blob(
                &mut bytes,
                outbox::encode_record_snapshot(outbox)?.as_slice(),
            )?;
        }
        AtomicCommitOutcome::Delivered { outbox } => {
            bytes.push(3);
            put_blob(
                &mut bytes,
                outbox::encode_record_snapshot(outbox)?.as_slice(),
            )?;
        }
        AtomicCommitOutcome::Ingested {
            admission,
            projection,
        } => {
            bytes.push(4);
            encode_admission(&mut bytes, admission);
            match projection {
                Some(status) => {
                    bytes.push(1);
                    put_blob(
                        &mut bytes,
                        projection::encode_status_snapshot(status)?.as_slice(),
                    )?;
                }
                None => bytes.push(0),
            }
        }
    }
    if bytes.len() > RECEIPT_MAX_BYTES {
        return Err(Error::AtomicCommitFailed);
    }
    Ok(bytes)
}

fn decode_outcome(bytes: &[u8]) -> Result<AtomicCommitOutcome, Error> {
    if bytes.len() > RECEIPT_MAX_BYTES {
        return Err(Error::AtomicCommitFailed);
    }
    let mut cursor = Cursor::new(bytes);
    if cursor.byte()? != RECEIPT_FORMAT_VERSION {
        return Err(Error::AtomicCommitFailed);
    }
    let outcome = match cursor.byte()? {
        0 => AtomicCommitOutcome::Prepared {
            journal: journal::decode_record_snapshot(cursor.blob()?)
                .map_err(|_| Error::AtomicCommitFailed)?,
        },
        1 => AtomicCommitOutcome::Signed {
            journal: journal::decode_record_snapshot(cursor.blob()?)
                .map_err(|_| Error::AtomicCommitFailed)?,
            event_id: radroots_storage::event::EventId::from_bytes(cursor.array()?),
        },
        2 => AtomicCommitOutcome::Enqueued {
            journal: journal::decode_record_snapshot(cursor.blob()?)
                .map_err(|_| Error::AtomicCommitFailed)?,
            admission: decode_admission(&mut cursor)?,
            outbox: Box::new(
                outbox::decode_record_snapshot(cursor.blob()?)
                    .map_err(|_| Error::AtomicCommitFailed)?,
            ),
        },
        3 => AtomicCommitOutcome::Delivered {
            outbox: Box::new(
                outbox::decode_record_snapshot(cursor.blob()?)
                    .map_err(|_| Error::AtomicCommitFailed)?,
            ),
        },
        4 => {
            let admission = decode_admission(&mut cursor)?;
            let projection = match cursor.byte()? {
                0 => None,
                1 => Some(Box::new(
                    projection::decode_status_snapshot(cursor.blob()?)
                        .map_err(|_| Error::AtomicCommitFailed)?,
                )),
                _ => return Err(Error::AtomicCommitFailed),
            };
            AtomicCommitOutcome::Ingested {
                admission,
                projection,
            }
        }
        _ => return Err(Error::AtomicCommitFailed),
    };
    cursor.finish()?;
    Ok(outcome)
}

fn encode_admission(bytes: &mut Vec<u8>, receipt: &AdmissionReceipt) {
    bytes.extend_from_slice(receipt.event_id().as_bytes());
    bytes.extend_from_slice(receipt.position().generation().as_bytes());
    bytes.extend_from_slice(&receipt.position().sequence().get().to_be_bytes());
    bytes.push(match receipt.stage() {
        AdmissionStage::Raw => 0,
        AdmissionStage::Verified => 1,
        AdmissionStage::Visible => 2,
    });
    bytes.push(match receipt.disposition() {
        AdmissionDisposition::Inserted => 0,
        AdmissionDisposition::Advanced => 1,
        AdmissionDisposition::Duplicate => 2,
    });
}

fn decode_admission(cursor: &mut Cursor<'_>) -> Result<AdmissionReceipt, Error> {
    let event_id = radroots_storage::event::EventId::from_bytes(cursor.array()?);
    let generation =
        SourceGeneration::new(cursor.array()?).map_err(|_| Error::AtomicCommitFailed)?;
    let sequence = EventSequence::new(cursor.u64()?).map_err(|_| Error::AtomicCommitFailed)?;
    let stage = match cursor.byte()? {
        0 => AdmissionStage::Raw,
        1 => AdmissionStage::Verified,
        2 => AdmissionStage::Visible,
        _ => return Err(Error::AtomicCommitFailed),
    };
    let disposition = match cursor.byte()? {
        0 => AdmissionDisposition::Inserted,
        1 => AdmissionDisposition::Advanced,
        2 => AdmissionDisposition::Duplicate,
        _ => return Err(Error::AtomicCommitFailed),
    };
    Ok(AdmissionReceipt::new(
        event_id,
        EventPosition::new(generation, sequence),
        stage,
        disposition,
    ))
}

fn put_blob(bytes: &mut Vec<u8>, value: &[u8]) -> Result<(), Error> {
    let length = u32::try_from(value.len()).map_err(|_| Error::AtomicCommitFailed)?;
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(value);
    Ok(())
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn byte(&mut self) -> Result<u8, Error> {
        let value = self
            .bytes
            .get(self.offset)
            .copied()
            .ok_or(Error::AtomicCommitFailed)?;
        self.offset += 1;
        Ok(value)
    }

    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        self.take(N)?
            .try_into()
            .map_err(|_| Error::AtomicCommitFailed)
    }

    fn blob(&mut self) -> Result<&'a [u8], Error> {
        let length = usize::try_from(self.u32()?).map_err(|_| Error::AtomicCommitFailed)?;
        self.take(length)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(Error::AtomicCommitFailed)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(Error::AtomicCommitFailed)?;
        self.offset = end;
        Ok(value)
    }

    fn finish(self) -> Result<(), Error> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(Error::AtomicCommitFailed)
        }
    }
}

fn preserve_primary<T>(primary: Error, _rollback: Result<(), T>) -> Error {
    primary
}

const fn workflow_name(kind: AtomicWorkflowKind) -> &'static str {
    match kind {
        AtomicWorkflowKind::Prepared => "prepared",
        AtomicWorkflowKind::Signed => "signed",
        AtomicWorkflowKind::Enqueued => "enqueued",
        AtomicWorkflowKind::Delivered => "delivered",
        AtomicWorkflowKind::Ingested => "ingested",
    }
}

fn workflow_kind(value: &str) -> Result<AtomicWorkflowKind, Error> {
    match value.as_bytes() {
        b"prepared" => Ok(AtomicWorkflowKind::Prepared),
        b"signed" => Ok(AtomicWorkflowKind::Signed),
        b"enqueued" => Ok(AtomicWorkflowKind::Enqueued),
        b"delivered" => Ok(AtomicWorkflowKind::Delivered),
        b"ingested" => Ok(AtomicWorkflowKind::Ingested),
        _ => Err(Error::AtomicCommitFailed),
    }
}

fn array<const N: usize>(bytes: Vec<u8>) -> Result<[u8; N], Error> {
    bytes.try_into().map_err(|_| Error::AtomicCommitFailed)
}

fn i64_from_u64(value: u64) -> Result<i64, Error> {
    i64::try_from(value).map_err(|_| Error::AtomicCommitFailed)
}

fn u64_from_i64(value: i64) -> Result<u64, Error> {
    u64::try_from(value).map_err(|_| Error::AtomicCommitFailed)
}

fn map_backend(_: sqlx::Error) -> Error {
    Error::BackendUnavailable
}

fn map_corrupt(_: sqlx::Error) -> Error {
    Error::AtomicCommitFailed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::runtime::{MIGRATIONS, migration_sql};
    use radroots_event::{SignedEvent, wire::Nip01EventWire};
    use radroots_storage::{
        Journal, Outbox,
        atomic::{CommitEnqueued, CommitIngested, CommitSigned},
        event::EventAdmission,
        journal::{
            IdempotencyDigest, IdempotencyKey, JournalRevision, JournalStage, JournalTransition,
            OperationId, OperationInstanceId, PrepareOperation,
        },
        outbox::{
            ClaimOutboxItems, DeliveryAttempt, DeliveryAttemptEvidence, DeliveryOutcome,
            DeliveryPayload, DeliveryPlanDigest, DeliveryReceipt, DeliveryRequest,
            DeliveryTargetReceipt, EnqueueOutboxItem, LeaseId, LeaseOwner, OutboxItemId,
            SatisfactionClass, SatisfactionPolicy, TargetPolicy, TargetSet,
        },
        projection::{ProjectionCheckpoint, ProjectionGeneration, ProjectionId},
        status::EventStoreMode,
    };
    use radroots_transport::{
        Target, TransportId,
        source::{EventProvenance, ObservedEvent},
    };
    use sqlx::sqlite::SqlitePoolOptions;

    async fn store(mode: EventStoreMode) -> SqliteStorage {
        let generation = SourceGeneration::new([71; 32]).expect("generation");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory SQLite");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("foreign keys");
        for migration in MIGRATIONS {
            sqlx::raw_sql(migration_sql(migration.version()).expect("registered SQL"))
                .execute(&pool)
                .await
                .expect("runtime migration");
        }
        sqlx::query(
            "INSERT INTO radroots_runtime_source_generations (
               generation, sequence_head, state, created_at_unix_ms, retired_at_unix_ms
             ) VALUES (?, 0, 'active', 1, NULL)",
        )
        .bind(generation.as_bytes().as_slice())
        .execute(&pool)
        .await
        .expect("source generation");
        SqliteStorage::new(pool, generation, mode)
    }

    fn instance(byte: u8) -> OperationInstanceId {
        OperationInstanceId::new([byte; 16]).expect("instance")
    }

    fn prepare(byte: u8, at: u64) -> PrepareOperation {
        PrepareOperation::new(
            instance(byte),
            OperationId::SyncPush,
            IdempotencyKey::parse(format!("atomic-operation-{byte:02x}")).expect("idempotency key"),
            IdempotencyDigest::new([byte; 32]),
            at,
        )
        .expect("prepare")
    }

    fn signed_event(content: &str, created_at: u64) -> SignedEvent {
        let mut wire = Nip01EventWire {
            id: "0".repeat(64),
            pubkey: "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df".to_owned(),
            created_at,
            kind: 1,
            tags: vec![],
            content: content.to_owned(),
            sig: "42".repeat(64),
            extra: Default::default(),
        };
        wire.id = wire
            .computed_event_id()
            .expect("canonical event id")
            .to_hex();
        let raw_json = serde_json::json!({
            "id": &wire.id,
            "pubkey": &wire.pubkey,
            "created_at": wire.created_at,
            "kind": wire.kind,
            "tags": &wire.tags,
            "content": &wire.content,
            "sig": &wire.sig,
        })
        .to_string();
        SignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
    }

    fn admission(event: SignedEvent, observed_at: u64) -> EventAdmission {
        let target = Target::new(TransportId::NOSTR, "wss://atomic.example").expect("target");
        let provenance = EventProvenance::new(
            TransportId::NOSTR,
            target.fingerprint().clone(),
            observed_at,
        )
        .expect("provenance");
        EventAdmission::raw(ObservedEvent::new(event, provenance))
    }

    fn delivery_request(event: SignedEvent) -> DeliveryRequest {
        DeliveryRequest::new(
            "atomic-delivery-request",
            DeliveryPayload::new(event),
            TargetSet::new(vec![
                Target::nostr_relay("wss://one.atomic.example").expect("first target"),
                Target::nostr_relay("wss://two.atomic.example").expect("second target"),
            ])
            .expect("target set"),
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
            10_000,
        )
        .expect("delivery request")
    }

    fn atomic_commit(byte: u8, at: u64, workflow: AtomicWorkflow) -> AtomicCommit {
        AtomicCommit::new(
            AtomicCommitId::new([byte; 16]).expect("commit id"),
            AtomicCommitDigest::new([byte; 32]),
            at,
            workflow,
        )
        .expect("atomic commit")
    }

    fn checkpoint(sequence: u64, at: u64) -> ProjectionCheckpoint {
        ProjectionCheckpoint::new(
            ProjectionId::parse("atomic_projection").expect("projection id"),
            ProjectionGeneration::new([81; 32]).expect("projection generation"),
            Some(EventPosition::new(
                SourceGeneration::new([71; 32]).expect("source generation"),
                EventSequence::new(sequence).expect("sequence"),
            )),
            sequence,
            at,
        )
        .expect("checkpoint")
    }

    #[tokio::test]
    async fn workflows_commit_replay_and_reconstruct_original_receipts() {
        let store = store(EventStoreMode::ReadWrite).await;
        let prepared_request = atomic_commit(1, 100, AtomicWorkflow::Prepared(prepare(1, 100)));
        let prepared = store
            .commit(prepared_request.clone())
            .await
            .expect("prepare commit");
        assert_eq!(prepared.disposition(), AtomicCommitDisposition::Committed);

        let outbound_event = signed_event("atomic outbound", 1_800_001_001);
        let signed = store
            .commit(atomic_commit(
                2,
                110,
                AtomicWorkflow::Signed(Box::new(CommitSigned::new(
                    instance(1),
                    JournalRevision::INITIAL,
                    outbound_event.clone(),
                ))),
            ))
            .await
            .expect("signed commit");
        assert!(matches!(
            signed.outcome(),
            AtomicCommitOutcome::Signed { .. }
        ));

        let replay = store
            .commit(prepared_request.clone())
            .await
            .expect("prepared replay");
        assert_eq!(replay.disposition(), AtomicCommitDisposition::Replay);
        assert_eq!(replay.outcome(), prepared.outcome());
        let conflicting = AtomicCommit::new(
            prepared_request.commit_id(),
            AtomicCommitDigest::new([99; 32]),
            100,
            AtomicWorkflow::Prepared(prepare(1, 100)),
        )
        .expect("conflicting request");
        assert_eq!(
            store.commit(conflicting).await,
            Err(Error::AtomicCommitConflict)
        );

        let outbound_admission = admission(outbound_event.clone(), 120);
        let enqueue = EnqueueOutboxItem::new(
            OutboxItemId::new([1; 16]).expect("item id"),
            instance(1),
            DeliveryPlanDigest::new([1; 32]),
            delivery_request(outbound_event.clone()),
            120,
        )
        .expect("enqueue");
        let enqueued = store
            .commit(atomic_commit(
                3,
                120,
                AtomicWorkflow::Enqueued(Box::new(
                    CommitEnqueued::new(
                        instance(1),
                        JournalRevision::new(2).expect("revision"),
                        outbound_admission,
                        enqueue,
                        120,
                    )
                    .expect("enqueued workflow"),
                )),
            ))
            .await
            .expect("enqueue commit");
        assert!(matches!(
            enqueued.outcome(),
            AtomicCommitOutcome::Enqueued { .. }
        ));

        let claimed = store
            .claim(
                ClaimOutboxItems::new(
                    LeaseOwner::parse("atomic-worker").expect("owner"),
                    LeaseId::new([9; 16]).expect("lease seed"),
                    130,
                    200,
                    1,
                )
                .expect("claim request"),
            )
            .await
            .expect("claim")
            .pop()
            .expect("claimed item");
        let delivery_receipt = DeliveryReceipt::for_request(
            claimed.record().request(),
            claimed
                .record()
                .request()
                .target_set()
                .targets()
                .iter()
                .cloned()
                .map(|target| DeliveryTargetReceipt::attempted(target, DeliveryOutcome::accepted()))
                .collect(),
        )
        .expect("delivery receipt");
        let delivered = store
            .commit(atomic_commit(
                4,
                140,
                AtomicWorkflow::Delivered(Box::new(
                    DeliveryAttemptEvidence::new(
                        claimed.record().item_id(),
                        claimed.lease().id(),
                        claimed.record().revision(),
                        DeliveryAttempt::FIRST,
                        delivery_receipt,
                        140,
                    )
                    .expect("delivery evidence"),
                )),
            ))
            .await
            .expect("delivery commit");
        assert!(matches!(
            delivered.outcome(),
            AtomicCommitOutcome::Delivered { .. }
        ));

        let inbound_event = signed_event("atomic inbound", 1_800_001_002);
        let ingested = store
            .commit(atomic_commit(
                5,
                150,
                AtomicWorkflow::Ingested(Box::new(CommitIngested::new(
                    admission(inbound_event, 150),
                    Some(checkpoint(2, 150)),
                ))),
            ))
            .await
            .expect("ingest commit");
        assert!(matches!(
            ingested.outcome(),
            AtomicCommitOutcome::Ingested {
                projection: Some(_),
                ..
            }
        ));

        for (byte, expected) in [
            (1, &prepared),
            (2, &signed),
            (3, &enqueued),
            (4, &delivered),
            (5, &ingested),
        ] {
            let stored = store
                .receipt(AtomicCommitId::new([byte; 16]).expect("commit id"))
                .await
                .expect("receipt lookup")
                .expect("stored receipt");
            assert_eq!(stored, *expected);
        }
    }

    #[tokio::test]
    async fn every_ingest_mutation_fault_rolls_back_all_prior_statements() {
        let fault_points = [
            (
                "CREATE TEMP TRIGGER atomic_fault_0 BEFORE UPDATE ON radroots_runtime_source_generations BEGIN SELECT RAISE(ABORT, 'atomic fault'); END",
                "radroots_runtime_source_generations",
            ),
            (
                "CREATE TEMP TRIGGER atomic_fault_1 BEFORE INSERT ON radroots_runtime_events BEGIN SELECT RAISE(ABORT, 'atomic fault'); END",
                "radroots_runtime_events",
            ),
            (
                "CREATE TEMP TRIGGER atomic_fault_2 BEFORE INSERT ON radroots_runtime_event_provenance BEGIN SELECT RAISE(ABORT, 'atomic fault'); END",
                "radroots_runtime_event_provenance",
            ),
            (
                "CREATE TEMP TRIGGER atomic_fault_3 BEFORE INSERT ON radroots_runtime_projection_checkpoints BEGIN SELECT RAISE(ABORT, 'atomic fault'); END",
                "radroots_runtime_projection_checkpoints",
            ),
            (
                "CREATE TEMP TRIGGER atomic_fault_4 BEFORE INSERT ON radroots_runtime_atomic_commits BEGIN SELECT RAISE(ABORT, 'atomic fault'); END",
                "radroots_runtime_atomic_commits",
            ),
        ];
        for (index, (trigger, table)) in fault_points.into_iter().enumerate() {
            let store = store(EventStoreMode::ReadWrite).await;
            sqlx::raw_sql(trigger)
                .execute(store.pool())
                .await
                .expect("fault trigger");
            let event = signed_event("fault injection", 1_800_002_000 + index as u64);
            let result = store
                .commit(atomic_commit(
                    20 + u8::try_from(index).expect("fault index"),
                    200,
                    AtomicWorkflow::Ingested(Box::new(CommitIngested::new(
                        admission(event, 200),
                        Some(checkpoint(1, 200)),
                    ))),
                ))
                .await;
            assert_eq!(result, Err(Error::BackendUnavailable), "fault at {table}");
            for (durable_table, count_query) in [
                (
                    "radroots_runtime_events",
                    "SELECT COUNT(*) FROM radroots_runtime_events",
                ),
                (
                    "radroots_runtime_event_provenance",
                    "SELECT COUNT(*) FROM radroots_runtime_event_provenance",
                ),
                (
                    "radroots_runtime_projection_checkpoints",
                    "SELECT COUNT(*) FROM radroots_runtime_projection_checkpoints",
                ),
                (
                    "radroots_runtime_atomic_commits",
                    "SELECT COUNT(*) FROM radroots_runtime_atomic_commits",
                ),
            ] {
                let count = sqlx::query_scalar::<_, i64>(count_query)
                    .fetch_one(store.pool())
                    .await
                    .expect("durable row count");
                assert_eq!(count, 0, "partial row remained in {durable_table}");
            }
            let sequence = sqlx::query_scalar::<_, i64>(
                "SELECT sequence_head FROM radroots_runtime_source_generations",
            )
            .fetch_one(store.pool())
            .await
            .expect("sequence head");
            assert_eq!(sequence, 0, "sequence advanced at {table}");
        }
    }

    #[tokio::test]
    async fn journal_and_enqueue_faults_restore_the_precommit_state() {
        let journal_store = store(EventStoreMode::ReadWrite).await;
        sqlx::raw_sql(
            "CREATE TEMP TRIGGER atomic_journal_insert_fault BEFORE INSERT ON radroots_runtime_journal_operations BEGIN SELECT RAISE(ABORT, 'atomic fault'); END",
        )
        .execute(journal_store.pool())
        .await
        .expect("journal insert fault");
        assert_eq!(
            journal_store
                .commit(atomic_commit(
                    50,
                    500,
                    AtomicWorkflow::Prepared(prepare(50, 500)),
                ))
                .await,
            Err(Error::BackendUnavailable)
        );
        assert!(
            journal_store
                .operation(instance(50))
                .await
                .expect("journal lookup")
                .is_none()
        );

        for (index, trigger) in [
            "CREATE TEMP TRIGGER atomic_outbox_item_fault BEFORE INSERT ON radroots_runtime_outbox_items BEGIN SELECT RAISE(ABORT, 'atomic fault'); END",
            "CREATE TEMP TRIGGER atomic_outbox_target_fault BEFORE INSERT ON radroots_runtime_outbox_targets BEGIN SELECT RAISE(ABORT, 'atomic fault'); END",
            "CREATE TEMP TRIGGER atomic_journal_update_fault BEFORE UPDATE ON radroots_runtime_journal_operations BEGIN SELECT RAISE(ABORT, 'atomic fault'); END",
        ]
        .into_iter()
        .enumerate()
        {
            let store = store(EventStoreMode::ReadWrite).await;
            let operation_byte = 60 + u8::try_from(index).expect("case index");
            let outbound_event =
                signed_event("enqueue fault", 1_800_003_000 + index as u64);
            let prepared = store
                .prepare(prepare(operation_byte, 600))
                .await
                .expect("prepare")
                .record()
                .clone();
            let signed = store
                .transition(JournalTransition::signed(
                    instance(operation_byte),
                    prepared.revision(),
                    *outbound_event.id(),
                ))
                .await
                .expect("sign journal");
            sqlx::raw_sql(trigger)
                .execute(store.pool())
                .await
                .expect("enqueue fault trigger");
            let enqueue = EnqueueOutboxItem::new(
                OutboxItemId::new([operation_byte; 16]).expect("item id"),
                instance(operation_byte),
                DeliveryPlanDigest::new([operation_byte; 32]),
                delivery_request(outbound_event.clone()),
                610,
            )
            .expect("enqueue");
            let request = atomic_commit(
                70 + u8::try_from(index).expect("case index"),
                610,
                AtomicWorkflow::Enqueued(Box::new(
                    CommitEnqueued::new(
                        instance(operation_byte),
                        signed.revision(),
                        admission(outbound_event, 610),
                        enqueue,
                        610,
                    )
                    .expect("enqueue workflow"),
                )),
            );
            assert_eq!(store.commit(request).await, Err(Error::BackendUnavailable));
            let durable = store
                .operation(instance(operation_byte))
                .await
                .expect("journal lookup")
                .expect("signed journal");
            assert_eq!(durable.state().stage(), JournalStage::Signed);
            assert_eq!(durable.revision(), signed.revision());
            assert_eq!(
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM radroots_runtime_events")
                    .fetch_one(store.pool())
                    .await
                    .expect("event count"),
                0
            );
            assert_eq!(
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM radroots_runtime_outbox_items")
                    .fetch_one(store.pool())
                    .await
                    .expect("outbox count"),
                0
            );
            assert_eq!(
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM radroots_runtime_atomic_commits")
                    .fetch_one(store.pool())
                    .await
                    .expect("commit count"),
                0
            );
        }
    }

    #[tokio::test]
    async fn delivery_faults_restore_the_claimed_outbox_record() {
        for (index, trigger) in [
            "CREATE TEMP TRIGGER atomic_delivery_update_fault BEFORE UPDATE ON radroots_runtime_outbox_items BEGIN SELECT RAISE(ABORT, 'atomic fault'); END",
            "CREATE TEMP TRIGGER atomic_delivery_evidence_fault BEFORE INSERT ON radroots_runtime_delivery_evidence BEGIN SELECT RAISE(ABORT, 'atomic fault'); END",
        ]
        .into_iter()
        .enumerate()
        {
            let store = store(EventStoreMode::ReadWrite).await;
            let operation_byte = 80 + u8::try_from(index).expect("case index");
            store
                .prepare(prepare(operation_byte, 800))
                .await
                .expect("prepare");
            let request = delivery_request(signed_event(
                "delivery fault",
                1_800_004_000 + index as u64,
            ));
            store
                .enqueue(
                    EnqueueOutboxItem::new(
                        OutboxItemId::new([operation_byte; 16]).expect("item id"),
                        instance(operation_byte),
                        DeliveryPlanDigest::new([operation_byte; 32]),
                        request.clone(),
                        810,
                    )
                    .expect("enqueue"),
                )
                .await
                .expect("enqueue");
            let claimed = store
                .claim(
                    ClaimOutboxItems::new(
                        LeaseOwner::parse("atomic-fault-worker").expect("owner"),
                        LeaseId::new([operation_byte; 16]).expect("lease seed"),
                        820,
                        900,
                        1,
                    )
                    .expect("claim request"),
                )
                .await
                .expect("claim")
                .pop()
                .expect("claimed item");
            sqlx::raw_sql(trigger)
                .execute(store.pool())
                .await
                .expect("delivery fault trigger");
            let receipt = DeliveryReceipt::for_request(
                &request,
                request
                    .target_set()
                    .targets()
                    .iter()
                    .cloned()
                    .map(|target| {
                        DeliveryTargetReceipt::attempted(target, DeliveryOutcome::accepted())
                    })
                    .collect(),
            )
            .expect("delivery receipt");
            let evidence = DeliveryAttemptEvidence::new(
                claimed.record().item_id(),
                claimed.lease().id(),
                claimed.record().revision(),
                DeliveryAttempt::FIRST,
                receipt,
                830,
            )
            .expect("delivery evidence");
            assert_eq!(
                store
                    .commit(atomic_commit(
                        90 + u8::try_from(index).expect("case index"),
                        830,
                        AtomicWorkflow::Delivered(Box::new(evidence)),
                    ))
                    .await,
                Err(Error::BackendUnavailable)
            );
            assert_eq!(
                store
                    .item(claimed.record().item_id())
                    .await
                    .expect("item lookup")
                    .expect("claimed record"),
                *claimed.record()
            );
            assert_eq!(
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM radroots_runtime_delivery_evidence")
                    .fetch_one(store.pool())
                    .await
                    .expect("evidence count"),
                0
            );
        }
    }

    #[tokio::test]
    async fn read_only_mode_rejects_atomic_mutation() {
        let store = store(EventStoreMode::ReadOnly).await;
        assert_eq!(
            store
                .commit(atomic_commit(
                    40,
                    400,
                    AtomicWorkflow::Prepared(prepare(40, 400)),
                ))
                .await,
            Err(Error::BackendUnavailable)
        );
    }

    #[test]
    fn rollback_failure_never_replaces_the_primary_failure() {
        let primary = Error::JournalRevisionConflict;
        assert_eq!(
            preserve_primary(primary.clone(), Err::<(), _>("rollback failed")),
            primary
        );
    }

    #[test]
    fn receipt_decoding_rejects_trailing_or_unknown_data() {
        let record = prepare(50, 500).into_record().expect("record");
        let outcome = AtomicCommitOutcome::Prepared { journal: record };
        let mut encoded = encode_outcome(&outcome).expect("encoded outcome");
        encoded.push(0);
        assert_eq!(
            decode_outcome(encoded.as_slice()),
            Err(Error::AtomicCommitFailed)
        );
        assert_eq!(decode_outcome(&[99, 0]), Err(Error::AtomicCommitFailed));
    }
}

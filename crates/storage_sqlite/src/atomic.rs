use crate::{SqliteStorage, projection};
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
};
use sqlx::{Row, Sqlite};

const RECEIPT_FORMAT_VERSION: u8 = 1;
const RECEIPT_MAX_BYTES: usize = 4 * 1024 * 1024;

#[cfg_attr(coverage_nightly, coverage(off))]
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
        if [
            committed.digest() != request.digest(),
            committed.outcome().kind() != request.workflow().kind(),
        ]
        .contains(&true)
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
        AtomicWorkflowKind::Ingested => "ingested",
    }
}

fn workflow_kind(value: &str) -> Result<AtomicWorkflowKind, Error> {
    match value.as_bytes() {
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
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::migration::runtime::{MIGRATIONS, migration_sql};
    use radroots_event::{SignedEvent, wire::Nip01EventWire};
    use radroots_storage::{
        atomic::{AtomicWorkflow, CommitIngested},
        event::{EventAdmission, SourceGeneration},
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

    fn signed_event() -> SignedEvent {
        let mut wire = Nip01EventWire {
            id: "0".repeat(64),
            pubkey: "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df".to_owned(),
            created_at: 1_800_001_001,
            kind: 1,
            tags: vec![],
            content: "atomic inbound".to_owned(),
            sig: "42".repeat(64),
            extra: Default::default(),
        };
        wire.id = wire.computed_event_id().expect("event id").to_hex();
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

    fn admission(observed_at: u64) -> EventAdmission {
        let target = Target::new(TransportId::NOSTR, "wss://atomic.example").expect("target");
        let provenance = EventProvenance::new(
            TransportId::NOSTR,
            target.fingerprint().clone(),
            observed_at,
        )
        .expect("provenance");
        EventAdmission::raw(ObservedEvent::new(signed_event(), provenance))
    }

    fn commit(byte: u8, digest: u8, workflow: AtomicWorkflow) -> AtomicCommit {
        AtomicCommit::new(
            AtomicCommitId::new([byte; 16]).expect("commit id"),
            AtomicCommitDigest::new([digest; 32]),
            100,
            workflow,
        )
        .expect("atomic commit")
    }

    fn checkpoint() -> ProjectionCheckpoint {
        ProjectionCheckpoint::new(
            ProjectionId::parse("atomic_projection").expect("projection id"),
            ProjectionGeneration::new([81; 32]).expect("projection generation"),
            None,
            1,
            100,
        )
        .expect("checkpoint")
    }

    #[tokio::test]
    async fn inbound_commit_is_atomic_replayable_and_reconstructable() {
        let store = store(EventStoreMode::ReadWrite).await;
        let request = commit(
            1,
            1,
            AtomicWorkflow::Ingested(Box::new(CommitIngested::new(
                admission(90),
                Some(checkpoint()),
            ))),
        );
        let committed = store.commit(request.clone()).await.expect("commit");
        assert_eq!(committed.disposition(), AtomicCommitDisposition::Committed);
        assert!(matches!(
            committed.outcome(),
            AtomicCommitOutcome::Ingested {
                projection: Some(_),
                ..
            }
        ));

        let replay = store.commit(request.clone()).await.expect("replay");
        assert_eq!(replay.disposition(), AtomicCommitDisposition::Replay);
        assert_eq!(replay.outcome(), committed.outcome());
        let reconstructed = store
            .receipt(request.commit_id())
            .await
            .expect("receipt lookup")
            .expect("receipt");
        assert_eq!(reconstructed.outcome(), committed.outcome());

        let conflict = commit(
            1,
            2,
            AtomicWorkflow::Ingested(Box::new(CommitIngested::new(admission(90), None))),
        );
        assert_eq!(
            store.commit(conflict).await,
            Err(Error::AtomicCommitConflict)
        );
    }

    #[tokio::test]
    async fn read_only_mode_rejects_atomic_mutation() {
        let store = store(EventStoreMode::ReadOnly).await;
        let request = commit(
            2,
            2,
            AtomicWorkflow::Ingested(Box::new(CommitIngested::new(admission(90), None))),
        );
        assert_eq!(store.commit(request).await, Err(Error::BackendUnavailable));
    }

    #[test]
    fn receipt_decoder_rejects_retired_and_trailing_payloads() {
        assert_eq!(
            decode_outcome(&vec![0; RECEIPT_MAX_BYTES + 1]),
            Err(Error::AtomicCommitFailed)
        );
        assert_eq!(decode_outcome(&[0]), Err(Error::AtomicCommitFailed));
        assert_eq!(
            decode_outcome(&[RECEIPT_FORMAT_VERSION, 0]),
            Err(Error::AtomicCommitFailed)
        );
        let outcome = AtomicCommitOutcome::Ingested {
            admission: AdmissionReceipt::new(
                radroots_storage::event::EventId::from_bytes([1; 32]),
                EventPosition::new(
                    SourceGeneration::new([2; 32]).expect("generation"),
                    EventSequence::new(1).expect("sequence"),
                ),
                AdmissionStage::Raw,
                AdmissionDisposition::Inserted,
            ),
            projection: None,
        };
        let mut bytes = encode_outcome(&outcome).expect("encode");
        bytes.push(0);
        assert_eq!(decode_outcome(&bytes), Err(Error::AtomicCommitFailed));
    }
}

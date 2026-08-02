use crate::SqliteStorage;
use radroots_storage::{
    Error, Journal,
    journal::{
        BoxFuture, CancellationState, EventId, IdempotencyDigest, IdempotencyKey, JournalRevision,
        JournalState, JournalTransition, OperationId, OperationInstanceId, OperationRecord,
        PrepareDisposition, PrepareOperation, PrepareReceipt, RECOVERABLE_QUERY_LIMIT_MAX,
        RecoveryPoint, RecoveryReason, RecoveryRecord,
    },
};
use sqlx::{Row, Sqlite};

impl Journal for SqliteStorage {
    fn prepare(&self, operation: PrepareOperation) -> BoxFuture<'_, Result<PrepareReceipt, Error>> {
        Box::pin(async move {
            self.require_journal_writer()?;
            let mut transaction = self
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            let receipt = prepare_transaction(&mut transaction, operation).await?;
            transaction.commit().await.map_err(map_backend)?;
            Ok(receipt)
        })
    }

    fn operation(
        &self,
        instance_id: OperationInstanceId,
    ) -> BoxFuture<'_, Result<Option<OperationRecord>, Error>> {
        Box::pin(async move {
            sqlx::query("SELECT * FROM radroots_runtime_journal_operations WHERE instance_id = ?")
                .bind(instance_id.as_bytes().as_slice())
                .fetch_optional(self.pool())
                .await
                .map_err(map_backend)?
                .as_ref()
                .map(decode_record)
                .transpose()
        })
    }

    fn by_idempotency_key(
        &self,
        operation_id: OperationId,
        idempotency_key: IdempotencyKey,
    ) -> BoxFuture<'_, Result<Option<OperationRecord>, Error>> {
        Box::pin(async move {
            sqlx::query(
                "SELECT * FROM radroots_runtime_journal_operations
                 WHERE operation_id = ? AND idempotency_key = ?",
            )
            .bind(operation_id.as_str().as_bytes())
            .bind(idempotency_key.as_str())
            .fetch_optional(self.pool())
            .await
            .map_err(map_backend)?
            .as_ref()
            .map(decode_record)
            .transpose()
        })
    }

    fn transition(
        &self,
        transition: JournalTransition,
    ) -> BoxFuture<'_, Result<OperationRecord, Error>> {
        Box::pin(async move {
            self.require_journal_writer()?;
            let mut transaction = self
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            let next = transition_transaction(&mut transaction, transition).await?;
            transaction.commit().await.map_err(map_backend)?;
            Ok(next)
        })
    }

    fn recoverable(&self, limit: u16) -> BoxFuture<'_, Result<Vec<OperationRecord>, Error>> {
        Box::pin(async move {
            if limit == 0 || limit > RECOVERABLE_QUERY_LIMIT_MAX {
                return Err(Error::InvalidJournalQueryLimit);
            }
            sqlx::query(
                "SELECT * FROM radroots_runtime_journal_operations
                 WHERE stage = 'recoverable'
                 ORDER BY updated_at_unix_ms, instance_id LIMIT ?",
            )
            .bind(i64::from(limit))
            .fetch_all(self.pool())
            .await
            .map_err(map_backend)?
            .iter()
            .map(decode_record)
            .collect()
        })
    }
}

pub(crate) async fn prepare_transaction(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    operation: PrepareOperation,
) -> Result<PrepareReceipt, Error> {
    if let Some(row) = sqlx::query(
        "SELECT * FROM radroots_runtime_journal_operations
         WHERE idempotency_key = ?",
    )
    .bind(operation.idempotency_key().as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_backend)?
    {
        let record = decode_record(&row)?;
        if record.operation_id() != operation.operation_id()
            || record.input_digest() != operation.input_digest()
            || record.instance_id() != operation.instance_id()
        {
            return Err(Error::IdempotencyConflict);
        }
        return Ok(PrepareReceipt::new(PrepareDisposition::Replay, record));
    }
    if sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM radroots_runtime_journal_operations WHERE instance_id = ?",
    )
    .bind(operation.instance_id().as_bytes().as_slice())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_backend)?
    .is_some()
    {
        return Err(Error::OperationIdentityMismatch);
    }

    let record = operation.into_record()?;
    insert_record(transaction, &record).await?;
    Ok(PrepareReceipt::new(PrepareDisposition::Created, record))
}

pub(crate) async fn transition_transaction(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    transition: JournalTransition,
) -> Result<OperationRecord, Error> {
    let row =
        sqlx::query("SELECT * FROM radroots_runtime_journal_operations WHERE instance_id = ?")
            .bind(transition.instance_id().as_bytes().as_slice())
            .fetch_optional(&mut **transaction)
            .await
            .map_err(map_backend)?
            .ok_or(Error::OperationNotFound)?;
    let current = decode_record(&row)?;
    let next = current.transition(&transition)?;
    let (stage, event_id, recovery, committed_at) = encode_state(next.state());
    let result = sqlx::query(
        "UPDATE radroots_runtime_journal_operations SET
           revision = ?, stage = ?, event_id = ?, recovery_record = ?,
           cancellation_state = ?, committed_at_unix_ms = ?, updated_at_unix_ms = ?
         WHERE instance_id = ? AND revision = ?",
    )
    .bind(i64_from_u64(next.revision().get())?)
    .bind(stage)
    .bind(event_id)
    .bind(recovery)
    .bind(cancellation_name(next.cancellation()))
    .bind(committed_at.map(i64_from_u64).transpose()?)
    .bind(i64_from_u64(updated_at(&next))?)
    .bind(next.instance_id().as_bytes().as_slice())
    .bind(i64_from_u64(current.revision().get())?)
    .execute(&mut **transaction)
    .await
    .map_err(map_backend)?;
    if result.rows_affected() != 1 {
        return Err(Error::JournalRevisionConflict);
    }
    Ok(next)
}

impl SqliteStorage {
    fn require_journal_writer(&self) -> Result<(), Error> {
        if self.event_mode() == radroots_storage::status::EventStoreMode::ReadOnly {
            return Err(Error::BackendUnavailable);
        }
        Ok(())
    }
}

async fn insert_record(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    record: &OperationRecord,
) -> Result<(), Error> {
    let (stage, event_id, recovery, committed_at) = encode_state(record.state());
    sqlx::query(
        "INSERT INTO radroots_runtime_journal_operations (
           instance_id, operation_id, idempotency_key, input_digest,
           prepared_at_unix_ms, revision, stage, event_id, recovery_record,
           cancellation_state, committed_at_unix_ms, updated_at_unix_ms
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(record.instance_id().as_bytes().as_slice())
    .bind(record.operation_id().as_str().as_bytes())
    .bind(record.idempotency_key().as_str())
    .bind(record.input_digest().as_bytes().as_slice())
    .bind(i64_from_u64(record.prepared_at_unix_ms())?)
    .bind(i64_from_u64(record.revision().get())?)
    .bind(stage)
    .bind(event_id)
    .bind(recovery)
    .bind(cancellation_name(record.cancellation()))
    .bind(committed_at.map(i64_from_u64).transpose()?)
    .bind(i64_from_u64(updated_at(record))?)
    .execute(&mut **transaction)
    .await
    .map_err(map_backend)?;
    Ok(())
}

fn decode_record(row: &sqlx::sqlite::SqliteRow) -> Result<OperationRecord, Error> {
    let instance_id = OperationInstanceId::new(array(
        row.try_get::<Vec<u8>, _>("instance_id")
            .map_err(map_corrupt)?,
    )?)
    .map_err(|_| Error::CorruptJournalRecord)?;
    let operation_text = String::from_utf8(
        row.try_get::<Vec<u8>, _>("operation_id")
            .map_err(map_corrupt)?,
    )
    .map_err(|_| Error::CorruptJournalRecord)?;
    let operation_id =
        OperationId::parse(operation_text.as_str()).map_err(|_| Error::CorruptJournalRecord)?;
    let idempotency_key = IdempotencyKey::parse(
        row.try_get::<String, _>("idempotency_key")
            .map_err(map_corrupt)?,
    )
    .map_err(|_| Error::CorruptJournalRecord)?;
    let input_digest = IdempotencyDigest::new(array(
        row.try_get::<Vec<u8>, _>("input_digest")
            .map_err(map_corrupt)?,
    )?);
    let prepared_at = u64_from_i64(row.try_get("prepared_at_unix_ms").map_err(map_corrupt)?)?;
    let revision =
        JournalRevision::new(u64_from_i64(row.try_get("revision").map_err(map_corrupt)?)?)
            .map_err(|_| Error::CorruptJournalRecord)?;
    let event_id = row
        .try_get::<Option<Vec<u8>>, _>("event_id")
        .map_err(map_corrupt)?
        .map(|bytes| array(bytes).map(EventId::from_bytes))
        .transpose()?;
    let recovery = row
        .try_get::<Option<Vec<u8>>, _>("recovery_record")
        .map_err(map_corrupt)?
        .map(|bytes| decode_recovery(bytes.as_slice()))
        .transpose()?;
    let committed_at = row
        .try_get::<Option<i64>, _>("committed_at_unix_ms")
        .map_err(map_corrupt)?
        .map(u64_from_i64)
        .transpose()?;
    let state = decode_state(
        row.try_get::<String, _>("stage")
            .map_err(map_corrupt)?
            .as_str(),
        event_id,
        recovery,
        committed_at,
    )?;
    let cancellation = cancellation(
        row.try_get::<String, _>("cancellation_state")
            .map_err(map_corrupt)?
            .as_str(),
    )?;
    OperationRecord::from_parts(
        instance_id,
        operation_id,
        idempotency_key,
        input_digest,
        prepared_at,
        revision,
        state,
        cancellation,
    )
    .map_err(|_| Error::CorruptJournalRecord)
}

pub(crate) fn encode_record_snapshot(record: &OperationRecord) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::with_capacity(160);
    bytes.push(1);
    bytes.extend_from_slice(record.instance_id().as_bytes());
    snapshot_put_string(&mut bytes, record.operation_id().as_str())?;
    snapshot_put_string(&mut bytes, record.idempotency_key().as_str())?;
    bytes.extend_from_slice(record.input_digest().as_bytes());
    bytes.extend_from_slice(&record.prepared_at_unix_ms().to_be_bytes());
    bytes.extend_from_slice(&record.revision().get().to_be_bytes());
    match record.state() {
        JournalState::Prepared => bytes.push(0),
        JournalState::Signed { event_id } => {
            bytes.push(1);
            bytes.extend_from_slice(event_id.as_bytes());
        }
        JournalState::Recoverable(recovery) => {
            bytes.push(2);
            snapshot_put_blob(&mut bytes, encode_recovery(recovery).as_slice())?;
        }
        JournalState::Committed {
            event_id,
            committed_at_unix_ms,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(event_id.as_bytes());
            bytes.extend_from_slice(&committed_at_unix_ms.to_be_bytes());
        }
    }
    bytes.push(match record.cancellation() {
        CancellationState::NotRequested => 0,
        CancellationState::CancelledBeforeCommit => 1,
        CancellationState::ObservedAfterCommit => 2,
    });
    Ok(bytes)
}

pub(crate) fn decode_record_snapshot(bytes: &[u8]) -> Result<OperationRecord, Error> {
    let mut offset = 0;
    if take_byte(bytes, &mut offset)? != 1 {
        return Err(Error::CorruptJournalRecord);
    }
    let instance_id = OperationInstanceId::new(take_array(bytes, &mut offset)?)
        .map_err(|_| Error::CorruptJournalRecord)?;
    let operation_id = OperationId::parse(snapshot_take_string(bytes, &mut offset)?)
        .map_err(|_| Error::CorruptJournalRecord)?;
    let idempotency_key = IdempotencyKey::parse(snapshot_take_string(bytes, &mut offset)?)
        .map_err(|_| Error::CorruptJournalRecord)?;
    let input_digest = IdempotencyDigest::new(take_array(bytes, &mut offset)?);
    let prepared_at_unix_ms = u64::from_be_bytes(take_array(bytes, &mut offset)?);
    let revision = JournalRevision::new(u64::from_be_bytes(take_array(bytes, &mut offset)?))
        .map_err(|_| Error::CorruptJournalRecord)?;
    let state = match take_byte(bytes, &mut offset)? {
        0 => JournalState::Prepared,
        1 => JournalState::Signed {
            event_id: EventId::from_bytes(take_array(bytes, &mut offset)?),
        },
        2 => JournalState::Recoverable(decode_recovery(snapshot_take_blob(bytes, &mut offset)?)?),
        3 => JournalState::Committed {
            event_id: EventId::from_bytes(take_array(bytes, &mut offset)?),
            committed_at_unix_ms: u64::from_be_bytes(take_array(bytes, &mut offset)?),
        },
        _ => return Err(Error::CorruptJournalRecord),
    };
    let cancellation = match take_byte(bytes, &mut offset)? {
        0 => CancellationState::NotRequested,
        1 => CancellationState::CancelledBeforeCommit,
        2 => CancellationState::ObservedAfterCommit,
        _ => return Err(Error::CorruptJournalRecord),
    };
    if offset != bytes.len() {
        return Err(Error::CorruptJournalRecord);
    }
    OperationRecord::from_parts(
        instance_id,
        operation_id,
        idempotency_key,
        input_digest,
        prepared_at_unix_ms,
        revision,
        state,
        cancellation,
    )
    .map_err(|_| Error::CorruptJournalRecord)
}

fn snapshot_put_string(bytes: &mut Vec<u8>, value: &str) -> Result<(), Error> {
    snapshot_put_blob(bytes, value.as_bytes())
}

fn snapshot_put_blob(bytes: &mut Vec<u8>, value: &[u8]) -> Result<(), Error> {
    let length = u16::try_from(value.len()).map_err(|_| Error::CorruptJournalRecord)?;
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(value);
    Ok(())
}

fn snapshot_take_string<'a>(bytes: &'a [u8], offset: &mut usize) -> Result<&'a str, Error> {
    core::str::from_utf8(snapshot_take_blob(bytes, offset)?)
        .map_err(|_| Error::CorruptJournalRecord)
}

fn snapshot_take_blob<'a>(bytes: &'a [u8], offset: &mut usize) -> Result<&'a [u8], Error> {
    let length = usize::from(u16::from_be_bytes(take_array(bytes, offset)?));
    let end = offset
        .checked_add(length)
        .ok_or(Error::CorruptJournalRecord)?;
    let value = bytes.get(*offset..end).ok_or(Error::CorruptJournalRecord)?;
    *offset = end;
    Ok(value)
}

type EncodedState = (&'static str, Option<Vec<u8>>, Option<Vec<u8>>, Option<u64>);

fn encode_state(state: &JournalState) -> EncodedState {
    match state {
        JournalState::Prepared => ("prepared", None, None, None),
        JournalState::Signed { event_id } => {
            ("signed", Some(event_id.as_bytes().to_vec()), None, None)
        }
        JournalState::Recoverable(record) => {
            let event_id = match record.point() {
                RecoveryPoint::Prepared => None,
                RecoveryPoint::Signed { event_id } => Some(event_id.as_bytes().to_vec()),
            };
            ("recoverable", event_id, Some(encode_recovery(record)), None)
        }
        JournalState::Committed {
            event_id,
            committed_at_unix_ms,
        } => (
            "committed",
            Some(event_id.as_bytes().to_vec()),
            None,
            Some(*committed_at_unix_ms),
        ),
    }
}

fn decode_state(
    stage: &str,
    event_id: Option<EventId>,
    recovery: Option<RecoveryRecord>,
    committed_at: Option<u64>,
) -> Result<JournalState, Error> {
    match (stage, event_id, recovery, committed_at) {
        ("prepared", None, None, None) => Ok(JournalState::Prepared),
        ("signed", Some(event_id), None, None) => Ok(JournalState::Signed { event_id }),
        ("recoverable", event_id, Some(recovery), None)
            if recovery_event_id(&recovery) == event_id =>
        {
            Ok(JournalState::Recoverable(recovery))
        }
        ("committed", Some(event_id), None, Some(committed_at_unix_ms)) => {
            Ok(JournalState::Committed {
                event_id,
                committed_at_unix_ms,
            })
        }
        _ => Err(Error::CorruptJournalRecord),
    }
}

fn recovery_event_id(record: &RecoveryRecord) -> Option<EventId> {
    match record.point() {
        RecoveryPoint::Prepared => None,
        RecoveryPoint::Signed { event_id } => Some(*event_id),
    }
}

fn encode_recovery(record: &RecoveryRecord) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(48);
    bytes.push(1);
    match record.point() {
        RecoveryPoint::Prepared => bytes.push(0),
        RecoveryPoint::Signed { event_id } => {
            bytes.push(1);
            bytes.extend_from_slice(event_id.as_bytes());
        }
    }
    bytes.push(recovery_reason_byte(record.reason()));
    bytes.extend_from_slice(&record.attempt().to_be_bytes());
    match record.retry_not_before_unix_ms() {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        None => bytes.push(0),
    }
    bytes
}

fn decode_recovery(bytes: &[u8]) -> Result<RecoveryRecord, Error> {
    let mut offset = 0;
    if take_byte(bytes, &mut offset)? != 1 {
        return Err(Error::CorruptJournalRecord);
    }
    let point = match take_byte(bytes, &mut offset)? {
        0 => RecoveryPoint::Prepared,
        1 => RecoveryPoint::Signed {
            event_id: EventId::from_bytes(take_array(bytes, &mut offset)?),
        },
        _ => return Err(Error::CorruptJournalRecord),
    };
    let reason = recovery_reason(take_byte(bytes, &mut offset)?)?;
    let attempt = u32::from_be_bytes(take_array(bytes, &mut offset)?);
    let retry = match take_byte(bytes, &mut offset)? {
        0 => None,
        1 => Some(u64::from_be_bytes(take_array(bytes, &mut offset)?)),
        _ => return Err(Error::CorruptJournalRecord),
    };
    if offset != bytes.len() {
        return Err(Error::CorruptJournalRecord);
    }
    RecoveryRecord::new(point, reason, attempt, retry).map_err(|_| Error::CorruptJournalRecord)
}

const fn recovery_reason_byte(reason: RecoveryReason) -> u8 {
    match reason {
        RecoveryReason::CancelledBeforeCommit => 0,
        RecoveryReason::SignerUnavailable => 1,
        RecoveryReason::TransportUnavailable => 2,
        RecoveryReason::StorageUnavailable => 3,
        RecoveryReason::DeadlineExceeded => 4,
        RecoveryReason::Interrupted => 5,
    }
}

const fn recovery_reason(value: u8) -> Result<RecoveryReason, Error> {
    match value {
        0 => Ok(RecoveryReason::CancelledBeforeCommit),
        1 => Ok(RecoveryReason::SignerUnavailable),
        2 => Ok(RecoveryReason::TransportUnavailable),
        3 => Ok(RecoveryReason::StorageUnavailable),
        4 => Ok(RecoveryReason::DeadlineExceeded),
        5 => Ok(RecoveryReason::Interrupted),
        _ => Err(Error::CorruptJournalRecord),
    }
}

const fn cancellation_name(value: CancellationState) -> &'static str {
    match value {
        CancellationState::NotRequested => "not_requested",
        CancellationState::CancelledBeforeCommit => "cancelled_before_commit",
        CancellationState::ObservedAfterCommit => "observed_after_commit",
    }
}

const fn cancellation(value: &str) -> Result<CancellationState, Error> {
    match value.as_bytes() {
        b"not_requested" => Ok(CancellationState::NotRequested),
        b"cancelled_before_commit" => Ok(CancellationState::CancelledBeforeCommit),
        b"observed_after_commit" => Ok(CancellationState::ObservedAfterCommit),
        _ => Err(Error::CorruptJournalRecord),
    }
}

fn updated_at(record: &OperationRecord) -> u64 {
    match record.state() {
        JournalState::Committed {
            committed_at_unix_ms,
            ..
        } => *committed_at_unix_ms,
        JournalState::Recoverable(recovery) => recovery
            .retry_not_before_unix_ms()
            .unwrap_or(record.prepared_at_unix_ms()),
        JournalState::Prepared | JournalState::Signed { .. } => record.prepared_at_unix_ms(),
    }
}

fn take_byte(bytes: &[u8], offset: &mut usize) -> Result<u8, Error> {
    let value = bytes
        .get(*offset)
        .copied()
        .ok_or(Error::CorruptJournalRecord)?;
    *offset += 1;
    Ok(value)
}

fn take_array<const N: usize>(bytes: &[u8], offset: &mut usize) -> Result<[u8; N], Error> {
    let end = offset.checked_add(N).ok_or(Error::CorruptJournalRecord)?;
    let value = bytes
        .get(*offset..end)
        .ok_or(Error::CorruptJournalRecord)?
        .try_into()
        .map_err(|_| Error::CorruptJournalRecord)?;
    *offset = end;
    Ok(value)
}

fn array<const N: usize>(bytes: Vec<u8>) -> Result<[u8; N], Error> {
    bytes.try_into().map_err(|_| Error::CorruptJournalRecord)
}

fn i64_from_u64(value: u64) -> Result<i64, Error> {
    i64::try_from(value).map_err(|_| Error::CorruptJournalRecord)
}

fn u64_from_i64(value: i64) -> Result<u64, Error> {
    u64::try_from(value).map_err(|_| Error::CorruptJournalRecord)
}

fn map_backend(_: sqlx::Error) -> Error {
    Error::BackendUnavailable
}

fn map_corrupt(_: sqlx::Error) -> Error {
    Error::CorruptJournalRecord
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::runtime::{MIGRATIONS, migration_sql};
    use radroots_storage::{
        Journal, event::SourceGeneration, journal::JournalStage, status::EventStoreMode,
    };
    use sqlx::sqlite::SqlitePoolOptions;

    async fn store(mode: EventStoreMode) -> SqliteStorage {
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
        SqliteStorage::new(
            pool,
            SourceGeneration::new([21; 32]).expect("generation"),
            mode,
        )
    }

    fn instance(byte: u8) -> OperationInstanceId {
        OperationInstanceId::new([byte; 16]).expect("instance")
    }

    fn key(byte: u8) -> IdempotencyKey {
        IdempotencyKey::parse(format!("journal-{byte:02x}")).expect("key")
    }

    fn prepare(
        instance_id: OperationInstanceId,
        key_byte: u8,
        digest: u8,
        at: u64,
    ) -> PrepareOperation {
        PrepareOperation::new(
            instance_id,
            OperationId::SyncPush,
            key(key_byte),
            IdempotencyDigest::new([digest; 32]),
            at,
        )
        .expect("prepare")
    }

    #[tokio::test]
    async fn prepare_replays_exact_identity_and_rejects_conflicts() {
        let store = store(EventStoreMode::ReadWrite).await;
        let request = prepare(instance(1), 1, 2, 100);
        let created = store.prepare(request.clone()).await.expect("created");
        assert_eq!(created.disposition(), PrepareDisposition::Created);
        assert_eq!(created.record().revision(), JournalRevision::INITIAL);
        let replay = store.prepare(request).await.expect("replay");
        assert_eq!(replay.disposition(), PrepareDisposition::Replay);
        assert_eq!(replay.record(), created.record());
        assert_eq!(
            store.prepare(prepare(instance(1), 1, 3, 100)).await,
            Err(Error::IdempotencyConflict)
        );
        assert_eq!(
            store.prepare(prepare(instance(1), 2, 2, 100)).await,
            Err(Error::OperationIdentityMismatch)
        );
        assert_eq!(
            store
                .operation(instance(1))
                .await
                .expect("lookup")
                .expect("record"),
            *created.record()
        );
        assert_eq!(
            store
                .by_idempotency_key(OperationId::SyncPush, key(1))
                .await
                .expect("key lookup")
                .expect("record"),
            *created.record()
        );
        assert!(
            store
                .by_idempotency_key(OperationId::FarmPublish, key(1))
                .await
                .expect("wrong operation lookup")
                .is_none()
        );
    }

    #[tokio::test]
    async fn lifecycle_recovery_commit_and_cancellation_round_trip() {
        let store = store(EventStoreMode::ReadWrite).await;
        let instance_id = instance(3);
        let event_id = EventId::from_bytes([4; 32]);
        let prepared = store
            .prepare(prepare(instance_id, 3, 3, 100))
            .await
            .expect("prepare")
            .record()
            .clone();
        let signed = store
            .transition(JournalTransition::signed(
                instance_id,
                prepared.revision(),
                event_id,
            ))
            .await
            .expect("signed");
        assert_eq!(signed.state().stage(), JournalStage::Signed);
        assert_eq!(
            store
                .transition(JournalTransition::signed(
                    instance_id,
                    prepared.revision(),
                    event_id,
                ))
                .await,
            Err(Error::JournalRevisionConflict)
        );

        let recovery = RecoveryRecord::new(
            RecoveryPoint::Signed { event_id },
            RecoveryReason::TransportUnavailable,
            2,
            Some(200),
        )
        .expect("recovery");
        let recoverable = store
            .transition(JournalTransition::recoverable(
                instance_id,
                signed.revision(),
                recovery.clone(),
            ))
            .await
            .expect("recoverable");
        assert_eq!(
            store.recoverable(10).await.expect("recovery query"),
            vec![recoverable.clone()]
        );
        assert_eq!(recoverable.state(), &JournalState::Recoverable(recovery));

        let resumed = store
            .transition(JournalTransition::resume(
                instance_id,
                recoverable.revision(),
            ))
            .await
            .expect("resume");
        assert_eq!(resumed.state().stage(), JournalStage::Signed);
        let committed = store
            .transition(JournalTransition::committed(
                instance_id,
                resumed.revision(),
                event_id,
                250,
            ))
            .await
            .expect("commit");
        assert_eq!(committed.state().stage(), JournalStage::Committed);
        let cancelled = store
            .transition(JournalTransition::cancelled(
                instance_id,
                committed.revision(),
                260,
            ))
            .await
            .expect("post-commit cancellation");
        assert_eq!(cancelled.state(), committed.state());
        assert_eq!(
            cancelled.cancellation(),
            CancellationState::ObservedAfterCommit
        );
        assert!(
            store
                .recoverable(10)
                .await
                .expect("empty recovery")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn cancellation_corruption_bounds_and_read_only_mode_fail_closed() {
        let store = store(EventStoreMode::ReadWrite).await;
        let instance_id = instance(5);
        let prepared = store
            .prepare(prepare(instance_id, 5, 5, 500))
            .await
            .expect("prepare")
            .record()
            .clone();
        let cancelled = store
            .transition(JournalTransition::cancelled(
                instance_id,
                prepared.revision(),
                501,
            ))
            .await
            .expect("cancel");
        assert_eq!(cancelled.state().stage(), JournalStage::Recoverable);
        assert_eq!(
            store.recoverable(0).await,
            Err(Error::InvalidJournalQueryLimit)
        );
        assert_eq!(
            store.recoverable(RECOVERABLE_QUERY_LIMIT_MAX + 1).await,
            Err(Error::InvalidJournalQueryLimit)
        );

        sqlx::query(
            "UPDATE radroots_runtime_journal_operations
             SET recovery_record = X'0100FF' WHERE instance_id = ?",
        )
        .bind(instance_id.as_bytes().as_slice())
        .execute(store.pool())
        .await
        .expect("forge corrupt recovery");
        assert_eq!(
            store.operation(instance_id).await,
            Err(Error::CorruptJournalRecord)
        );

        let read_only = SqliteStorage::new(
            store.pool().clone(),
            SourceGeneration::new([21; 32]).expect("generation"),
            EventStoreMode::ReadOnly,
        );
        assert_eq!(
            read_only.prepare(prepare(instance(6), 6, 6, 600)).await,
            Err(Error::BackendUnavailable)
        );
    }
}

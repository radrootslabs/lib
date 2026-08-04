use crate::SqliteStorage;
use radroots_event_codec::Codec;
use radroots_storage::{
    Error, Outbox,
    outbox::{
        BoxFuture, ClaimOutboxItems, ClaimedOutboxItem, DeliveryAttempt, DeliveryAttemptEvidence,
        DeliveryOutcome, DeliveryOutcomeKind, DeliveryPayload, DeliveryPlanDigest, DeliveryRequest,
        EnqueueDisposition, EnqueueOutboxItem, EnqueueReceipt, LeaseId, LeaseOwner, OutboxItemId,
        OutboxLease, OutboxRecord, OutboxRevision, OutboxStage, OutboxStatus, Retryability,
        SatisfactionClass, SatisfactionPolicy, SatisfactionResult, TARGET_SET_MAX_ITEMS, Target,
        TargetDeliveryEvidence, TargetFingerprint, TargetLabel, TargetPolicy, TargetScope,
        TargetSet, TransportId,
    },
};
use sqlx::{Row, Sqlite, SqliteConnection};

#[cfg_attr(coverage_nightly, coverage(off))]
impl Outbox for SqliteStorage {
    fn enqueue(&self, item: EnqueueOutboxItem) -> BoxFuture<'_, Result<EnqueueReceipt, Error>> {
        Box::pin(async move {
            self.require_outbox_writer()?;
            let mut transaction = self
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            let receipt = enqueue_transaction(&mut transaction, item).await?;
            transaction.commit().await.map_err(map_backend)?;
            Ok(receipt)
        })
    }

    fn item(&self, item_id: OutboxItemId) -> BoxFuture<'_, Result<Option<OutboxRecord>, Error>> {
        Box::pin(async move {
            let mut connection = self.pool().acquire().await.map_err(map_backend)?;
            load_record(&mut connection, item_id).await
        })
    }

    fn claim(
        &self,
        request: ClaimOutboxItems,
    ) -> BoxFuture<'_, Result<Vec<ClaimedOutboxItem>, Error>> {
        Box::pin(async move {
            self.require_outbox_writer()?;
            let mut transaction = self
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            let rows = sqlx::query(
                "SELECT item_id FROM radroots_runtime_outbox_items
                 WHERE stage IN ('pending', 'leased', 'retryable')
                   AND (retry_not_before_unix_ms IS NULL OR retry_not_before_unix_ms <= ?)
                   AND (stage <> 'leased' OR lease_expires_at_unix_ms <= ?)
                 ORDER BY created_at_unix_ms, item_id LIMIT ?",
            )
            .bind(i64_from_u64(request.now_unix_ms())?)
            .bind(i64_from_u64(request.now_unix_ms())?)
            .bind(i64::from(request.limit()))
            .fetch_all(&mut *transaction)
            .await
            .map_err(map_backend)?;
            let item_ids = rows
                .iter()
                .map(|row| {
                    OutboxItemId::new(array(
                        row.try_get::<Vec<u8>, _>("item_id").map_err(map_corrupt)?,
                    )?)
                    .map_err(|_| Error::CorruptOutboxRecord)
                })
                .collect::<Result<Vec<_>, _>>()?;

            let mut claimed = Vec::with_capacity(item_ids.len());
            for item_id in item_ids {
                let mut record = load_record(&mut transaction, item_id)
                    .await?
                    .ok_or(Error::CorruptOutboxRecord)?;
                let prior_revision = record.revision();
                let lease = OutboxLease::new(
                    request.lease_id_for(item_id),
                    request.owner().clone(),
                    request.now_unix_ms(),
                    request.lease_expires_at_unix_ms(),
                )?;
                record.claim(lease.clone())?;
                update_record(&mut transaction, &record, prior_revision).await?;
                claimed.push(ClaimedOutboxItem::new(record, lease));
            }
            transaction.commit().await.map_err(map_backend)?;
            Ok(claimed)
        })
    }

    fn record_attempt(
        &self,
        evidence: DeliveryAttemptEvidence,
    ) -> BoxFuture<'_, Result<OutboxRecord, Error>> {
        Box::pin(async move {
            self.require_outbox_writer()?;
            let mut transaction = self
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            let record = record_attempt_transaction(&mut transaction, evidence).await?;
            transaction.commit().await.map_err(map_backend)?;
            Ok(record)
        })
    }

    fn release(
        &self,
        item_id: OutboxItemId,
        lease_id: LeaseId,
        expected_revision: OutboxRevision,
        released_at_unix_ms: u64,
        retry_not_before_unix_ms: Option<u64>,
    ) -> BoxFuture<'_, Result<OutboxRecord, Error>> {
        Box::pin(async move {
            self.require_outbox_writer()?;
            let mut transaction = self
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            let mut record = load_record(&mut transaction, item_id)
                .await?
                .ok_or(Error::OutboxItemNotFound)?;
            let prior_revision = record.revision();
            record.release(
                lease_id,
                expected_revision,
                released_at_unix_ms,
                retry_not_before_unix_ms,
            )?;
            update_record(&mut transaction, &record, prior_revision).await?;
            transaction.commit().await.map_err(map_backend)?;
            Ok(record)
        })
    }

    fn status(&self) -> BoxFuture<'_, Result<OutboxStatus, Error>> {
        Box::pin(async move {
            let row = sqlx::query(
                "SELECT
                   COALESCE(SUM(CASE WHEN stage = 'pending' THEN 1 ELSE 0 END), 0) AS pending,
                   COALESCE(SUM(CASE WHEN stage = 'leased' THEN 1 ELSE 0 END), 0) AS leased,
                   COALESCE(SUM(CASE WHEN stage = 'retryable' THEN 1 ELSE 0 END), 0) AS retryable,
                   COALESCE(SUM(CASE WHEN stage = 'satisfied' THEN 1 ELSE 0 END), 0) AS satisfied,
                   COALESCE(SUM(CASE WHEN stage = 'exhausted' THEN 1 ELSE 0 END), 0) AS exhausted
                 FROM radroots_runtime_outbox_items",
            )
            .fetch_one(self.pool())
            .await
            .map_err(map_backend)?;
            Ok(OutboxStatus {
                pending: count(&row, "pending")?,
                leased: count(&row, "leased")?,
                retryable: count(&row, "retryable")?,
                satisfied: count(&row, "satisfied")?,
                exhausted: count(&row, "exhausted")?,
            })
        })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) async fn enqueue_transaction(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    item: EnqueueOutboxItem,
) -> Result<EnqueueReceipt, Error> {
    if let Some(record) = load_record(transaction, item.item_id()).await? {
        if record.operation_instance_id() != item.operation_instance_id()
            || record.plan_digest() != item.plan_digest()
            || record.request() != item.request()
            || record.created_at_unix_ms() != item.created_at_unix_ms()
        {
            return Err(Error::OutboxPlanConflict);
        }
        return Ok(EnqueueReceipt::new(EnqueueDisposition::Replay, record));
    }
    if sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM radroots_runtime_outbox_items WHERE operation_instance_id = ?",
    )
    .bind(item.operation_instance_id().as_bytes().as_slice())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_backend)?
    .is_some()
    {
        return Err(Error::OutboxPlanConflict);
    }
    if sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM radroots_runtime_journal_operations WHERE instance_id = ?",
    )
    .bind(item.operation_instance_id().as_bytes().as_slice())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_backend)?
    .is_none()
    {
        return Err(Error::OperationNotFound);
    }

    let record = item.into_record();
    insert_record(transaction, &record).await?;
    Ok(EnqueueReceipt::new(EnqueueDisposition::Created, record))
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) async fn record_attempt_transaction(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    evidence: DeliveryAttemptEvidence,
) -> Result<OutboxRecord, Error> {
    let mut record = load_record(transaction, evidence.item_id())
        .await?
        .ok_or(Error::OutboxItemNotFound)?;
    let prior_revision = record.revision();
    let receipt = evidence.receipt().clone();
    let attempt = evidence.attempt();
    let recorded_at = evidence.recorded_at_unix_ms();
    record.record_attempt(evidence)?;
    update_record(transaction, &record, prior_revision).await?;
    for target_receipt in receipt.target_receipts() {
        let outcome = encode_outcome(target_receipt.outcome())?;
        sqlx::query(
            "INSERT INTO radroots_runtime_delivery_evidence (
               item_id, target_fingerprint, attempt, attempted, outcome,
               retryability, recorded_at_unix_ms
             ) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(record.item_id().as_bytes().as_slice())
        .bind(target_receipt.target().fingerprint().as_str().as_bytes())
        .bind(i64::from(attempt.get()))
        .bind(i64::from(target_receipt.was_attempted()))
        .bind(outcome)
        .bind(retryability_name(target_receipt.outcome().retryability()))
        .bind(i64_from_u64(recorded_at)?)
        .execute(&mut **transaction)
        .await
        .map_err(map_backend)?;
    }
    Ok(record)
}

impl SqliteStorage {
    fn require_outbox_writer(&self) -> Result<(), Error> {
        if self.event_mode() == radroots_storage::status::EventStoreMode::ReadOnly {
            return Err(Error::BackendUnavailable);
        }
        Ok(())
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn insert_record(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    record: &OutboxRecord,
) -> Result<(), Error> {
    let request = encode_request(record.request())?;
    sqlx::query(
        "INSERT INTO radroots_runtime_outbox_items (
           item_id, operation_instance_id, plan_digest, delivery_request, revision, stage,
           satisfaction, created_at_unix_ms, updated_at_unix_ms
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(record.item_id().as_bytes().as_slice())
    .bind(record.operation_instance_id().as_bytes().as_slice())
    .bind(record.plan_digest().as_bytes().as_slice())
    .bind(request)
    .bind(i64_from_u64(record.revision().get())?)
    .bind(stage_name(record.stage()))
    .bind(satisfaction_name(record.satisfaction()))
    .bind(i64_from_u64(record.created_at_unix_ms())?)
    .bind(i64_from_u64(record.updated_at_unix_ms())?)
    .execute(&mut **transaction)
    .await
    .map_err(map_backend)?;

    for (ordinal, target) in record.request().target_set().targets().iter().enumerate() {
        sqlx::query(
            "INSERT INTO radroots_runtime_outbox_targets (
               item_id, target_fingerprint, target_request, ordinal
             ) VALUES (?, ?, ?, ?)",
        )
        .bind(record.item_id().as_bytes().as_slice())
        .bind(target.fingerprint().as_str().as_bytes())
        .bind(encode_target(target)?)
        .bind(i64::try_from(ordinal).map_err(|_| Error::CorruptOutboxRecord)?)
        .execute(&mut **transaction)
        .await
        .map_err(map_backend)?;
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn update_record(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    record: &OutboxRecord,
    prior_revision: OutboxRevision,
) -> Result<(), Error> {
    let lease = record.lease();
    let result = sqlx::query(
        "UPDATE radroots_runtime_outbox_items SET
           revision = ?, stage = ?, lease_id = ?, lease_owner = ?,
           lease_acquired_at_unix_ms = ?, lease_expires_at_unix_ms = ?, last_attempt = ?,
           satisfaction = ?, retry_not_before_unix_ms = ?, updated_at_unix_ms = ?
         WHERE item_id = ? AND revision = ?",
    )
    .bind(i64_from_u64(record.revision().get())?)
    .bind(stage_name(record.stage()))
    .bind(lease.map(|lease| lease.id().as_bytes().to_vec()))
    .bind(lease.map(|lease| lease.owner().as_str()))
    .bind(
        lease
            .map(|lease| i64_from_u64(lease.acquired_at_unix_ms()))
            .transpose()?,
    )
    .bind(
        lease
            .map(|lease| i64_from_u64(lease.expires_at_unix_ms()))
            .transpose()?,
    )
    .bind(
        record
            .last_attempt()
            .map(|attempt| i64::from(attempt.get())),
    )
    .bind(satisfaction_name(record.satisfaction()))
    .bind(
        record
            .retry_not_before_unix_ms()
            .map(i64_from_u64)
            .transpose()?,
    )
    .bind(i64_from_u64(record.updated_at_unix_ms())?)
    .bind(record.item_id().as_bytes().as_slice())
    .bind(i64_from_u64(prior_revision.get())?)
    .execute(&mut **transaction)
    .await
    .map_err(map_backend)?;
    if result.rows_affected() != 1 {
        return Err(Error::OutboxRevisionConflict);
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn load_record(
    connection: &mut SqliteConnection,
    item_id: OutboxItemId,
) -> Result<Option<OutboxRecord>, Error> {
    let Some(row) = sqlx::query("SELECT * FROM radroots_runtime_outbox_items WHERE item_id = ?")
        .bind(item_id.as_bytes().as_slice())
        .fetch_optional(&mut *connection)
        .await
        .map_err(map_backend)?
    else {
        return Ok(None);
    };
    let request = decode_request(
        row.try_get::<Vec<u8>, _>("delivery_request")
            .map_err(map_corrupt)?
            .as_slice(),
    )?;
    validate_targets(connection, item_id, &request).await?;
    let evidence = load_evidence(connection, item_id).await?;
    let operation_instance_id = radroots_storage::journal::OperationInstanceId::new(array(
        row.try_get::<Vec<u8>, _>("operation_instance_id")
            .map_err(map_corrupt)?,
    )?)
    .map_err(|_| Error::CorruptOutboxRecord)?;
    let enqueue = EnqueueOutboxItem::new(
        item_id,
        operation_instance_id,
        DeliveryPlanDigest::new(array(
            row.try_get::<Vec<u8>, _>("plan_digest")
                .map_err(map_corrupt)?,
        )?),
        request,
        u64_from_i64(row.try_get("created_at_unix_ms").map_err(map_corrupt)?)?,
    )
    .map_err(|_| Error::CorruptOutboxRecord)?;
    let lease = decode_lease(&row)?;
    let last_attempt = row
        .try_get::<Option<i64>, _>("last_attempt")
        .map_err(map_corrupt)?
        .map(|value| {
            DeliveryAttempt::new(u32::try_from(value).map_err(|_| Error::CorruptOutboxRecord)?)
                .map_err(|_| Error::CorruptOutboxRecord)
        })
        .transpose()?;
    OutboxRecord::from_durable_parts(
        enqueue,
        OutboxRevision::new(u64_from_i64(row.try_get("revision").map_err(map_corrupt)?)?)
            .map_err(|_| Error::CorruptOutboxRecord)?,
        stage(
            row.try_get::<String, _>("stage")
                .map_err(map_corrupt)?
                .as_str(),
        )?,
        lease,
        last_attempt,
        evidence,
        satisfaction(
            row.try_get::<String, _>("satisfaction")
                .map_err(map_corrupt)?
                .as_str(),
        )?,
        row.try_get::<Option<i64>, _>("retry_not_before_unix_ms")
            .map_err(map_corrupt)?
            .map(u64_from_i64)
            .transpose()?,
        u64_from_i64(row.try_get("updated_at_unix_ms").map_err(map_corrupt)?)?,
    )
    .map(Some)
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn validate_targets(
    connection: &mut SqliteConnection,
    item_id: OutboxItemId,
    request: &DeliveryRequest,
) -> Result<(), Error> {
    let rows = sqlx::query(
        "SELECT target_fingerprint, target_request, ordinal
         FROM radroots_runtime_outbox_targets WHERE item_id = ? ORDER BY ordinal",
    )
    .bind(item_id.as_bytes().as_slice())
    .fetch_all(&mut *connection)
    .await
    .map_err(map_backend)?;
    if rows.len() != request.target_set().len() {
        return Err(Error::CorruptOutboxRecord);
    }
    for (ordinal, (row, expected)) in rows.iter().zip(request.target_set().targets()).enumerate() {
        let stored_ordinal = row.try_get::<i64, _>("ordinal").map_err(map_corrupt)?;
        let fingerprint = String::from_utf8(
            row.try_get::<Vec<u8>, _>("target_fingerprint")
                .map_err(map_corrupt)?,
        )
        .map_err(|_| Error::CorruptOutboxRecord)?;
        let target = decode_target(
            row.try_get::<Vec<u8>, _>("target_request")
                .map_err(map_corrupt)?
                .as_slice(),
        )?;
        if stored_ordinal != i64::try_from(ordinal).map_err(|_| Error::CorruptOutboxRecord)?
            || fingerprint != expected.fingerprint().as_str()
            || target != *expected
        {
            return Err(Error::CorruptOutboxRecord);
        }
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn load_evidence(
    connection: &mut SqliteConnection,
    item_id: OutboxItemId,
) -> Result<Vec<TargetDeliveryEvidence>, Error> {
    sqlx::query(
        "SELECT evidence.target_fingerprint, evidence.attempt, evidence.attempted,
                evidence.outcome, evidence.retryability, evidence.recorded_at_unix_ms
         FROM radroots_runtime_delivery_evidence AS evidence
         JOIN radroots_runtime_outbox_targets AS target
           ON target.item_id = evidence.item_id
          AND target.target_fingerprint = evidence.target_fingerprint
         WHERE evidence.item_id = ?
         ORDER BY evidence.attempt, target.ordinal",
    )
    .bind(item_id.as_bytes().as_slice())
    .fetch_all(&mut *connection)
    .await
    .map_err(map_backend)?
    .iter()
    .map(|row| {
        let target = TargetFingerprint::parse(
            String::from_utf8(
                row.try_get::<Vec<u8>, _>("target_fingerprint")
                    .map_err(map_corrupt)?,
            )
            .map_err(|_| Error::CorruptOutboxRecord)?,
        )
        .map_err(|_| Error::CorruptOutboxRecord)?;
        let outcome = decode_outcome(
            row.try_get::<Vec<u8>, _>("outcome")
                .map_err(map_corrupt)?
                .as_slice(),
        )?;
        let retryability = row
            .try_get::<String, _>("retryability")
            .map_err(map_corrupt)?;
        if retryability != retryability_name(outcome.retryability()) {
            return Err(Error::CorruptOutboxRecord);
        }
        TargetDeliveryEvidence::new(
            target,
            DeliveryAttempt::new(
                u32::try_from(row.try_get::<i64, _>("attempt").map_err(map_corrupt)?)
                    .map_err(|_| Error::CorruptOutboxRecord)?,
            )
            .map_err(|_| Error::CorruptOutboxRecord)?,
            match row.try_get::<i64, _>("attempted").map_err(map_corrupt)? {
                0 => false,
                1 => true,
                _ => return Err(Error::CorruptOutboxRecord),
            },
            outcome,
            u64_from_i64(row.try_get("recorded_at_unix_ms").map_err(map_corrupt)?)?,
        )
        .map_err(|_| Error::CorruptOutboxRecord)
    })
    .collect()
}

fn decode_lease(row: &sqlx::sqlite::SqliteRow) -> Result<Option<OutboxLease>, Error> {
    let id = row
        .try_get::<Option<Vec<u8>>, _>("lease_id")
        .map_err(map_corrupt)?;
    let owner = row
        .try_get::<Option<String>, _>("lease_owner")
        .map_err(map_corrupt)?;
    let acquired = row
        .try_get::<Option<i64>, _>("lease_acquired_at_unix_ms")
        .map_err(map_corrupt)?;
    let expires = row
        .try_get::<Option<i64>, _>("lease_expires_at_unix_ms")
        .map_err(map_corrupt)?;
    match (id, owner, acquired, expires) {
        (None, None, None, None) => Ok(None),
        (Some(id), Some(owner), Some(acquired), Some(expires)) => OutboxLease::new(
            LeaseId::new(array(id)?).map_err(|_| Error::CorruptOutboxRecord)?,
            LeaseOwner::parse(owner).map_err(|_| Error::CorruptOutboxRecord)?,
            u64_from_i64(acquired)?,
            u64_from_i64(expires)?,
        )
        .map(Some)
        .map_err(|_| Error::CorruptOutboxRecord),
        _ => Err(Error::CorruptOutboxRecord),
    }
}

pub(crate) fn encode_record_snapshot(record: &OutboxRecord) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::with_capacity(256);
    bytes.push(1);
    bytes.extend_from_slice(record.item_id().as_bytes());
    bytes.extend_from_slice(record.operation_instance_id().as_bytes());
    bytes.extend_from_slice(record.plan_digest().as_bytes());
    put_blob(&mut bytes, encode_request(record.request())?.as_slice())?;
    bytes.extend_from_slice(&record.revision().get().to_be_bytes());
    bytes.push(match record.stage() {
        OutboxStage::Pending => 0,
        OutboxStage::Leased => 1,
        OutboxStage::Retryable => 2,
        OutboxStage::Satisfied => 3,
        OutboxStage::Exhausted => 4,
    });
    match record.lease() {
        Some(lease) => {
            bytes.push(1);
            bytes.extend_from_slice(lease.id().as_bytes());
            put_str(&mut bytes, lease.owner().as_str())?;
            bytes.extend_from_slice(&lease.acquired_at_unix_ms().to_be_bytes());
            bytes.extend_from_slice(&lease.expires_at_unix_ms().to_be_bytes());
        }
        None => bytes.push(0),
    }
    match record.last_attempt() {
        Some(attempt) => {
            bytes.push(1);
            bytes.extend_from_slice(&attempt.get().to_be_bytes());
        }
        None => bytes.push(0),
    }
    let evidence_count =
        u32::try_from(record.evidence().len()).map_err(|_| Error::CorruptOutboxRecord)?;
    bytes.extend_from_slice(&evidence_count.to_be_bytes());
    for evidence in record.evidence() {
        put_str(&mut bytes, evidence.target().as_str())?;
        bytes.extend_from_slice(&evidence.attempt().get().to_be_bytes());
        bytes.push(u8::from(evidence.was_attempted()));
        put_blob(&mut bytes, encode_outcome(evidence.outcome())?.as_slice())?;
        bytes.extend_from_slice(&evidence.recorded_at_unix_ms().to_be_bytes());
    }
    bytes.push(match record.satisfaction() {
        SatisfactionResult::Pending => 0,
        SatisfactionResult::Satisfied => 1,
        SatisfactionResult::Exhausted => 2,
    });
    match record.retry_not_before_unix_ms() {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        None => bytes.push(0),
    }
    bytes.extend_from_slice(&record.created_at_unix_ms().to_be_bytes());
    bytes.extend_from_slice(&record.updated_at_unix_ms().to_be_bytes());
    Ok(bytes)
}

pub(crate) fn decode_record_snapshot(bytes: &[u8]) -> Result<OutboxRecord, Error> {
    let mut cursor = Cursor::new(bytes);
    if cursor.byte()? != 1 {
        return Err(Error::CorruptOutboxRecord);
    }
    let item_id = OutboxItemId::new(cursor.array()?).map_err(|_| Error::CorruptOutboxRecord)?;
    let operation_instance_id =
        radroots_storage::journal::OperationInstanceId::new(cursor.array()?)
            .map_err(|_| Error::CorruptOutboxRecord)?;
    let plan_digest = DeliveryPlanDigest::new(cursor.array()?);
    let request = decode_request(cursor.blob()?)?;
    let revision = OutboxRevision::new(cursor.u64()?).map_err(|_| Error::CorruptOutboxRecord)?;
    let stage = match cursor.byte()? {
        0 => OutboxStage::Pending,
        1 => OutboxStage::Leased,
        2 => OutboxStage::Retryable,
        3 => OutboxStage::Satisfied,
        4 => OutboxStage::Exhausted,
        _ => return Err(Error::CorruptOutboxRecord),
    };
    let lease = match cursor.byte()? {
        0 => None,
        1 => Some(
            OutboxLease::new(
                LeaseId::new(cursor.array()?).map_err(|_| Error::CorruptOutboxRecord)?,
                LeaseOwner::parse(cursor.string()?).map_err(|_| Error::CorruptOutboxRecord)?,
                cursor.u64()?,
                cursor.u64()?,
            )
            .map_err(|_| Error::CorruptOutboxRecord)?,
        ),
        _ => return Err(Error::CorruptOutboxRecord),
    };
    let last_attempt = match cursor.byte()? {
        0 => None,
        1 => Some(DeliveryAttempt::new(cursor.u32()?).map_err(|_| Error::CorruptOutboxRecord)?),
        _ => return Err(Error::CorruptOutboxRecord),
    };
    let evidence_count = usize::try_from(cursor.u32()?).map_err(|_| Error::CorruptOutboxRecord)?;
    if evidence_count > bytes.len() / 20 {
        return Err(Error::CorruptOutboxRecord);
    }
    let mut evidence = Vec::with_capacity(evidence_count);
    for _ in 0..evidence_count {
        let target =
            TargetFingerprint::parse(cursor.string()?).map_err(|_| Error::CorruptOutboxRecord)?;
        let attempt =
            DeliveryAttempt::new(cursor.u32()?).map_err(|_| Error::CorruptOutboxRecord)?;
        let attempted = match cursor.byte()? {
            0 => false,
            1 => true,
            _ => return Err(Error::CorruptOutboxRecord),
        };
        let outcome = decode_outcome(cursor.blob()?)?;
        let recorded_at_unix_ms = cursor.u64()?;
        evidence.push(
            TargetDeliveryEvidence::new(target, attempt, attempted, outcome, recorded_at_unix_ms)
                .map_err(|_| Error::CorruptOutboxRecord)?,
        );
    }
    let satisfaction = match cursor.byte()? {
        0 => SatisfactionResult::Pending,
        1 => SatisfactionResult::Satisfied,
        2 => SatisfactionResult::Exhausted,
        _ => return Err(Error::CorruptOutboxRecord),
    };
    let retry_not_before_unix_ms = match cursor.byte()? {
        0 => None,
        1 => Some(cursor.u64()?),
        _ => return Err(Error::CorruptOutboxRecord),
    };
    let created_at_unix_ms = cursor.u64()?;
    let updated_at_unix_ms = cursor.u64()?;
    cursor.finish()?;
    let enqueue = EnqueueOutboxItem::new(
        item_id,
        operation_instance_id,
        plan_digest,
        request,
        created_at_unix_ms,
    )
    .map_err(|_| Error::CorruptOutboxRecord)?;
    OutboxRecord::from_durable_parts(
        enqueue,
        revision,
        stage,
        lease,
        last_attempt,
        evidence,
        satisfaction,
        retry_not_before_unix_ms,
        updated_at_unix_ms,
    )
    .map_err(|_| Error::CorruptOutboxRecord)
}

fn encode_request(value: &DeliveryRequest) -> Result<Vec<u8>, Error> {
    let mut bytes = vec![1];
    put_str(&mut bytes, value.request_id().as_str())?;
    put_blob(&mut bytes, value.payload().event().raw_json().as_bytes())?;
    put_u16(
        &mut bytes,
        u16::try_from(value.target_set().len()).map_err(|_| Error::CorruptOutboxRecord)?,
    );
    for target in value.target_set().targets() {
        put_blob(&mut bytes, encode_target(target)?.as_slice())?;
    }
    bytes.push(match value.satisfaction().class() {
        SatisfactionClass::Accepted => 0,
        SatisfactionClass::Delivered => 1,
    });
    let policy = value.satisfaction().targets();
    if policy.is_any() {
        bytes.push(0);
    } else if policy.is_all() {
        bytes.push(1);
    } else if let Some(threshold) = policy.quorum_threshold() {
        bytes.push(2);
        put_u16(&mut bytes, threshold);
    } else if let Some(required) = policy.required_targets() {
        bytes.push(3);
        put_u16(
            &mut bytes,
            u16::try_from(required.len()).map_err(|_| Error::CorruptOutboxRecord)?,
        );
        for target in required {
            put_str(&mut bytes, target.as_str())?;
        }
    } else {
        return Err(Error::CorruptOutboxRecord);
    }
    bytes.extend_from_slice(&value.deadline_unix_ms().to_be_bytes());
    Ok(bytes)
}

fn decode_request(bytes: &[u8]) -> Result<DeliveryRequest, Error> {
    let mut cursor = Cursor::new(bytes);
    if cursor.byte()? != 1 {
        return Err(Error::CorruptOutboxRecord);
    }
    let request_id = cursor.string()?.to_owned();
    let raw_event = cursor.string_blob()?;
    let event = Codec::decode_signed_event(raw_event).map_err(|_| Error::CorruptOutboxRecord)?;
    let target_count = usize::from(cursor.u16()?);
    if target_count == 0 || target_count > TARGET_SET_MAX_ITEMS {
        return Err(Error::CorruptOutboxRecord);
    }
    let mut targets = Vec::with_capacity(target_count);
    for _ in 0..target_count {
        targets.push(decode_target(cursor.blob()?)?);
    }
    let class = match cursor.byte()? {
        0 => SatisfactionClass::Accepted,
        1 => SatisfactionClass::Delivered,
        _ => return Err(Error::CorruptOutboxRecord),
    };
    let target_policy = match cursor.byte()? {
        0 => TargetPolicy::any(),
        1 => TargetPolicy::all(),
        2 => TargetPolicy::quorum(cursor.u16()?).map_err(|_| Error::CorruptOutboxRecord)?,
        3 => {
            let count = usize::from(cursor.u16()?);
            let mut required = Vec::with_capacity(count);
            for _ in 0..count {
                required.push(
                    TargetFingerprint::parse(cursor.string()?)
                        .map_err(|_| Error::CorruptOutboxRecord)?,
                );
            }
            TargetPolicy::required(required).map_err(|_| Error::CorruptOutboxRecord)?
        }
        _ => return Err(Error::CorruptOutboxRecord),
    };
    let deadline = cursor.u64()?;
    cursor.finish()?;
    DeliveryRequest::new(
        request_id,
        DeliveryPayload::new(event),
        TargetSet::new(targets).map_err(|_| Error::CorruptOutboxRecord)?,
        SatisfactionPolicy::new(class, target_policy),
        deadline,
    )
    .map_err(|_| Error::CorruptOutboxRecord)
}

fn encode_target(value: &Target) -> Result<Vec<u8>, Error> {
    let mut bytes = vec![1];
    put_str(&mut bytes, value.kind().as_str())?;
    put_str(&mut bytes, value.uri().as_str())?;
    put_optional_str(&mut bytes, value.scope().map(TargetScope::as_str))?;
    put_optional_str(&mut bytes, value.label().map(TargetLabel::as_str))?;
    Ok(bytes)
}

fn decode_target(bytes: &[u8]) -> Result<Target, Error> {
    let mut cursor = Cursor::new(bytes);
    if cursor.byte()? != 1 {
        return Err(Error::CorruptOutboxRecord);
    }
    let kind = TransportId::parse(cursor.string()?).map_err(|_| Error::CorruptOutboxRecord)?;
    let uri = cursor.string()?.to_owned();
    let scope = cursor
        .optional_string()?
        .map(TargetScope::parse)
        .transpose()
        .map_err(|_| Error::CorruptOutboxRecord)?;
    let label = cursor
        .optional_string()?
        .map(TargetLabel::parse)
        .transpose()
        .map_err(|_| Error::CorruptOutboxRecord)?;
    cursor.finish()?;
    Target::new_with_metadata(kind, uri, scope, label).map_err(|_| Error::CorruptOutboxRecord)
}

fn encode_outcome(value: &DeliveryOutcome) -> Result<Vec<u8>, Error> {
    let mut bytes = vec![1];
    bytes.push(match value.kind() {
        DeliveryOutcomeKind::Accepted => 0,
        DeliveryOutcomeKind::Delivered => 1,
        DeliveryOutcomeKind::Rejected => 2,
        DeliveryOutcomeKind::Unavailable => 3,
        DeliveryOutcomeKind::Failed => 4,
    });
    bytes.push(retryability_byte(value.retryability()));
    match (value.code(), value.message()) {
        (None, None) => bytes.push(0),
        (Some(code), Some(message)) => {
            bytes.push(1);
            put_str(&mut bytes, code)?;
            put_str(&mut bytes, message)?;
        }
        _ => return Err(Error::CorruptOutboxRecord),
    }
    Ok(bytes)
}

fn decode_outcome(bytes: &[u8]) -> Result<DeliveryOutcome, Error> {
    let mut cursor = Cursor::new(bytes);
    if cursor.byte()? != 1 {
        return Err(Error::CorruptOutboxRecord);
    }
    let kind = cursor.byte()?;
    let retryability = retryability(cursor.byte()?)?;
    let outcome = match kind {
        0 if retryability == Retryability::NotApplicable => DeliveryOutcome::accepted(),
        1 if retryability == Retryability::NotApplicable => DeliveryOutcome::delivered(),
        2 if retryability == Retryability::Terminal => DeliveryOutcome::rejected(),
        3 if retryability == Retryability::Retryable => DeliveryOutcome::unavailable(),
        4 => DeliveryOutcome::failed(retryability).map_err(|_| Error::CorruptOutboxRecord)?,
        _ => return Err(Error::CorruptOutboxRecord),
    };
    let outcome = match cursor.byte()? {
        0 => outcome,
        1 => outcome
            .with_detail(cursor.string()?, cursor.string()?)
            .map_err(|_| Error::CorruptOutboxRecord)?,
        _ => return Err(Error::CorruptOutboxRecord),
    };
    cursor.finish()?;
    Ok(outcome)
}

fn put_str(bytes: &mut Vec<u8>, value: &str) -> Result<(), Error> {
    let length = u16::try_from(value.len()).map_err(|_| Error::CorruptOutboxRecord)?;
    put_u16(bytes, length);
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn put_optional_str(bytes: &mut Vec<u8>, value: Option<&str>) -> Result<(), Error> {
    match value {
        Some(value) => {
            bytes.push(1);
            put_str(bytes, value)
        }
        None => {
            bytes.push(0);
            Ok(())
        }
    }
}

fn put_blob(bytes: &mut Vec<u8>, value: &[u8]) -> Result<(), Error> {
    let length = u32::try_from(value.len()).map_err(|_| Error::CorruptOutboxRecord)?;
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(value);
    Ok(())
}

fn put_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_be_bytes());
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
            .ok_or(Error::CorruptOutboxRecord)?;
        self.offset += 1;
        Ok(value)
    }

    fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    fn string(&mut self) -> Result<&'a str, Error> {
        let length = usize::from(self.u16()?);
        core::str::from_utf8(self.take(length)?).map_err(|_| Error::CorruptOutboxRecord)
    }

    fn optional_string(&mut self) -> Result<Option<&'a str>, Error> {
        match self.byte()? {
            0 => Ok(None),
            1 => self.string().map(Some),
            _ => Err(Error::CorruptOutboxRecord),
        }
    }

    fn blob(&mut self) -> Result<&'a [u8], Error> {
        let length = usize::try_from(self.u32()?).map_err(|_| Error::CorruptOutboxRecord)?;
        self.take(length)
    }

    fn string_blob(&mut self) -> Result<&'a str, Error> {
        core::str::from_utf8(self.blob()?).map_err(|_| Error::CorruptOutboxRecord)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        self.take(N)?
            .try_into()
            .map_err(|_| Error::CorruptOutboxRecord)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(Error::CorruptOutboxRecord)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(Error::CorruptOutboxRecord)?;
        self.offset = end;
        Ok(value)
    }

    fn finish(self) -> Result<(), Error> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(Error::CorruptOutboxRecord)
        }
    }
}

const fn stage_name(value: OutboxStage) -> &'static str {
    match value {
        OutboxStage::Pending => "pending",
        OutboxStage::Leased => "leased",
        OutboxStage::Retryable => "retryable",
        OutboxStage::Satisfied => "satisfied",
        OutboxStage::Exhausted => "exhausted",
    }
}

const fn stage(value: &str) -> Result<OutboxStage, Error> {
    match value.as_bytes() {
        b"pending" => Ok(OutboxStage::Pending),
        b"leased" => Ok(OutboxStage::Leased),
        b"retryable" => Ok(OutboxStage::Retryable),
        b"satisfied" => Ok(OutboxStage::Satisfied),
        b"exhausted" => Ok(OutboxStage::Exhausted),
        _ => Err(Error::CorruptOutboxRecord),
    }
}

const fn satisfaction_name(value: SatisfactionResult) -> &'static str {
    match value {
        SatisfactionResult::Pending => "pending",
        SatisfactionResult::Satisfied => "satisfied",
        SatisfactionResult::Exhausted => "exhausted",
    }
}

const fn satisfaction(value: &str) -> Result<SatisfactionResult, Error> {
    match value.as_bytes() {
        b"pending" => Ok(SatisfactionResult::Pending),
        b"satisfied" => Ok(SatisfactionResult::Satisfied),
        b"exhausted" => Ok(SatisfactionResult::Exhausted),
        _ => Err(Error::CorruptOutboxRecord),
    }
}

const fn retryability_name(value: Retryability) -> &'static str {
    match value {
        Retryability::Retryable => "retryable",
        Retryability::Terminal => "terminal",
        Retryability::NotApplicable => "not_applicable",
    }
}

const fn retryability_byte(value: Retryability) -> u8 {
    match value {
        Retryability::NotApplicable => 0,
        Retryability::Retryable => 1,
        Retryability::Terminal => 2,
    }
}

const fn retryability(value: u8) -> Result<Retryability, Error> {
    match value {
        0 => Ok(Retryability::NotApplicable),
        1 => Ok(Retryability::Retryable),
        2 => Ok(Retryability::Terminal),
        _ => Err(Error::CorruptOutboxRecord),
    }
}

fn count(row: &sqlx::sqlite::SqliteRow, column: &str) -> Result<u64, Error> {
    u64_from_i64(row.try_get::<i64, _>(column).map_err(map_corrupt)?)
}

fn array<const N: usize>(bytes: Vec<u8>) -> Result<[u8; N], Error> {
    bytes.try_into().map_err(|_| Error::CorruptOutboxRecord)
}

fn i64_from_u64(value: u64) -> Result<i64, Error> {
    i64::try_from(value).map_err(|_| Error::CorruptOutboxRecord)
}

fn u64_from_i64(value: i64) -> Result<u64, Error> {
    u64::try_from(value).map_err(|_| Error::CorruptOutboxRecord)
}

fn map_backend(_: sqlx::Error) -> Error {
    Error::BackendUnavailable
}

fn map_corrupt(_: sqlx::Error) -> Error {
    Error::CorruptOutboxRecord
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::migration::runtime::{MIGRATIONS, migration_sql};
    use radroots_event::{SignedEvent, wire::Nip01EventWire};
    use radroots_storage::{
        Journal, Outbox,
        event::SourceGeneration,
        journal::{
            IdempotencyDigest, IdempotencyKey, OperationId, OperationInstanceId, PrepareOperation,
        },
        outbox::{DeliveryReceipt, DeliveryTargetReceipt},
        status::EventStoreMode,
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
            SourceGeneration::new([31; 32]).expect("generation"),
            mode,
        )
    }

    fn instance(byte: u8) -> OperationInstanceId {
        OperationInstanceId::new([byte; 16]).expect("operation instance")
    }

    async fn prepare(store: &SqliteStorage, byte: u8) {
        Journal::prepare(
            store,
            PrepareOperation::new(
                instance(byte),
                OperationId::SyncPush,
                IdempotencyKey::parse(format!("outbox-operation-{byte:02x}"))
                    .expect("idempotency key"),
                IdempotencyDigest::new([byte; 32]),
                10,
            )
            .expect("prepare operation"),
        )
        .await
        .expect("prepared journal operation");
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

    fn request() -> DeliveryRequest {
        DeliveryRequest::new(
            "sqlite-outbox-request",
            DeliveryPayload::new(signed_event()),
            TargetSet::new(vec![
                Target::nostr_relay("wss://one.example").expect("first target"),
                Target::nostr_relay("wss://two.example").expect("second target"),
            ])
            .expect("target set"),
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
            10_000,
        )
        .expect("delivery request")
    }

    fn enqueue(item: u8, operation: u8, digest: u8) -> EnqueueOutboxItem {
        EnqueueOutboxItem::new(
            OutboxItemId::new([item; 16]).expect("item id"),
            instance(operation),
            DeliveryPlanDigest::new([digest; 32]),
            request(),
            20,
        )
        .expect("enqueue")
    }

    fn claim_request(now: u64, expires: u64, seed: u8) -> ClaimOutboxItems {
        ClaimOutboxItems::new(
            LeaseOwner::parse("sqlite-worker").expect("owner"),
            LeaseId::new([seed; 16]).expect("lease seed"),
            now,
            expires,
            10,
        )
        .expect("claim request")
    }

    fn receipt(request: &DeliveryRequest, outcomes: [DeliveryOutcome; 2]) -> DeliveryReceipt {
        DeliveryReceipt::for_request(
            request,
            request
                .target_set()
                .targets()
                .iter()
                .cloned()
                .zip(outcomes)
                .map(|(target, outcome)| DeliveryTargetReceipt::attempted(target, outcome))
                .collect(),
        )
        .expect("delivery receipt")
    }

    #[tokio::test]
    async fn enqueue_replays_exact_plans_and_rejects_identity_conflicts() {
        let store = store(EventStoreMode::ReadWrite).await;
        prepare(&store, 1).await;
        let item = enqueue(1, 1, 2);
        let created = store.enqueue(item.clone()).await.expect("created");
        assert_eq!(created.disposition(), EnqueueDisposition::Created);
        let replay = store.enqueue(item).await.expect("replay");
        assert_eq!(replay.disposition(), EnqueueDisposition::Replay);
        assert_eq!(replay.record(), created.record());
        assert_eq!(
            store.enqueue(enqueue(1, 1, 3)).await,
            Err(Error::OutboxPlanConflict)
        );
        assert_eq!(
            store.enqueue(enqueue(2, 1, 2)).await,
            Err(Error::OutboxPlanConflict)
        );
        assert_eq!(
            store
                .item(OutboxItemId::new([1; 16]).expect("item id"))
                .await
                .expect("lookup")
                .expect("record"),
            *created.record()
        );
    }

    #[tokio::test]
    async fn claims_expire_release_and_honor_retry_deferral() {
        let store = store(EventStoreMode::ReadWrite).await;
        prepare(&store, 2).await;
        store.enqueue(enqueue(2, 2, 2)).await.expect("enqueue");
        let first = store
            .claim(claim_request(100, 200, 3))
            .await
            .expect("claim")
            .pop()
            .expect("claimed");
        assert!(
            store
                .claim(claim_request(150, 250, 4))
                .await
                .expect("concurrent claim")
                .is_empty()
        );
        let reclaimed = store
            .claim(claim_request(200, 300, 5))
            .await
            .expect("expired claim")
            .pop()
            .expect("reclaimed");
        assert_ne!(first.lease().id(), reclaimed.lease().id());
        let released = store
            .release(
                reclaimed.record().item_id(),
                reclaimed.lease().id(),
                reclaimed.record().revision(),
                210,
                Some(250),
            )
            .await
            .expect("release");
        assert_eq!(released.stage(), OutboxStage::Pending);
        assert!(
            store
                .claim(claim_request(249, 300, 6))
                .await
                .expect("deferred claim")
                .is_empty()
        );
        assert_eq!(
            store
                .claim(claim_request(250, 350, 7))
                .await
                .expect("ready claim")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn partial_evidence_survives_recovery_and_advances_to_satisfaction() {
        let store = store(EventStoreMode::ReadWrite).await;
        prepare(&store, 3).await;
        store.enqueue(enqueue(3, 3, 3)).await.expect("enqueue");
        let first = store
            .claim(claim_request(100, 200, 4))
            .await
            .expect("claim")
            .pop()
            .expect("claimed");
        let partial = store
            .record_attempt(
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
                .expect("partial evidence"),
            )
            .await
            .expect("partial attempt");
        assert_eq!(partial.stage(), OutboxStage::Retryable);
        assert_eq!(partial.evidence().len(), 2);

        let recovered = SqliteStorage::new(
            store.pool().clone(),
            SourceGeneration::new([31; 32]).expect("generation"),
            EventStoreMode::ReadWrite,
        );
        let second = recovered
            .claim(claim_request(250, 350, 5))
            .await
            .expect("recovery claim")
            .pop()
            .expect("reclaimed");
        let satisfied = recovered
            .record_attempt(
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
                .expect("success evidence"),
            )
            .await
            .expect("successful attempt");
        assert_eq!(satisfied.stage(), OutboxStage::Satisfied);
        assert_eq!(satisfied.evidence().len(), 4);
        assert_eq!(recovered.status().await.expect("status").satisfied, 1);

        sqlx::query(
            "UPDATE radroots_runtime_delivery_evidence SET outcome = X'FF'
             WHERE item_id = ? AND attempt = 2",
        )
        .bind(satisfied.item_id().as_bytes().as_slice())
        .execute(recovered.pool())
        .await
        .expect("forge corrupt evidence");
        assert_eq!(
            recovered.item(satisfied.item_id()).await,
            Err(Error::CorruptOutboxRecord)
        );
        let read_only = SqliteStorage::new(
            recovered.pool().clone(),
            SourceGeneration::new([31; 32]).expect("generation"),
            EventStoreMode::ReadOnly,
        );
        assert_eq!(
            read_only.claim(claim_request(400, 500, 6)).await,
            Err(Error::BackendUnavailable)
        );
    }

    #[test]
    fn versioned_codecs_round_trip_every_policy_target_and_outcome_shape() {
        let targets = TargetSet::new(vec![
            Target::local_with_metadata(
                "local://queue",
                Some(TargetScope::parse("farm.one").expect("scope")),
                Some(TargetLabel::parse("Farm queue").expect("label")),
            )
            .expect("local target"),
            Target::nostr_relay("wss://relay.example").expect("relay target"),
        ])
        .expect("target set");
        let required = targets.targets()[0].fingerprint().clone();
        for policy in [
            TargetPolicy::any(),
            TargetPolicy::all(),
            TargetPolicy::quorum(1).expect("quorum"),
            TargetPolicy::required(vec![required]).expect("required"),
        ] {
            let request = DeliveryRequest::new(
                "codec-request",
                DeliveryPayload::new(signed_event()),
                targets.clone(),
                SatisfactionPolicy::new(SatisfactionClass::Delivered, policy),
                20_000,
            )
            .expect("request");
            assert_eq!(
                decode_request(&encode_request(&request).expect("encode request"))
                    .expect("decode request"),
                request
            );
        }

        let outcomes = [
            DeliveryOutcome::accepted(),
            DeliveryOutcome::delivered(),
            DeliveryOutcome::rejected(),
            DeliveryOutcome::unavailable(),
            DeliveryOutcome::failed(Retryability::Retryable)
                .expect("retryable failure")
                .with_detail("relay_timeout", "relay request timed out")
                .expect("failure detail"),
            DeliveryOutcome::failed(Retryability::Terminal).expect("terminal failure"),
        ];
        for outcome in outcomes {
            assert_eq!(
                decode_outcome(&encode_outcome(&outcome).expect("encode outcome"))
                    .expect("decode outcome"),
                outcome
            );
        }
        for target in targets.targets() {
            let encoded = encode_target(target).expect("encode target");
            assert_eq!(decode_target(&encoded).expect("decode target"), *target);
            assert_eq!(decode_target(&[0]), Err(Error::CorruptOutboxRecord));
            let mut trailing = encoded;
            trailing.push(0);
            assert_eq!(decode_target(&trailing), Err(Error::CorruptOutboxRecord));
        }

        let encoded = encode_request(&request()).expect("encode request");
        for end in 0..encoded.len() {
            let _ = decode_request(&encoded[..end]);
        }
        let mut invalid_version = encoded.clone();
        invalid_version[0] = 0;
        assert_eq!(
            decode_request(&invalid_version),
            Err(Error::CorruptOutboxRecord)
        );
        let request_id_length = usize::from(u16::from_be_bytes([encoded[1], encoded[2]]));
        let event_length_offset = 3 + request_id_length;
        let event_length = usize::try_from(u32::from_be_bytes(
            encoded[event_length_offset..event_length_offset + 4]
                .try_into()
                .expect("event length"),
        ))
        .expect("event length fits");
        let target_count_offset = event_length_offset + 4 + event_length;
        let mut no_targets = encoded.clone();
        no_targets[target_count_offset..target_count_offset + 2]
            .copy_from_slice(&0_u16.to_be_bytes());
        assert_eq!(decode_request(&no_targets), Err(Error::CorruptOutboxRecord));
        let mut too_many_targets = encoded;
        too_many_targets[target_count_offset..target_count_offset + 2]
            .copy_from_slice(&u16::MAX.to_be_bytes());
        assert_eq!(
            decode_request(&too_many_targets),
            Err(Error::CorruptOutboxRecord)
        );

        for kind in 0..=5 {
            for retryability in 0..=3 {
                for detail in 0..=2 {
                    let _ = decode_outcome(&[1, kind, retryability, detail]);
                }
            }
        }
        assert_eq!(decode_outcome(&[0]), Err(Error::CorruptOutboxRecord));
    }
}

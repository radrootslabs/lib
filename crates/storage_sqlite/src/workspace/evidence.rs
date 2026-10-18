//! Append-only protected obtained bytes and exact original observation data.
use super::{
    attempt,
    crypto::{IssuancePayload, PRIVATE_PAYLOAD_MAX_BYTES, WorkspaceCrypto},
};
use core::num::NonZeroU64;
use radroots_domain::{
    CommandDigest, EnvelopeEventId, OperationId, PublicKey, SigningAttemptId, TransactionId,
    WorkspaceId,
};
use radroots_storage::command::{EvidenceReceipt, OperationError, SealEvidence, SigningAttempt};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqliteConnection};
use zeroize::Zeroizing;

pub(super) const TABLE: &str = "CREATE TABLE seal_evidence (workspace BLOB NOT NULL, operation_id BLOB NOT NULL, recipient BLOB NOT NULL, attempt_id BLOB NOT NULL CHECK(length(attempt_id)=32), event_id BLOB NOT NULL CHECK(length(event_id)=32), evidence_digest BLOB NOT NULL CHECK(length(evidence_digest)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), part_count INTEGER NOT NULL CHECK(part_count BETWEEN 1 AND 4), byte_count INTEGER NOT NULL CHECK(byte_count BETWEEN 1 AND 65536), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(workspace,operation_id,recipient), FOREIGN KEY(workspace,operation_id,recipient) REFERENCES signing_attempts(workspace,operation_id,recipient)) STRICT, WITHOUT ROWID";
pub(super) const PARTS: &str = "CREATE TABLE seal_evidence_parts (workspace BLOB NOT NULL, operation_id BLOB NOT NULL, recipient BLOB NOT NULL, part INTEGER NOT NULL CHECK(part BETWEEN 1 AND 4), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(workspace,operation_id,recipient,part), FOREIGN KEY(workspace,operation_id,recipient) REFERENCES seal_evidence(workspace,operation_id,recipient)) STRICT, WITHOUT ROWID";
pub(super) const TX_INDEX: &str = "CREATE UNIQUE INDEX seal_evidence_transaction_identity ON seal_evidence(workspace,transaction_id)";
const MAX_PROTECTED_TOTAL: usize = 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u8,
    workspace: WorkspaceId,
    operation_id: OperationId,
    command_digest: CommandDigest,
    plan_digest: [u8; 32],
    recipient: PublicKey,
    attempt_id: SigningAttemptId,
    claim_transaction: TransactionId,
    intent_digest: [u8; 32],
    event_id: EnvelopeEventId,
    evidence_digest: [u8; 32],
    transaction_id: TransactionId,
    observed_at: Option<u64>,
    byte_count: usize,
    part_count: usize,
}
pub(super) struct ProtectedEvidence {
    pub manifest: Vec<u8>,
    pub parts: Vec<Vec<u8>>,
    pub receipt: EvidenceReceipt,
    pub bytes: usize,
}
pub(super) async fn protect(
    attempt: &SigningAttempt,
    evidence: &SealEvidence,
    transaction: TransactionId,
    crypto: &WorkspaceCrypto,
) -> Result<ProtectedEvidence, OperationError> {
    if !evidence.matches_attempt(attempt) {
        return Err(OperationError::InvalidEvidence);
    }
    let bytes = evidence.raw_json().as_bytes();
    let chunks = bytes.chunks(PRIVATE_PAYLOAD_MAX_BYTES);
    let receipt = EvidenceReceipt::from_committed_store(evidence, transaction);
    let manifest = Manifest {
        version: 1,
        workspace: evidence.workspace_id(),
        operation_id: evidence.operation_id(),
        command_digest: evidence.command_digest(),
        plan_digest: *evidence.plan_digest(),
        recipient: evidence.recipient(),
        attempt_id: evidence.attempt_id(),
        claim_transaction: attempt.transaction_id(),
        intent_digest: *evidence.intent().digest(),
        event_id: evidence.event_id(),
        evidence_digest: *evidence.digest(),
        transaction_id: transaction,
        observed_at: evidence.observed_at_unix_ms().map(NonZeroU64::get),
        byte_count: bytes.len(),
        part_count: chunks.len(),
    };
    let plaintext =
        Zeroizing::new(serde_json::to_vec(&manifest).map_err(|_| OperationError::InvalidEvidence)?);
    let scope = attempt::scope(attempt);
    let manifest = crypto
        .seal_issuance(scope, IssuancePayload::Evidence, 0, &plaintext)
        .await?;
    let mut total = manifest.len();
    let mut parts = Vec::new();
    for (index, chunk) in chunks.enumerate() {
        let part = crypto
            .seal_issuance(scope, IssuancePayload::Evidence, (index + 1) as u8, chunk)
            .await?;
        total = total
            .checked_add(part.len())
            .filter(|size| *size <= MAX_PROTECTED_TOTAL)
            .ok_or(OperationError::InvalidEvidence)?;
        parts.push(part);
    }
    Ok(ProtectedEvidence {
        manifest,
        parts,
        receipt,
        bytes: bytes.len(),
    })
}
pub(super) async fn read(
    connection: &mut SqliteConnection,
    attempt: &SigningAttempt,
    crypto: &WorkspaceCrypto,
) -> Result<Option<(SealEvidence, ProtectedEvidence)>, OperationError> {
    let scope = attempt::scope(attempt);
    let row=sqlx::query("SELECT CASE WHEN length(attempt_id)=32 THEN attempt_id END,CASE WHEN length(event_id)=32 THEN event_id END,CASE WHEN length(evidence_digest)=32 THEN evidence_digest END,CASE WHEN length(transaction_id)=32 THEN transaction_id END,part_count,byte_count,CASE WHEN length(protected)>0 AND length(protected)<=262144 THEN protected END FROM seal_evidence WHERE workspace=? AND operation_id=? AND recipient=?")
        .bind(scope.workspace.as_bytes().as_slice()).bind(scope.operation.as_bytes().as_slice()).bind(scope.recipient.as_bytes().as_slice()).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let value = |index| {
        row.try_get::<Option<Vec<u8>>, _>(index)
            .map_err(|_| OperationError::RecoveryRequired)?
            .ok_or(OperationError::RecoveryRequired)
    };
    let attempt_id = value(0)?;
    let event_id = value(1)?;
    let digest = value(2)?;
    let transaction = value(3)?;
    let manifest = value(6)?;
    let count: i64 = row
        .try_get(4)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let bytes: i64 = row
        .try_get(5)
        .map_err(|_| OperationError::RecoveryRequired)?;
    if !(1..=4).contains(&count)
        || !(1..=65536).contains(&bytes)
        || (bytes as usize).div_ceil(PRIVATE_PAYLOAD_MAX_BYTES) != count as usize
    {
        return Err(OperationError::RecoveryRequired);
    }
    let transaction = TransactionId::from_bytes(
        transaction
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    );
    let plaintext = crypto
        .open_issuance(scope, IssuancePayload::Evidence, 0, &manifest)
        .await?;
    let decoded: Manifest = plaintext
        .expose_secret(|value| serde_json::from_slice(value))
        .map_err(|_| OperationError::RecoveryRequired)?;
    if decoded.version != 1
        || decoded.workspace != scope.workspace
        || decoded.operation_id != scope.operation
        || decoded.command_digest != attempt.bound_plan().prepared().identity().command_digest()
        || decoded.plan_digest != scope.plan_digest
        || decoded.recipient != scope.recipient
        || decoded.attempt_id != scope.attempt
        || decoded.claim_transaction != attempt.transaction_id()
        || decoded.intent_digest != *attempt.binding().digest()
        || decoded.event_id.as_bytes() != &attempt.seal().event_id()
        || decoded.event_id.as_bytes().as_slice() != event_id
        || decoded.attempt_id.as_bytes().as_slice() != attempt_id
        || decoded.evidence_digest.as_slice() != digest
        || decoded.transaction_id != transaction
        || decoded.part_count != count as usize
        || decoded.byte_count != bytes as usize
    {
        return Err(OperationError::RecoveryRequired);
    }
    let observed = match decoded.observed_at {
        None => None,
        Some(value) => Some(
            NonZeroU64::new(value)
                .filter(|time| time.get() <= 253_402_300_799_999)
                .ok_or(OperationError::RecoveryRequired)?,
        ),
    };
    let (parts,total):(i64,i64)=sqlx::query_as("SELECT count(*),coalesce(sum(length(protected)),0) FROM seal_evidence_parts WHERE workspace=? AND operation_id=? AND recipient=?")
        .bind(scope.workspace.as_bytes().as_slice()).bind(scope.operation.as_bytes().as_slice()).bind(scope.recipient.as_bytes().as_slice()).fetch_one(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    if parts != count || total <= 0 || total > (MAX_PROTECTED_TOTAL - manifest.len()) as i64 {
        return Err(OperationError::RecoveryRequired);
    }
    let rows=sqlx::query("SELECT part,CASE WHEN length(protected)>0 AND length(protected)<=262144 AND (SELECT coalesce(sum(length(p.protected)),0) FROM seal_evidence_parts p WHERE p.workspace=seal_evidence_parts.workspace AND p.operation_id=seal_evidence_parts.operation_id AND p.recipient=seal_evidence_parts.recipient)<=1048576 THEN protected END FROM seal_evidence_parts WHERE workspace=? AND operation_id=? AND recipient=? ORDER BY part LIMIT 5")
        .bind(scope.workspace.as_bytes().as_slice()).bind(scope.operation.as_bytes().as_slice()).bind(scope.recipient.as_bytes().as_slice()).fetch_all(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    if rows.len() != count as usize {
        return Err(OperationError::RecoveryRequired);
    }
    let mut raw = Zeroizing::new(Vec::with_capacity(bytes as usize));
    let mut protected_parts = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let number: i64 = row
            .try_get(0)
            .map_err(|_| OperationError::RecoveryRequired)?;
        if number != (index + 1) as i64 {
            return Err(OperationError::RecoveryRequired);
        }
        let protected: Vec<u8> = row
            .try_get::<Option<Vec<u8>>, _>(1)
            .map_err(|_| OperationError::RecoveryRequired)?
            .ok_or(OperationError::RecoveryRequired)?;
        let part = crypto
            .open_issuance(scope, IssuancePayload::Evidence, number as u8, &protected)
            .await?;
        let length = if index + 1 == rows.len() {
            bytes as usize - index * PRIVATE_PAYLOAD_MAX_BYTES
        } else {
            PRIVATE_PAYLOAD_MAX_BYTES
        };
        part.expose_secret(|value| {
            if value.len() != length {
                return Err(OperationError::RecoveryRequired);
            }
            raw.extend_from_slice(value);
            Ok(())
        })?;
        protected_parts.push(protected);
    }
    let text = core::str::from_utf8(&raw).map_err(|_| OperationError::RecoveryRequired)?;
    let evidence = SealEvidence::verify(attempt, text, observed)
        .map_err(|_| OperationError::RecoveryRequired)?;
    if evidence.digest() != &decoded.evidence_digest {
        return Err(OperationError::RecoveryRequired);
    }
    let receipt = EvidenceReceipt::from_committed_store(&evidence, transaction);
    Ok(Some((
        evidence,
        ProtectedEvidence {
            manifest,
            parts: protected_parts,
            receipt,
            bytes: bytes as usize,
        },
    )))
}
pub(super) async fn insert(
    connection: &mut SqliteConnection,
    value: &ProtectedEvidence,
) -> Result<(), OperationError> {
    let receipt = value.receipt;
    sqlx::query("INSERT INTO seal_evidence(workspace,operation_id,recipient,attempt_id,event_id,evidence_digest,transaction_id,part_count,byte_count,protected) VALUES(?,?,?,?,?,?,?,?,?,?)")
        .bind(receipt.workspace_id().as_bytes().as_slice()).bind(receipt.operation_id().as_bytes().as_slice()).bind(receipt.recipient().as_bytes().as_slice()).bind(receipt.attempt_id().as_bytes().as_slice()).bind(receipt.event_id().as_bytes().as_slice()).bind(receipt.digest().as_slice()).bind(receipt.transaction_id().as_bytes().as_slice()).bind(value.parts.len() as i64).bind(value.bytes as i64).bind(&value.manifest).execute(&mut *connection).await.map_err(|_|OperationError::StorageFailure)?;
    for (index, part) in value.parts.iter().enumerate() {
        sqlx::query("INSERT INTO seal_evidence_parts(workspace,operation_id,recipient,part,protected) VALUES(?,?,?,?,?)")
        .bind(receipt.workspace_id().as_bytes().as_slice()).bind(receipt.operation_id().as_bytes().as_slice()).bind(receipt.recipient().as_bytes().as_slice()).bind((index+1) as i64).bind(part).execute(&mut *connection).await.map_err(|_|OperationError::StorageFailure)?;
    }
    Ok(())
}
pub(super) async fn unchanged(
    connection: &mut SqliteConnection,
    attempt: &SigningAttempt,
    value: Option<&ProtectedEvidence>,
) -> Result<(), OperationError> {
    let scope = attempt::scope(attempt);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM seal_evidence WHERE workspace=? AND operation_id=? AND recipient=?",
    )
    .bind(scope.workspace.as_bytes().as_slice())
    .bind(scope.operation.as_bytes().as_slice())
    .bind(scope.recipient.as_bytes().as_slice())
    .fetch_one(&mut *connection)
    .await
    .map_err(|_| OperationError::RecoveryRequired)?;
    let parts:i64=sqlx::query_scalar("SELECT count(*) FROM seal_evidence_parts WHERE workspace=? AND operation_id=? AND recipient=?")
        .bind(scope.workspace.as_bytes().as_slice()).bind(scope.operation.as_bytes().as_slice()).bind(scope.recipient.as_bytes().as_slice()).fetch_one(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let Some(value) = value else {
        return if count == 0 && parts == 0 {
            Ok(())
        } else {
            Err(OperationError::RecoveryRequired)
        };
    };
    let receipt = value.receipt;
    let matched:i64=sqlx::query_scalar("SELECT count(*) FROM seal_evidence WHERE workspace=? AND operation_id=? AND recipient=? AND attempt_id=? AND event_id=? AND evidence_digest=? AND transaction_id=? AND part_count=? AND byte_count=? AND protected=?")
        .bind(scope.workspace.as_bytes().as_slice()).bind(scope.operation.as_bytes().as_slice()).bind(scope.recipient.as_bytes().as_slice()).bind(receipt.attempt_id().as_bytes().as_slice()).bind(receipt.event_id().as_bytes().as_slice()).bind(receipt.digest().as_slice()).bind(receipt.transaction_id().as_bytes().as_slice()).bind(value.parts.len() as i64).bind(value.bytes as i64).bind(&value.manifest).fetch_one(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    if count != 1 || matched != 1 || parts != value.parts.len() as i64 {
        return Err(OperationError::RecoveryRequired);
    }
    for (index, part) in value.parts.iter().enumerate() {
        let matched:i64=sqlx::query_scalar("SELECT count(*) FROM seal_evidence_parts WHERE workspace=? AND operation_id=? AND recipient=? AND part=? AND protected=?")
        .bind(scope.workspace.as_bytes().as_slice()).bind(scope.operation.as_bytes().as_slice()).bind(scope.recipient.as_bytes().as_slice()).bind((index+1) as i64).bind(part).fetch_one(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
        if matched != 1 {
            return Err(OperationError::RecoveryRequired);
        }
    }
    Ok(())
}

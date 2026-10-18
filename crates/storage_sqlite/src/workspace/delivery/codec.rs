use super::super::{
    crypto::{DeliveryPayload, DeliveryScope, PRIVATE_PAYLOAD_MAX_BYTES, WorkspaceCrypto},
    installation::EvidenceRef,
};
use super::{MAX_BYTES, MAX_PROTECTED_BYTES};
use radroots_domain::{EnvelopeEventId, TransactionId, UnixSeconds};
use radroots_storage::command::{DeliveryAttempt, DeliveryIntent, OperationError, PublicationWork};
use radroots_transport::{TargetSet, policy::SatisfactionPolicy};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqliteConnection};
use zeroize::Zeroizing;

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;

#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct WorkRef {
    installation: TransactionId,
    plan: [u8; 32],
    evidence: EvidenceRef,
    transaction: TransactionId,
    queued_at: UnixSeconds,
}
impl WorkRef {
    pub(super) fn new(work: PublicationWork) -> Self {
        Self {
            installation: work.installation_transaction_id(),
            plan: *work.plan_digest(),
            evidence: EvidenceRef::new(work.evidence()),
            transaction: work.transaction_id(),
            queued_at: work.queued_at(),
        }
    }
}

#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Header {
    version: u8,
    work: WorkRef,
    request: String,
    event: EnvelopeEventId,
    intent: [u8; 32],
    #[serde(deserialize_with = "super::json::targets")]
    targets: TargetSet,
    #[serde(deserialize_with = "super::json::small_vec")]
    selection: Vec<radroots_transport::target::TargetFingerprint>,
    #[serde(deserialize_with = "super::json::policy")]
    policy: SatisfactionPolicy,
    deadline: u64,
    transaction: TransactionId,
    retained_at: UnixSeconds,
    #[serde(deserialize_with = "super::json::small_vec")]
    ordinals: Vec<u8>,
}
impl Header {
    fn new(value: &DeliveryAttempt) -> Self {
        Self {
            version: 1,
            work: WorkRef::new(value.work()),
            request: value.intent().request_id().as_str().into(),
            event: value.intent().event_id(),
            intent: *value.intent().digest(),
            targets: value.intent().targets().clone(),
            selection: value.intent().selection().to_vec(),
            policy: value.intent().policy().clone(),
            deadline: value.intent().deadline_unix_ms().get(),
            transaction: value.transaction_id(),
            retained_at: value.retained_at(),
            ordinals: value.target_ordinals().to_vec(),
        }
    }
    fn into_attempt(
        self,
        work: PublicationWork,
        raw: &str,
    ) -> Result<DeliveryAttempt, OperationError> {
        if self.version != 1 || self.work != WorkRef::new(work) {
            return Err(OperationError::RecoveryRequired);
        }
        let intent = DeliveryIntent::new(
            &self.request,
            self.event,
            &self.targets,
            &self.policy,
            core::num::NonZeroU64::new(self.deadline).ok_or(OperationError::RecoveryRequired)?,
        )
        .map_err(|_| OperationError::RecoveryRequired)?;
        let selected = TargetSet::new(
            intent
                .targets()
                .targets()
                .iter()
                .filter(|target| self.selection.contains(target.fingerprint()))
                .cloned()
                .collect(),
        )
        .map_err(|_| OperationError::RecoveryRequired)?;
        if selected
            .targets()
            .iter()
            .map(|target| target.fingerprint())
            .ne(self.selection.iter())
        {
            return Err(OperationError::RecoveryRequired);
        }
        let intent = intent
            .with_selection(&selected)
            .map_err(|_| OperationError::RecoveryRequired)?;
        if intent.digest() != &self.intent {
            return Err(OperationError::RecoveryRequired);
        }
        DeliveryAttempt::from_store_report(
            work,
            intent,
            raw,
            self.transaction,
            self.retained_at,
            self.ordinals,
        )
        .map_err(|_| OperationError::RecoveryRequired)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u8,
    work: WorkRef,
    request: String,
    event: EnvelopeEventId,
    transaction: TransactionId,
    parts: usize,
    bytes: usize,
    digest: [u8; 32],
}

pub(super) fn request_key(request: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"radroots.delivery.request.key.v1\0");
    hash.update(request.as_bytes());
    hash.finalize().into()
}
pub(super) fn scope(work: PublicationWork, key: [u8; 32]) -> DeliveryScope {
    let e = work.evidence();
    DeliveryScope {
        workspace: e.workspace_id(),
        operation: e.operation_id(),
        recipient: e.recipient(),
        request: key,
    }
}
fn invalid<E>(_: E) -> OperationError {
    OperationError::RecoveryRequired
}
fn bytes(row: &sqlx::sqlite::SqliteRow, index: usize) -> Result<Vec<u8>, OperationError> {
    row.try_get::<Option<Vec<u8>>, _>(index)
        .map_err(invalid)?
        .ok_or(OperationError::RecoveryRequired)
}

pub(super) struct ProtectedAttempt {
    pub report: DeliveryAttempt,
    pub manifest: Vec<u8>,
    pub parts: Vec<Vec<u8>>,
    pub bytes: usize,
}
pub(super) struct Fact {
    pub scope: DeliveryScope,
    pub event: EnvelopeEventId,
    pub transaction: TransactionId,
    pub bytes: usize,
    pub manifest_digest: [u8; 32],
    pub parts: Vec<[u8; 32]>,
}
impl ProtectedAttempt {
    pub(super) fn fact(&self) -> Fact {
        Fact {
            scope: scope(
                self.report.work(),
                request_key(self.report.intent().request_id().as_str()),
            ),
            event: self.report.intent().event_id(),
            transaction: self.report.transaction_id(),
            bytes: self.bytes,
            manifest_digest: Sha256::digest(&self.manifest).into(),
            parts: self
                .parts
                .iter()
                .map(|v| Sha256::digest(v).into())
                .collect(),
        }
    }
}

pub(super) async fn protect(
    report: DeliveryAttempt,
    crypto: &WorkspaceCrypto,
) -> Result<ProtectedAttempt, OperationError> {
    // Every target descriptor is independently within the private-record bound;
    // the composite frame has the fixed 24-part owning-store bound.
    for target in report.intent().targets().targets() {
        if serde_json::to_vec(target)
            .map_err(|_| OperationError::InvalidCommand)?
            .len()
            > PRIVATE_PAYLOAD_MAX_BYTES
        {
            return Err(OperationError::InvalidCommand);
        }
    }
    let header = Zeroizing::new(
        serde_json::to_vec(&Header::new(&report)).map_err(|_| OperationError::InvalidCommand)?,
    );
    let size = 4usize
        .checked_add(header.len())
        .and_then(|n| n.checked_add(report.raw_json().len()))
        .filter(|n| *n <= MAX_BYTES)
        .ok_or(OperationError::InvalidCommand)?;
    let mut plain = Zeroizing::new(Vec::with_capacity(size));
    plain.extend_from_slice(&(header.len() as u32).to_be_bytes());
    plain.extend_from_slice(&header);
    plain.extend_from_slice(report.raw_json().as_bytes());
    let selected = scope(
        report.work(),
        request_key(report.intent().request_id().as_str()),
    );
    let mut parts = Vec::new();
    let mut total = 0usize;
    for (index, chunk) in plain.chunks(PRIVATE_PAYLOAD_MAX_BYTES).enumerate() {
        let protected = crypto
            .seal_delivery(selected, DeliveryPayload::Attempt, (index + 1) as u8, chunk)
            .await?;
        total = total
            .checked_add(protected.len())
            .filter(|n| *n <= MAX_PROTECTED_BYTES)
            .ok_or(OperationError::InvalidEvidence)?;
        parts.push(protected);
    }
    let manifest = Manifest {
        version: 1,
        work: WorkRef::new(report.work()),
        request: report.intent().request_id().as_str().into(),
        event: report.intent().event_id(),
        transaction: report.transaction_id(),
        parts: parts.len(),
        bytes: plain.len(),
        digest: Sha256::digest(&plain).into(),
    };
    let encoded =
        Zeroizing::new(serde_json::to_vec(&manifest).map_err(|_| OperationError::InvalidCommand)?);
    let manifest = crypto
        .seal_delivery(selected, DeliveryPayload::Attempt, 0, &encoded)
        .await?;
    if total
        .checked_add(manifest.len())
        .is_none_or(|n| n > MAX_PROTECTED_BYTES)
    {
        return Err(OperationError::InvalidEvidence);
    }
    Ok(ProtectedAttempt {
        report,
        manifest,
        parts,
        bytes: plain.len(),
    })
}

pub(super) async fn read(
    connection: &mut SqliteConnection,
    work: PublicationWork,
    key: [u8; 32],
    crypto: &WorkspaceCrypto,
) -> Result<Option<ProtectedAttempt>, OperationError> {
    let s = scope(work, key);
    let row=sqlx::query("SELECT CASE WHEN length(event_id)=32 THEN event_id END,CASE WHEN length(transaction_id)=32 THEN transaction_id END,part_count,byte_count,CASE WHEN length(protected)>0 AND length(protected)<=262144 THEN protected END FROM chat_delivery_attempts WHERE workspace=? AND operation_id=? AND recipient=? AND request_key=?")
        .bind(s.workspace.as_bytes().as_slice()).bind(s.operation.as_bytes().as_slice()).bind(s.recipient.as_bytes().as_slice()).bind(s.request.as_slice()).fetch_optional(&mut *connection).await.map_err(invalid)?;
    let Some(row) = row else { return Ok(None) };
    let event = bytes(&row, 0)?;
    let transaction = bytes(&row, 1)?;
    let manifest = bytes(&row, 4)?;
    let count: i64 = row.try_get(2).map_err(invalid)?;
    let size: i64 = row.try_get(3).map_err(invalid)?;
    if !(1..=24).contains(&count)
        || !(1..=MAX_BYTES as i64).contains(&size)
        || (size as usize).div_ceil(PRIVATE_PAYLOAD_MAX_BYTES) != count as usize
    {
        return Err(OperationError::RecoveryRequired);
    }
    let totals:(i64,i64)=sqlx::query_as("SELECT count(*),coalesce(sum(n),0) FROM (SELECT length(protected) AS n FROM chat_delivery_parts WHERE workspace=? AND operation_id=? AND recipient=? AND request_key=? LIMIT 25)")
        .bind(s.workspace.as_bytes().as_slice()).bind(s.operation.as_bytes().as_slice()).bind(s.recipient.as_bytes().as_slice()).bind(s.request.as_slice()).fetch_one(&mut *connection).await.map_err(invalid)?;
    if totals.0 != count
        || totals.1 <= 0
        || (totals.1 as usize)
            .checked_add(manifest.len())
            .is_none_or(|n| n > MAX_PROTECTED_BYTES)
    {
        return Err(OperationError::RecoveryRequired);
    }
    let decoded = crypto
        .open_delivery(s, DeliveryPayload::Attempt, 0, &manifest)
        .await?;
    let value: Manifest = decoded.expose_secret(|raw| {
        if !super::json::bounded(raw, 32) {
            return Err(OperationError::RecoveryRequired);
        }
        serde_json::from_slice(raw).map_err(invalid)
    })?;
    if value.version != 1
        || value.work != WorkRef::new(work)
        || request_key(&value.request) != key
        || value.event.as_bytes().as_slice() != event
        || value.transaction.as_bytes().as_slice() != transaction
        || value.parts != count as usize
        || value.bytes != size as usize
    {
        return Err(OperationError::RecoveryRequired);
    }
    let rows=sqlx::query("SELECT part,CASE WHEN length(protected)>0 AND length(protected)<=262144 THEN protected END FROM chat_delivery_parts WHERE workspace=? AND operation_id=? AND recipient=? AND request_key=? ORDER BY part LIMIT 25")
        .bind(s.workspace.as_bytes().as_slice()).bind(s.operation.as_bytes().as_slice()).bind(s.recipient.as_bytes().as_slice()).bind(s.request.as_slice()).fetch_all(&mut *connection).await.map_err(invalid)?;
    let mut parts = Vec::with_capacity(rows.len());
    let mut plain = Zeroizing::new(Vec::with_capacity(value.bytes));
    for (index, row) in rows.iter().enumerate() {
        if row.try_get::<i64, _>(0).map_err(invalid)? != index as i64 + 1 {
            return Err(OperationError::RecoveryRequired);
        }
        let protected = bytes(row, 1)?;
        let piece = crypto
            .open_delivery(s, DeliveryPayload::Attempt, (index + 1) as u8, &protected)
            .await?;
        piece.expose_secret(|raw| {
            if plain
                .len()
                .checked_add(raw.len())
                .is_none_or(|n| n > value.bytes)
            {
                return Err(OperationError::RecoveryRequired);
            }
            plain.extend_from_slice(raw);
            Ok(())
        })?;
        parts.push(protected);
    }
    if plain.len() != value.bytes
        || <[u8; 32]>::from(Sha256::digest(&plain)) != value.digest
        || plain.len() < 4
    {
        return Err(OperationError::RecoveryRequired);
    }
    let header_size = u32::from_be_bytes(plain[..4].try_into().map_err(invalid)?) as usize;
    let boundary = 4usize
        .checked_add(header_size)
        .filter(|n| *n < plain.len())
        .ok_or(OperationError::RecoveryRequired)?;
    if !super::json::bounded(&plain[4..boundary], 32) {
        return Err(OperationError::RecoveryRequired);
    }
    let header: Header = serde_json::from_slice(&plain[4..boundary]).map_err(invalid)?;
    let report = header.into_attempt(
        work,
        core::str::from_utf8(&plain[boundary..]).map_err(invalid)?,
    )?;
    if report.intent().request_id().as_str() != value.request
        || report.intent().event_id() != value.event
        || report.transaction_id() != value.transaction
    {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(Some(ProtectedAttempt {
        report,
        manifest,
        parts,
        bytes: value.bytes,
    }))
}

pub(super) async fn insert(
    connection: &mut SqliteConnection,
    value: &ProtectedAttempt,
) -> Result<(), OperationError> {
    let s = scope(
        value.report.work(),
        request_key(value.report.intent().request_id().as_str()),
    );
    sqlx::query("INSERT INTO chat_delivery_attempts(workspace,operation_id,recipient,request_key,event_id,transaction_id,part_count,byte_count,protected) VALUES(?,?,?,?,?,?,?,?,?)")
        .bind(s.workspace.as_bytes().as_slice()).bind(s.operation.as_bytes().as_slice()).bind(s.recipient.as_bytes().as_slice()).bind(s.request.as_slice()).bind(value.report.intent().event_id().as_bytes().as_slice()).bind(value.report.transaction_id().as_bytes().as_slice()).bind(value.parts.len() as i64).bind(value.bytes as i64).bind(&value.manifest).execute(&mut *connection).await.map_err(|_|OperationError::StorageFailure)?;
    for (index, part) in value.parts.iter().enumerate() {
        sqlx::query("INSERT INTO chat_delivery_parts(workspace,operation_id,recipient,request_key,part,protected) VALUES(?,?,?,?,?,?)")
        .bind(s.workspace.as_bytes().as_slice()).bind(s.operation.as_bytes().as_slice()).bind(s.recipient.as_bytes().as_slice()).bind(s.request.as_slice()).bind(index as i64+1).bind(part).execute(&mut *connection).await.map_err(|_|OperationError::StorageFailure)?;
    }
    Ok(())
}

impl Fact {
    pub(super) async fn unchanged(
        &self,
        connection: &mut SqliteConnection,
    ) -> Result<(), OperationError> {
        let s = self.scope;
        let row=sqlx::query("SELECT CASE WHEN length(event_id)=32 THEN event_id END,CASE WHEN length(transaction_id)=32 THEN transaction_id END,part_count,byte_count,CASE WHEN length(protected)>0 AND length(protected)<=262144 THEN protected END FROM chat_delivery_attempts WHERE workspace=? AND operation_id=? AND recipient=? AND request_key=?")
            .bind(s.workspace.as_bytes().as_slice()).bind(s.operation.as_bytes().as_slice()).bind(s.recipient.as_bytes().as_slice()).bind(s.request.as_slice()).fetch_optional(&mut *connection).await.map_err(invalid)?.ok_or(OperationError::RecoveryRequired)?;
        if bytes(&row, 0)? != self.event.as_bytes()
            || bytes(&row, 1)? != self.transaction.as_bytes()
            || row.try_get::<i64, _>(2).map_err(invalid)? != self.parts.len() as i64
            || row.try_get::<i64, _>(3).map_err(invalid)? != self.bytes as i64
            || <[u8; 32]>::from(Sha256::digest(bytes(&row, 4)?)) != self.manifest_digest
        {
            return Err(OperationError::RecoveryRequired);
        }
        // Compare one bounded ciphertext at a time. No provider, decryption,
        // signing or network operation occurs while holding the write fence.
        for (index, expected) in self.parts.iter().enumerate() {
            let row=sqlx::query("SELECT CASE WHEN length(protected)>0 AND length(protected)<=262144 THEN protected END FROM chat_delivery_parts WHERE workspace=? AND operation_id=? AND recipient=? AND request_key=? AND part=?")
                .bind(s.workspace.as_bytes().as_slice()).bind(s.operation.as_bytes().as_slice()).bind(s.recipient.as_bytes().as_slice()).bind(s.request.as_slice()).bind(index as i64+1).fetch_optional(&mut *connection).await.map_err(invalid)?.ok_or(OperationError::RecoveryRequired)?;
            if <[u8; 32]>::from(Sha256::digest(bytes(&row, 0)?)) != *expected {
                return Err(OperationError::RecoveryRequired);
            }
        }
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM (SELECT 1 FROM chat_delivery_parts WHERE workspace=? AND operation_id=? AND recipient=? AND request_key=? LIMIT 25)")
            .bind(s.workspace.as_bytes().as_slice()).bind(s.operation.as_bytes().as_slice()).bind(s.recipient.as_bytes().as_slice()).bind(s.request.as_slice()).fetch_one(connection).await.map_err(invalid)?;
        if count != self.parts.len() as i64 {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}

pub(super) async fn absent(
    connection: &mut SqliteConnection,
    work: PublicationWork,
    key: [u8; 32],
) -> Result<(), OperationError> {
    let s = scope(work, key);
    let count:i64=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM chat_delivery_attempts WHERE workspace=?1 AND operation_id=?2 AND recipient=?3 AND request_key=?4)+EXISTS(SELECT 1 FROM chat_delivery_parts WHERE workspace=?1 AND operation_id=?2 AND recipient=?3 AND request_key=?4)")
        .bind(s.workspace.as_bytes().as_slice()).bind(s.operation.as_bytes().as_slice()).bind(s.recipient.as_bytes().as_slice()).bind(s.request.as_slice()).fetch_one(connection).await.map_err(invalid)?;
    if count != 0 {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}

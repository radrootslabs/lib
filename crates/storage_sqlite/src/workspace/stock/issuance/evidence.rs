//! Append-only obtained bytes. Capture never obtains fresh business permission.
use super::super::inventory::{Turn, checkpoint};
use super::*;
use radroots_signing::{
    issuance::{OutcomeKind, SigningOutcome},
    request::CancellationSignal,
};
use std::num::NonZeroU64;
use zeroize::Zeroizing;
mod codec;
pub(in crate::workspace::stock) use codec::read_entry;
const MAX_RAW: usize = 64 * 1024;

/// Independently checked obtained bytes. A store error leaves the caller's
/// original borrowed value intact. This supplies no installation or retry right.
pub struct StockEvidenceCapture {
    anchor: CommitAttempt,
    binding: WorkspaceBinding,
    claim: OperationId,
    intent: [u8; 32],
    raw: Zeroizing<String>,
    observed: Option<NonZeroU64>,
}
impl StockEvidenceCapture {
    pub fn verify(
        binding: WorkspaceBinding,
        attempt: &StockSigningAttempt,
        raw: &str,
        observed: Option<NonZeroU64>,
    ) -> Result<Self, OperationError> {
        if raw.len() > MAX_RAW {
            return Err(WorkspaceError::Capacity.into());
        }
        if binding.input().workspace_id() != attempt.attempt().workspace_id()
            || binding.actor().participant() != attempt.bound_plan().preparation().rumor().author()
        {
            return Err(OperationError::InvalidEvidence);
        }
        let selected = claim::intent(
            attempt.bound_plan(),
            attempt.recipient(),
            CancellationSignal::new(),
        )?;
        if selected.binding() != attempt.binding() {
            return Err(OperationError::InvalidEvidence);
        }
        let (_, kind) = SigningOutcome::capture(&selected, raw.into(), observed).into_parts();
        let OutcomeKind::Obtained(signed) = kind else {
            return Err(OperationError::InvalidEvidence);
        };
        Ok(Self {
            anchor: CommitAttempt::new(
                attempt.attempt().workspace_id(),
                attempt.attempt().operation_id(),
                attempt.attempt().command_digest(),
                None,
            ),
            binding,
            claim: OperationId::from_bytes(*attempt.attempt_id().as_bytes()),
            intent: *attempt.binding().digest(),
            raw: Zeroizing::new(raw.into()),
            observed: signed.observed_at_unix_ms(),
        })
    }
    pub fn with_json<T>(&self, use_json: impl FnOnce(&str) -> T) -> T {
        use_json(&self.raw)
    }
    pub const fn observed_at_unix_ms(&self) -> Option<NonZeroU64> {
        self.observed
    }
    pub(in crate::workspace) const fn attempt(&self) -> CommitAttempt {
        self.anchor
    }
    pub(in crate::workspace) fn retained_copy(&self) -> Self {
        Self {
            anchor: self.anchor,
            binding: self.binding.clone(),
            claim: self.claim,
            intent: self.intent,
            raw: Zeroizing::new(self.raw.to_string()),
            observed: self.observed,
        }
    }
}
impl std::fmt::Debug for StockEvidenceCapture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockEvidenceCapture(<redacted>)")
    }
}

/// Confirmed append identity, independent of command installation or delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StockEvidenceReceipt {
    attempt: CommitAttempt,
    claim: SigningAttemptId,
    recipient: PublicKey,
    digest: [u8; 32],
    observed: Option<NonZeroU64>,
}
impl StockEvidenceReceipt {
    pub const fn attempt(&self) -> CommitAttempt {
        self.attempt
    }
    pub const fn signing_attempt_id(&self) -> SigningAttemptId {
        self.claim
    }
    pub const fn recipient(&self) -> PublicKey {
        self.recipient
    }
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub const fn observed_at_unix_ms(&self) -> Option<NonZeroU64> {
        self.observed
    }
}
pub struct StockCapturedEvidence {
    receipt: StockEvidenceReceipt,
    raw: Zeroizing<String>,
}
impl StockCapturedEvidence {
    pub const fn receipt(&self) -> StockEvidenceReceipt {
        self.receipt
    }
    pub fn with_json<T>(&self, use_json: impl FnOnce(&str) -> T) -> T {
        use_json(&self.raw)
    }
}
impl std::fmt::Debug for StockCapturedEvidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockCapturedEvidence(<redacted>)")
    }
}
#[derive(Debug)]
pub enum StockEvidenceOutcome {
    Committed(StockEvidenceReceipt),
    Unknown(CommitAttempt),
}

/// At most 32 authenticated references in append order. The cursor discovers
/// retained bytes after process loss without reconstructing a signer output.
#[derive(Debug)]
pub struct StockEvidencePage {
    receipts: Vec<StockEvidenceReceipt>,
    next: Option<Sequence>,
}
impl StockEvidencePage {
    pub fn receipts(&self) -> &[StockEvidenceReceipt] {
        &self.receipts
    }
    pub const fn next(&self) -> Option<Sequence> {
        self.next
    }
}

pub(in crate::workspace) async fn page(
    turn: &mut Turn<'_>,
    request: StockClaimRequest,
    after: Option<Sequence>,
    proof: AttachmentProof,
) -> Result<StockEvidencePage, OperationError> {
    let auth = super::super::signing_plan::load(
        turn,
        &request.binding,
        request.minimum,
        request.operation,
        request.digest,
        &proof,
    )
    .await?;
    if auth.access.authority.catalog != 11 {
        return Err(OperationError::UnsupportedOperation);
    }
    let minimum = InventoryCheckpointRef::from_host_metadata(
        turn.selected.authority(),
        Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?,
        None,
    )?;
    let witness = checkpoint::authenticate_minimum(
        turn.connection,
        turn.selected,
        turn.crypto,
        &auth.access.current,
        &auth.access.authority,
        minimum,
    )
    .await?;
    let rows=sqlx::query("SELECT CASE WHEN length(e.evidence_digest)=32 THEN e.evidence_digest END,CASE WHEN length(e.entry_id)=32 THEN e.entry_id END,o.revision FROM stock_signing_evidence e JOIN stock_operations o ON o.actor=e.actor AND o.operation_id=e.entry_id WHERE e.actor=? AND e.operation_id=? AND e.recipient=? AND (? IS NULL OR o.revision>?) ORDER BY o.revision LIMIT 33")
        .bind(turn.selected.participant().as_bytes().as_slice()).bind(request.operation.as_bytes().as_slice()).bind(request.recipient.as_bytes().as_slice()).bind(after.map(|v|v.get() as i64)).bind(after.map(|v|v.get() as i64)).fetch_all(&mut *turn.connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let more = rows.len() > 32;
    let mut last = None;
    let mut receipts = Vec::with_capacity(rows.len().min(32));
    for row in rows.into_iter().take(32) {
        let digest: Vec<u8> = row
            .try_get::<Option<Vec<u8>>, _>(0)
            .map_err(|_| OperationError::RecoveryRequired)?
            .ok_or(OperationError::RecoveryRequired)?;
        let entry: Vec<u8> = row
            .try_get::<Option<Vec<u8>>, _>(1)
            .map_err(|_| OperationError::RecoveryRequired)?
            .ok_or(OperationError::RecoveryRequired)?;
        let retained = read_entry(
            turn.connection,
            turn.selected,
            turn.crypto,
            OperationId::from_bytes(
                entry
                    .try_into()
                    .map_err(|_| OperationError::RecoveryRequired)?,
            ),
        )
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
        let revision: i64 = row
            .try_get(2)
            .map_err(|_| OperationError::RecoveryRequired)?;
        if revision <= 0 || retained.frame.header.revision.get() != revision as u64 {
            return Err(OperationError::RecoveryRequired);
        }
        last = Some(retained.frame.header.revision);
        let value = retained.value.receipt;
        if value.digest.as_slice() != digest
            || value.attempt.operation_id() != request.operation
            || value.recipient != request.recipient
        {
            return Err(OperationError::RecoveryRequired);
        }
        receipts.push(value);
    }
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let checked: Result<(), OperationError> = async {
        witness.confirm(&mut tx).await?;
        auth.confirm(&mut tx, turn.namespace, turn.clock, &proof, false)
            .await?;
        Ok(())
    }
    .await;
    if tx.rollback().await.is_err() {
        *turn.uncertain = true;
        return Err(OperationError::RecoveryRequired);
    }
    checked?;
    let next = if more {
        Some(last.ok_or(OperationError::RecoveryRequired)?)
    } else {
        None
    };
    Ok(StockEvidencePage { receipts, next })
}

pub(in crate::workspace) async fn capture(
    turn: &mut Turn<'_>,
    supplied: StockEvidenceCapture,
    proof: AttachmentProof,
) -> Result<StockEvidenceOutcome, OperationError> {
    if *turn.uncertain {
        return Err(OperationError::RecoveryRequired);
    }
    turn.namespace.validate()?;
    let input = supplied.binding.input();
    if !SellerAuthoritySelection::from_attachment(turn.selected).matches_attachment(input) {
        return Err(OperationError::Unauthorized);
    }
    let (authority, record, root, grant) =
        super::super::read(turn.connection, input, turn.crypto).await?;
    if authority.catalog != 11 {
        return Err(OperationError::UnsupportedOperation);
    }
    record.authenticate_retained_capture(input, &proof)?;
    let original = claim::read_entry(turn.connection, turn.selected, turn.crypto, supplied.claim)
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
    if original.frame.header.actor != supplied.binding.actor()
        || original.report.binding().digest() != &supplied.intent
    {
        return Err(OperationError::InvalidEvidence);
    }
    // Verify again against the actual protected claim, never a caller descriptor.
    let checked = StockEvidenceCapture::verify(
        supplied.binding.clone(),
        &original.report,
        &supplied.raw,
        supplied.observed,
    )?;
    let digest: [u8; 32] = Sha256::digest(checked.raw.as_bytes()).into();
    let before = authority
        .inventory
        .ok_or(OperationError::RecoveryRequired)?;
    let minimum = InventoryCheckpointRef::from_host_metadata(
        input.authority(),
        Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?,
        None,
    )?;
    let witness = checkpoint::authenticate_minimum(
        turn.connection,
        turn.selected,
        turn.crypto,
        &supplied.binding,
        &authority,
        minimum,
    )
    .await?;
    let existing = lookup(
        turn.connection,
        turn.selected,
        turn.crypto,
        original.report.bound_plan().plan().operation_id(),
        original.report.recipient(),
        digest,
    )
    .await?;
    let (frame, updated_root, receipt) = if let Some(retained) = &existing {
        if retained.value.raw.as_str() != checked.raw.as_str()
            || retained.value.receipt.claim != original.report.attempt_id()
        {
            return Err(OperationError::RecoveryRequired);
        }
        (None, None, retained.value.receipt)
    } else {
        let entry = OperationId::from_bytes(super::super::super::random()?);
        let transaction = TransactionId::from_bytes(super::super::super::random()?);
        let revision = Sequence::new(
            before
                .revision
                .get()
                .checked_add(1)
                .ok_or(WorkspaceError::Capacity)?,
        )
        .map_err(|_| WorkspaceError::Capacity)?;
        let body = Zeroizing::new(codec::encode(&original, &checked));
        let mut header = ledger::Header {
            version: 5,
            family: ledger::Family::SigningEvidence,
            namespace: SellerAuthoritySelection::from_attachment(turn.selected),
            actor: original.frame.header.actor,
            operation: entry,
            digest: original.frame.header.digest,
            transaction,
            revision,
            previous: before.head,
            head: radroots_domain::CheckpointDigest::from_bytes([0; 32]),
            pool: None,
            order: original.frame.header.order,
            related_operation: original.frame.header.related_operation,
            subject_grant: None,
            bytes: 0,
            parts: 0,
            body_digest: [0; 32],
        };
        header.head = codec::head(&header, &body);
        let mut next = authority.clone();
        next.inventory = Some(InventoryCheckpoint {
            revision,
            head: Some(header.head),
            ..before
        });
        let frame = ledger::prepare(turn.crypto, turn.selected, header, &body).await?;
        let updated = turn
            .crypto
            .seal_stock_root(turn.selected, next.catalog, &next.encode()?)
            .await?;
        let receipt = StockEvidenceReceipt {
            attempt: CommitAttempt::new(
                input.workspace_id(),
                original.report.bound_plan().plan().operation_id(),
                original.frame.header.digest,
                Some(transaction),
            ),
            claim: original.report.attempt_id(),
            recipient: original.report.recipient(),
            digest,
            observed: checked.observed,
        };
        (Some(frame), Some(updated), receipt)
    };
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let changed:Result<(),OperationError>=async {
        turn.namespace.validate()?;
        super::super::unchanged(&mut tx,input,authority.catalog,&root,&grant).await?;
        record.authenticate_retained_capture(input,&proof)?;
        witness.confirm(&mut tx).await?;original.confirm(&mut tx).await?;
        if let Some(existing)=&existing {existing.confirm(&mut tx).await?;}
        if let Some(frame)=&frame {
            frame.insert_record(&mut tx).await?;
            sqlx::query("INSERT INTO stock_signing_evidence(actor,operation_id,recipient,evidence_digest,entry_id) VALUES(?,?,?,?,?)").bind(input.participant().as_bytes().as_slice()).bind(receipt.attempt.operation_id().as_bytes().as_slice()).bind(receipt.recipient.as_bytes().as_slice()).bind(digest.as_slice()).bind(frame.header.operation.as_bytes().as_slice()).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
            let result=sqlx::query("UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?").bind(updated_root.as_ref().ok_or(OperationError::RecoveryRequired)?).bind(&root).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
            if result.rows_affected()!=1 {return Err(OperationError::RecoveryRequired);}
        }
        turn.namespace.validate()?;Ok(())
    }.await;
    if let Err(error) = changed {
        if tx.rollback().await.is_err() {
            *turn.uncertain = true;
            return Ok(StockEvidenceOutcome::Unknown(receipt.attempt));
        }
        return Err(error);
    }
    let done = if frame.is_some() {
        tx.commit().await
    } else {
        tx.rollback().await
    };
    if done.is_err() {
        *turn.uncertain = true;
        return Ok(StockEvidenceOutcome::Unknown(receipt.attempt));
    }
    Ok(StockEvidenceOutcome::Committed(receipt))
}

async fn lookup(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    operation: OperationId,
    recipient: PublicKey,
    digest: [u8; 32],
) -> Result<Option<codec::Retained>, OperationError> {
    let id:Option<Vec<u8>>=sqlx::query_scalar("SELECT CASE WHEN length(entry_id)=32 THEN entry_id END FROM stock_signing_evidence WHERE actor=? AND operation_id=? AND recipient=? AND evidence_digest=?").bind(input.participant().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(recipient.as_bytes().as_slice()).bind(digest.as_slice()).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    match id {
        None => Ok(None),
        Some(id) => Ok(Some(
            read_entry(
                connection,
                input,
                crypto,
                OperationId::from_bytes(
                    id.try_into()
                        .map_err(|_| OperationError::RecoveryRequired)?,
                ),
            )
            .await?
            .ok_or(OperationError::RecoveryRequired)?,
        )),
    }
}

pub(in crate::workspace) async fn read(
    turn: &mut Turn<'_>,
    request: StockClaimRequest,
    digest: [u8; 32],
    proof: AttachmentProof,
) -> Result<Option<StockCapturedEvidence>, OperationError> {
    let auth = super::super::signing_plan::load(
        turn,
        &request.binding,
        request.minimum,
        request.operation,
        request.digest,
        &proof,
    )
    .await?;
    if auth.access.authority.catalog != 11 {
        return Err(OperationError::UnsupportedOperation);
    }
    let claim = claim::existing(turn, &auth, request.operation, request.recipient).await?;
    let retained = lookup(
        turn.connection,
        turn.selected,
        turn.crypto,
        request.operation,
        request.recipient,
        digest,
    )
    .await?;
    if retained.is_some() && claim.0.is_none() {
        return Err(OperationError::RecoveryRequired);
    }
    // Full protected ancestry proves natural evidence presence/absence.
    let minimum = InventoryCheckpointRef::from_host_metadata(
        turn.selected.authority(),
        Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?,
        None,
    )?;
    let witness = checkpoint::authenticate_minimum(
        turn.connection,
        turn.selected,
        turn.crypto,
        &auth.access.current,
        &auth.access.authority,
        minimum,
    )
    .await?;
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let checked: Result<(), OperationError> = async {
        witness.confirm(&mut tx).await?;
        claim.1.confirm(&mut tx).await?;
        auth.confirm(&mut tx, turn.namespace, turn.clock, &proof, false)
            .await?;
        if let Some(value) = &retained {
            value.confirm(&mut tx).await?;
        }
        Ok(())
    }
    .await;
    if tx.rollback().await.is_err() {
        *turn.uncertain = true;
        return Err(OperationError::RecoveryRequired);
    }
    checked?;
    Ok(retained.map(|value| value.value))
}
impl NativeWorkspace {
    pub async fn stock_signing_evidence_page(
        &self,
        request: StockClaimRequest,
        after: Option<Sequence>,
        proof: &AttachmentProof,
    ) -> Result<StockEvidencePage, OperationError> {
        self.worker
            .stock_signing_evidence_page(
                request,
                after,
                proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?,
            )
            .await
    }
    pub async fn capture_stock_signing_evidence(
        &self,
        evidence: &StockEvidenceCapture,
        proof: &AttachmentProof,
    ) -> Result<StockEvidenceOutcome, OperationError> {
        self.worker
            .capture_stock_signing_evidence(
                evidence.retained_copy(),
                proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?,
            )
            .await
    }
    pub async fn stock_signing_evidence(
        &self,
        request: StockClaimRequest,
        digest: [u8; 32],
        proof: &AttachmentProof,
    ) -> Result<Option<StockCapturedEvidence>, OperationError> {
        self.worker
            .stock_signing_evidence(
                request,
                digest,
                proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?,
            )
            .await
    }
}

//! Retain an owned original invocation's refusal; this never releases stock.
use super::super::inventory::{Turn, checkpoint};
use super::*;
use radroots_domain::CheckpointDigest;
use radroots_signing::provider::ProviderError;
use zeroize::Zeroizing;

const MAGIC: &[u8; 8] = b"RDSTKN1\0";
const BYTES: usize = 105;

/// An authenticated retained fact about one original invocation, not a permit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StockUnissuedReceipt {
    attempt: CommitAttempt,
    claim: SigningAttemptId,
    recipient: PublicKey,
    reason: ProviderError,
    checkpoint: InventoryCheckpointRef,
}
impl StockUnissuedReceipt {
    pub const fn attempt(&self) -> CommitAttempt {
        self.attempt
    }
    pub const fn signing_attempt_id(&self) -> SigningAttemptId {
        self.claim
    }
    pub const fn recipient(&self) -> PublicKey {
        self.recipient
    }
    pub const fn reason(&self) -> ProviderError {
        self.reason
    }
    pub const fn checkpoint(&self) -> InventoryCheckpointRef {
        self.checkpoint
    }
}
/// Confirmed append versus actual transaction uncertainty, never business success.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StockUnissuedOutcome {
    Committed(Box<StockUnissuedReceipt>),
    Unknown(CommitAttempt),
}

/// Private worker transport, constructed only from the owned original proof.
pub(in crate::workspace) struct Request {
    pub(in crate::workspace) binding: WorkspaceBinding,
    claim: SigningAttemptId,
    operation: OperationId,
    digest: CommandDigest,
    recipient: PublicKey,
    intent: [u8; 32],
    reason: u8,
}
impl Request {
    fn from_observation(
        binding: WorkspaceBinding,
        value: &StockUnissuedInvocation,
    ) -> Result<Self, OperationError> {
        let original = value.attempt();
        if binding.input().workspace_id() != original.attempt().workspace_id()
            || binding.actor().participant() != original.bound_plan().preparation().rumor().author()
        {
            return Err(OperationError::InvalidEvidence);
        }
        Ok(Self {
            binding,
            claim: original.attempt_id(),
            operation: original.attempt().operation_id(),
            digest: original.attempt().command_digest(),
            recipient: original.recipient(),
            intent: *original.binding().digest(),
            reason: encode_reason(value.reason())?,
        })
    }
    pub(in crate::workspace) fn attempt(&self) -> CommitAttempt {
        CommitAttempt::new(
            self.binding.input().workspace_id(),
            self.operation,
            self.digest,
            None,
        )
    }
}
fn encode_reason(reason: ProviderError) -> Result<u8, OperationError> {
    Ok(match reason.as_str() {
        "invalid_input" => 1,
        "limit_exceeded" => 2,
        "unsupported_version" => 3,
        "capability_unavailable" => 4,
        "identity_changed" => 5,
        "authentication_failed" => 6,
        "cancelled" => 7,
        "deadline_exceeded" => 8,
        "provider_failure" => 9,
        _ => return Err(OperationError::UnsupportedOperation),
    })
}
fn decode_reason(reason: u8) -> Result<ProviderError, OperationError> {
    Ok(match reason {
        1 => ProviderError::InvalidInput,
        2 => ProviderError::LimitExceeded,
        3 => ProviderError::UnsupportedVersion,
        4 => ProviderError::CapabilityUnavailable,
        5 => ProviderError::IdentityChanged,
        6 => ProviderError::AuthenticationFailed,
        7 => ProviderError::Cancelled,
        8 => ProviderError::DeadlineExceeded,
        9 => ProviderError::ProviderFailure,
        _ => return Err(OperationError::RecoveryRequired),
    })
}
fn body(original: &claim::codec::Retained, reason: u8) -> Zeroizing<Vec<u8>> {
    let mut bytes = Zeroizing::new(Vec::with_capacity(BYTES));
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(original.frame.header.operation.as_bytes());
    bytes.extend_from_slice(original.frame.header.transaction.as_bytes());
    bytes.extend_from_slice(original.frame.header.head.as_bytes());
    bytes.push(reason);
    bytes
}
fn head(header: &ledger::Header, body: &[u8]) -> CheckpointDigest {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.original_unissued_chain.v1\0");
    hash.update(header.namespace.workspace_id().as_bytes());
    hash.update(header.namespace.authority().executor_id().as_bytes());
    hash.update(
        header
            .namespace
            .authority()
            .generation()
            .get()
            .to_be_bytes(),
    );
    hash.update(header.namespace.route_digest().as_bytes());
    hash.update(header.actor.participant().as_bytes());
    hash.update(
        header
            .actor
            .grant_id()
            .map(|v| *v.as_bytes())
            .unwrap_or([0; 32]),
    );
    hash.update(header.operation.as_bytes());
    hash.update(header.digest.as_bytes());
    hash.update(header.transaction.as_bytes());
    hash.update(header.revision.get().to_be_bytes());
    hash.update([u8::from(header.previous.is_some())]);
    hash.update(header.previous.map(|v| *v.as_bytes()).unwrap_or([0; 32]));
    hash.update(Sha256::digest(body));
    CheckpointDigest::from_bytes(hash.finalize().into())
}
fn receipt(
    input: &AttachmentInput,
    header: &ledger::Header,
    original: &claim::codec::Retained,
    reason: u8,
) -> Result<StockUnissuedReceipt, OperationError> {
    Ok(StockUnissuedReceipt {
        attempt: CommitAttempt::new(
            input.workspace_id(),
            original.report.attempt().operation_id(),
            header.digest,
            Some(header.transaction),
        ),
        claim: original.report.attempt_id(),
        recipient: original.report.recipient(),
        reason: decode_reason(reason)?,
        checkpoint: InventoryCheckpointRef::from_host_metadata(
            input.authority(),
            header.revision,
            Some(header.head),
        )?,
    })
}
pub(in crate::workspace::stock) struct Retained {
    frame: ledger::Retained,
    value: Box<StockUnissuedReceipt>,
    projection: Projection,
}
impl Retained {
    pub(in crate::workspace::stock) fn identity(&self) -> checkpoint::EntryIdentity {
        checkpoint::EntryIdentity {
            operation: self.frame.header.operation,
            revision: self.frame.header.revision,
            head: self.frame.header.head,
            protected_digest: Sha256::digest(&self.frame.protected).into(),
        }
    }
    pub(in crate::workspace::stock) fn receipt(&self) -> StockUnissuedReceipt {
        *self.value
    }
    pub(in crate::workspace::stock) async fn confirm(
        &self,
        connection: &mut SqliteConnection,
    ) -> Result<(), OperationError> {
        ledger::confirm_header(connection, &self.frame.header, &self.frame.protected).await?;
        ledger::confirm_parts(
            connection,
            self.frame.header.actor,
            self.frame.header.operation,
            self.frame.parts_witness,
            self.frame.header.parts,
        )
        .await?;
        self.projection
            .confirm(
                connection,
                self.frame.header.actor.participant(),
                self.frame.header.operation,
            )
            .await?;
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM stock_movements WHERE actor=? AND operation_id=?",
        )
        .bind(self.frame.header.actor.participant().as_bytes().as_slice())
        .bind(self.frame.header.operation.as_bytes().as_slice())
        .fetch_one(connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
        if count != 0 {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
    pub(in crate::workspace::stock) fn into_history(self) -> ledger::HistoryEntry {
        ledger::HistoryEntry {
            aborted: None,
            preparation: None,
            installation: None,
            header: self.frame.header,
            protected: self.frame.protected,
            parts_witness: self.frame.parts_witness,
            pools: vec![],
            mapping: None,
            movements: vec![],
            signing: Some(self.projection),
            local_record: None,
        }
    }
}
pub(in crate::workspace::stock) async fn read_entry(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    entry: OperationId,
) -> Result<Option<Retained>, OperationError> {
    let Some(frame) = ledger::read(connection, input, crypto, entry).await? else {
        return Ok(None);
    };
    let raw = frame.body.as_slice();
    if frame.header.family != ledger::Family::OriginalUnissued
        || raw.len() != BYTES
        || &raw[..8] != MAGIC
    {
        return Err(OperationError::RecoveryRequired);
    }
    let array = |start, end| {
        raw[start..end]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)
    };
    let original = Box::pin(claim::read_entry(
        connection,
        input,
        crypto,
        OperationId::from_bytes(array(8, 40)?),
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    if original.frame.header.transaction != TransactionId::from_bytes(array(40, 72)?)
        || original.frame.header.head != CheckpointDigest::from_bytes(array(72, 104)?)
        || frame.header.actor != original.frame.header.actor
        || frame.header.related_operation != original.frame.header.related_operation
        || frame.header.order != original.frame.header.order
        || frame.header.digest != original.frame.header.digest
        || frame.header.revision <= original.frame.header.revision
        || head(&frame.header, raw) != frame.header.head
    {
        return Err(OperationError::RecoveryRequired);
    }
    let reason = raw[104];
    decode_reason(reason)?;
    let projection = Projection {
        operation: original.report.attempt().operation_id(),
        recipient: original.report.recipient(),
        kind: ProjectionKind::OriginalUnissued(reason),
    };
    let value = receipt(input, &frame.header, &original, reason)?;
    let retained = Retained {
        frame,
        value: Box::new(value),
        projection,
    };
    retained.confirm(connection).await?;
    Ok(Some(retained))
}
pub(in crate::workspace::stock) async fn lookup(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    operation: OperationId,
    recipient: PublicKey,
) -> Result<Option<Retained>, OperationError> {
    let entry:Option<Vec<u8>>=sqlx::query_scalar("SELECT CASE WHEN length(entry_id)=32 THEN entry_id END FROM stock_original_unissued WHERE actor=? AND operation_id=? AND recipient=?")
        .bind(input.participant().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(recipient.as_bytes().as_slice())
        .fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let Some(entry) = entry else { return Ok(None) };
    let retained = Box::pin(read_entry(
        connection,
        input,
        crypto,
        OperationId::from_bytes(
            entry
                .try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        ),
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    if retained.value.attempt.operation_id() != operation || retained.value.recipient != recipient {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(Some(retained))
}
pub(in crate::workspace) async fn capture(
    turn: &mut Turn<'_>,
    request: Request,
    proof: AttachmentProof,
) -> Result<StockUnissuedOutcome, OperationError> {
    if *turn.uncertain {
        return Err(OperationError::RecoveryRequired);
    }
    turn.namespace.validate()?;
    let input = request.binding.input();
    if !SellerAuthoritySelection::from_attachment(turn.selected).matches_attachment(input) {
        return Err(OperationError::Unauthorized);
    }
    let (authority, record, root, grant) =
        super::super::read(turn.connection, input, turn.crypto).await?;
    if !matches!(authority.catalog, 14..=15) {
        return Err(OperationError::UnsupportedOperation);
    }
    record.authenticate_retained_capture(input, &proof)?;
    let original = Box::pin(claim::read_entry(
        turn.connection,
        input,
        turn.crypto,
        OperationId::from_bytes(*request.claim.as_bytes()),
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    if original.frame.header.actor != request.binding.actor()
        || original.report.attempt().operation_id() != request.operation
        || original.report.attempt().command_digest() != request.digest
        || original.report.recipient() != request.recipient
        || original.report.binding().digest() != &request.intent
    {
        return Err(OperationError::InvalidEvidence);
    }
    decode_reason(request.reason)?;
    let before = authority
        .inventory
        .ok_or(OperationError::RecoveryRequired)?;
    let witness = Box::pin(checkpoint::authenticate_minimum(
        turn.connection,
        turn.selected,
        turn.crypto,
        &request.binding,
        &authority,
        InventoryCheckpointRef::from_host_metadata(
            input.authority(),
            Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?,
            None,
        )?,
    ))
    .await?;
    let existing = Box::pin(lookup(
        turn.connection,
        input,
        turn.crypto,
        request.operation,
        request.recipient,
    ))
    .await?;
    let (frame, updated_root, value) = if let Some(retained) = &existing {
        if retained.value.claim != request.claim
            || retained.value.reason != decode_reason(request.reason)?
        {
            return Err(OperationError::RecoveryRequired);
        }
        (None, None, *retained.value)
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
        let body = body(&original, request.reason);
        let mut header = ledger::Header {
            version: 5,
            family: ledger::Family::OriginalUnissued,
            namespace: SellerAuthoritySelection::from_attachment(turn.selected),
            actor: original.frame.header.actor,
            operation: entry,
            digest: original.frame.header.digest,
            transaction,
            revision,
            previous: before.head,
            head: CheckpointDigest::from_bytes([0; 32]),
            pool: None,
            order: original.frame.header.order,
            related_operation: original.frame.header.related_operation,
            subject_grant: None,
            bytes: 0,
            parts: 0,
            body_digest: [0; 32],
        };
        header.head = head(&header, &body);
        let value = receipt(input, &header, &original, request.reason)?;
        let mut next = authority.clone();
        next.inventory = Some(InventoryCheckpoint {
            revision,
            head: Some(header.head),
            ..before
        });
        let frame = ledger::prepare(turn.crypto, input, header, &body).await?;
        let updated = turn
            .crypto
            .seal_stock_root(input, next.catalog, &next.encode()?)
            .await?;
        (Some(frame), Some(updated), value)
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
        if let Some(retained)=&existing {retained.confirm(&mut tx).await?;}
        if let Some(frame)=&frame {
            frame.insert_record(&mut tx).await?;
            sqlx::query("INSERT INTO stock_original_unissued(actor,operation_id,recipient,reason,entry_id) VALUES(?,?,?,?,?)")
                .bind(input.participant().as_bytes().as_slice()).bind(request.operation.as_bytes().as_slice())
                .bind(request.recipient.as_bytes().as_slice()).bind(i64::from(request.reason))
                .bind(frame.header.operation.as_bytes().as_slice()).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
            let updated=sqlx::query("UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?")
                .bind(updated_root.as_ref().ok_or(OperationError::RecoveryRequired)?).bind(&root)
                .execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
            if updated.rows_affected()!=1 {return Err(OperationError::RecoveryRequired);}
        }
        turn.namespace.validate()?;Ok(())
    }.await;
    if let Err(error) = changed {
        if tx.rollback().await.is_err() {
            *turn.uncertain = true;
            return Ok(StockUnissuedOutcome::Unknown(value.attempt));
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
        return Ok(StockUnissuedOutcome::Unknown(value.attempt));
    }
    Ok(StockUnissuedOutcome::Committed(Box::new(value)))
}
pub(in crate::workspace) async fn read(
    turn: &mut Turn<'_>,
    request: StockClaimRequest,
    proof: AttachmentProof,
) -> Result<Option<StockUnissuedReceipt>, OperationError> {
    let auth = super::super::signing_plan::load(
        turn,
        &request.binding,
        request.minimum,
        request.operation,
        request.digest,
        &proof,
    )
    .await?;
    if !matches!(auth.access.authority.catalog, 14..=15) {
        return Err(OperationError::UnsupportedOperation);
    }
    let witness = Box::pin(checkpoint::authenticate_minimum(
        turn.connection,
        turn.selected,
        turn.crypto,
        &auth.access.current,
        &auth.access.authority,
        InventoryCheckpointRef::from_host_metadata(
            turn.selected.authority(),
            Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?,
            None,
        )?,
    ))
    .await?;
    let retained = Box::pin(lookup(
        turn.connection,
        turn.selected,
        turn.crypto,
        request.operation,
        request.recipient,
    ))
    .await?;
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let checked: Result<(), OperationError> = async {
        witness.confirm(&mut tx).await?;
        if let Some(value) = &retained {
            value.confirm(&mut tx).await?;
        }
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
    Ok(retained.map(|v| *v.value))
}
impl super::super::NativeWorkspace {
    /// Borrows the actual owned original observation. Failed/uncertain writes
    /// leave that observation available; no new issuance or stock effect occurs.
    pub async fn capture_stock_original_unissued(
        &self,
        binding: WorkspaceBinding,
        observation: &StockUnissuedInvocation,
        proof: &AttachmentProof,
    ) -> Result<StockUnissuedOutcome, OperationError> {
        self.worker
            .capture_stock_original_unissued(
                Request::from_observation(binding, observation)?,
                proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?,
                false,
            )
            .await
    }
    /// Reconciles only original material on this namespace's retained owner.
    pub async fn reconcile_stock_original_unissued(
        &self,
        binding: WorkspaceBinding,
        observation: &StockUnissuedInvocation,
        proof: &AttachmentProof,
    ) -> Result<StockUnissuedOutcome, OperationError> {
        self.worker
            .capture_stock_original_unissued(
                Request::from_observation(binding, observation)?,
                proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?,
                true,
            )
            .await
    }
    /// Reads the authenticated original invocation fact; never a release permit.
    pub async fn stock_original_unissued(
        &self,
        request: StockClaimRequest,
        proof: &AttachmentProof,
    ) -> Result<Option<StockUnissuedReceipt>, OperationError> {
        self.worker
            .stock_original_unissued(
                request,
                proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?,
            )
            .await
    }
}

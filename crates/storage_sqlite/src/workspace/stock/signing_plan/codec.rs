use super::*;
use zeroize::Zeroizing;
const MAGIC: &[u8] = b"radroots.stock.signing_plan.v1\0";
pub(super) const PREFIX_BYTES: usize = MAGIC.len() + 32 * 5 + 8 * 2;
pub(in crate::workspace::stock) struct Retained {
    pub frame: ledger::Retained,
    pub bound: StockBoundSigningPlan,
}
pub(super) fn encode(
    original: &acceptance::codec::Retained,
    plan: &CommerceSigningPlan,
    at: UnixSeconds,
) -> Result<Zeroizing<Vec<u8>>, OperationError> {
    if at < original.receipt.prepared_at() || at >= original.receipt.first_issue_deadline() {
        return Err(OperationError::DeadlineExceeded);
    }
    let length = PREFIX_BYTES + plan.protected_wire().len();
    if length > ledger::BODY_MAX_BYTES {
        return Err(WorkspaceError::Capacity.into());
    }
    let mut body = Zeroizing::new(Vec::with_capacity(length));
    body.extend_from_slice(MAGIC);
    body.extend_from_slice(original.frame.header.operation.as_bytes());
    body.extend_from_slice(original.frame.header.transaction.as_bytes());
    body.extend_from_slice(original.frame.header.head.as_bytes());
    body.extend_from_slice(&original.receipt.first_issue_deadline().get().to_be_bytes());
    body.extend_from_slice(&at.get().to_be_bytes());
    for intent in intents(plan, original.receipt.first_issue_deadline())? {
        body.extend_from_slice(&intent);
    }
    body.extend_from_slice(plan.protected_wire());
    Ok(body)
}
pub(super) fn head(header: &ledger::Header, body: &[u8]) -> CheckpointDigest {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.signing_plan_chain.v1\0");
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
            .map(|value| *value.as_bytes())
            .unwrap_or([0; 32]),
    );
    hash.update(header.operation.as_bytes());
    hash.update(header.digest.as_bytes());
    hash.update(header.transaction.as_bytes());
    hash.update(header.revision.get().to_be_bytes());
    hash.update([u8::from(header.previous.is_some())]);
    hash.update(
        header
            .previous
            .map(|value| *value.as_bytes())
            .unwrap_or([0; 32]),
    );
    hash.update(Sha256::digest(body));
    CheckpointDigest::from_bytes(hash.finalize().into())
}
pub(in crate::workspace::stock) async fn read_entry(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    operation: OperationId,
) -> Result<Option<Retained>, OperationError> {
    let Some(frame) = ledger::read(connection, input, crypto, operation).await? else {
        return Ok(None);
    };
    let header = &frame.header;
    if header.family != ledger::Family::SigningPlan
        || frame.body.len() <= PREFIX_BYTES
        || &frame.body[..MAGIC.len()] != MAGIC
    {
        return Err(OperationError::RecoveryRequired);
    }
    let mut offset = MAGIC.len();
    fn take<const N: usize>(bytes: &[u8], offset: &mut usize) -> Result<[u8; N], OperationError> {
        let value = bytes
            .get(*offset..*offset + N)
            .ok_or(OperationError::RecoveryRequired)?;
        *offset += N;
        value
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)
    }
    let related = OperationId::from_bytes(take(&frame.body, &mut offset)?);
    let transaction = TransactionId::from_bytes(take(&frame.body, &mut offset)?);
    let original_head = CheckpointDigest::from_bytes(take(&frame.body, &mut offset)?);
    let deadline = u64::from_be_bytes(take(&frame.body, &mut offset)?);
    let at = UnixSeconds::new(u64::from_be_bytes(take(&frame.body, &mut offset)?))
        .map_err(|_| OperationError::RecoveryRequired)?;
    let expected_intents: [[u8; 32]; 2] = [
        take(&frame.body, &mut offset)?,
        take(&frame.body, &mut offset)?,
    ];
    let original = acceptance::codec::read(connection, input, crypto, related)
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
    if header.related_operation != Some(related)
        || header.digest != original.frame.header.digest
        || header.order != original.frame.header.order
        || header.actor != original.frame.header.actor
        || header.revision <= original.frame.header.revision
        || transaction != original.frame.header.transaction
        || original_head != original.frame.header.head
        || deadline != original.receipt.first_issue_deadline().get()
        || at < original.receipt.prepared_at()
        || at >= original.receipt.first_issue_deadline()
        || head(header, &frame.body) != header.head
    {
        return Err(OperationError::RecoveryRequired);
    }
    let plan = CommerceSigningPlan::parse(
        related,
        header.digest,
        original.receipt.rumor(),
        &frame.body[offset..],
    )
    .map_err(|_| OperationError::RecoveryRequired)?;
    let intents = intents(&plan, original.receipt.first_issue_deadline())?;
    if expected_intents != intents {
        return Err(OperationError::RecoveryRequired);
    }
    let checkpoint = InventoryCheckpointRef::from_host_metadata(
        input.authority(),
        header.revision,
        Some(header.head),
    )?
    .with_optional_authorization(original.receipt.checkpoint().authorization());
    let bound = StockBoundSigningPlan {
        intents,
        attempt: CommitAttempt::new(
            input.workspace_id(),
            related,
            header.digest,
            Some(header.transaction),
        ),
        preparation: original.receipt,
        plan,
        bound_at: at,
        checkpoint,
    };
    let retained = Retained { frame, bound };
    retained.confirm(connection).await?;
    Ok(Some(retained))
}
impl Retained {
    pub(in crate::workspace::stock) fn identity(&self) -> EntryIdentity {
        EntryIdentity {
            operation: self.frame.header.operation,
            revision: self.frame.header.revision,
            head: self.frame.header.head,
            protected_digest: Sha256::digest(&self.frame.protected).into(),
        }
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
            local_record: None,
            installation: None,
            signing: None,
            header: self.frame.header,
            protected: self.frame.protected,
            parts_witness: self.frame.parts_witness,
            pools: vec![],
            mapping: None,
            movements: vec![],
        }
    }
}

pub(super) fn intents(
    plan: &CommerceSigningPlan,
    deadline: UnixSeconds,
) -> Result<[[u8; 32]; 2], OperationError> {
    let deadline = std::num::NonZeroU64::new(
        deadline
            .get()
            .checked_mul(1000)
            .ok_or(OperationError::RecoveryRequired)?,
    )
    .ok_or(OperationError::RecoveryRequired)?;
    let mut result = [[0; 32]; 2];
    for (index, seal) in plan.seals().iter().enumerate() {
        let call = radroots_signing::provider::ProviderCall::new(
            seal.fields().author,
            deadline,
            radroots_signing::request::CancellationSignal::new(),
        );
        result[index] = *radroots_signing::issuance::SigningIntent::from_commerce_seal(seal, call)
            .map_err(|_| OperationError::InvalidEvidence)?
            .binding()
            .digest();
    }
    Ok(result)
}

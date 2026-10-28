//! Fixed prefix followed by original exact JSON, without escape inflation.
use super::*;
const PREFIX: usize = 117;
const MAGIC: &[u8; 8] = b"RDSTKS1\0";
pub(super) fn encode(
    original: &claim::codec::Retained,
    supplied: &StockEvidenceRequest,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(PREFIX + supplied.with_json(str::len));
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(original.frame.header.operation.as_bytes());
    out.extend_from_slice(original.frame.header.transaction.as_bytes());
    out.extend_from_slice(original.frame.header.head.as_bytes());
    out.push(u8::from(supplied.observed_at_unix_ms().is_some()));
    out.extend_from_slice(
        &supplied
            .observed_at_unix_ms()
            .map(NonZeroU64::get)
            .unwrap_or(0)
            .to_be_bytes(),
    );
    out.extend_from_slice(&(supplied.with_json(str::len) as u32).to_be_bytes());
    supplied.with_json(|raw| out.extend_from_slice(raw.as_bytes()));
    out
}
pub(super) fn head(header: &ledger::Header, body: &[u8]) -> radroots_domain::CheckpointDigest {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.signing_evidence_chain.v1\0");
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
    radroots_domain::CheckpointDigest::from_bytes(hash.finalize().into())
}
pub(in crate::workspace::stock) struct Retained {
    pub frame: ledger::Retained,
    pub value: StockCapturedEvidence,
    projection: Projection,
    local_record: Box<radroots_protocol::commerce::DecodedRecord>,
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
    Ok(Some(
        Box::pin(read_frame(connection, input, crypto, frame)).await?,
    ))
}
async fn read_frame(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    frame: ledger::Retained,
) -> Result<Retained, OperationError> {
    let body = frame.body.as_slice();
    if frame.header.family != ledger::Family::SigningEvidence
        || !(PREFIX..=PREFIX + MAX_RAW).contains(&body.len())
        || &body[..8] != MAGIC
    {
        return Err(OperationError::RecoveryRequired);
    }
    let array = |start, end| {
        body[start..end]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)
    };
    let claim = OperationId::from_bytes(array(8, 40)?);
    let transaction = TransactionId::from_bytes(array(40, 72)?);
    let original_head = radroots_domain::CheckpointDigest::from_bytes(array(72, 104)?);
    let at = u64::from_be_bytes(
        body[105..113]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    );
    let observed = match (body[104], at) {
        (0, 0) => None,
        (1, n) if n > 0 => NonZeroU64::new(n),
        _ => return Err(OperationError::RecoveryRequired),
    };
    let length = u32::from_be_bytes(
        body[113..PREFIX]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    ) as usize;
    if length != body.len() - PREFIX {
        return Err(OperationError::RecoveryRequired);
    }
    let raw = std::str::from_utf8(&body[PREFIX..]).map_err(|_| OperationError::RecoveryRequired)?;
    let original = Box::pin(claim::read_entry(connection, input, crypto, claim))
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
    let header = &frame.header;
    if original.frame.header.transaction != transaction
        || original.frame.header.head != original_head
        || header.actor != original.frame.header.actor
        || header.related_operation != original.frame.header.related_operation
        || header.order != original.frame.header.order
        || header.digest != original.frame.header.digest
        || header.revision <= original.frame.header.revision
        || head(header, body) != header.head
    {
        return Err(OperationError::RecoveryRequired);
    }
    let selected = claim::intent(
        original.report.bound_plan(),
        original.report.recipient(),
        CancellationSignal::new(),
    )?;
    let (_, kind) = SigningOutcome::capture(&selected, raw.into(), observed).into_parts();
    let OutcomeKind::Obtained(signed) = kind else {
        return Err(OperationError::RecoveryRequired);
    };
    if signed.observed_at_unix_ms() != observed {
        return Err(OperationError::RecoveryRequired);
    }
    let digest = Sha256::digest(raw.as_bytes()).into();
    let rumor = original.report.bound_plan().preparation().rumor();
    let logical =
        radroots_protocol::private_rumor::DecodedRumor::decode(rumor.protected_wire().as_bytes())
            .map_err(|_| OperationError::RecoveryRequired)?;
    let local_record = radroots_protocol::commerce::decode(logical.protected_content().as_bytes())
        .map_err(|_| OperationError::RecoveryRequired)?;
    if local_record.record_id() != rumor.record_id() || logical.event_id() != rumor.event_id() {
        return Err(OperationError::RecoveryRequired);
    }
    let projection = Projection {
        operation: original.report.bound_plan().plan().operation_id(),
        recipient: original.report.recipient(),
        kind: ProjectionKind::Obtained(digest),
    };
    let receipt = StockEvidenceReceipt::from_committed_store(
        CommitAttempt::new(
            input.workspace_id(),
            projection.operation,
            header.digest,
            Some(header.transaction),
        ),
        original.report.attempt_id(),
        projection.recipient,
        digest,
        observed,
    )?;
    let value = StockCapturedEvidence {
        receipt,
        raw: Zeroizing::new(raw.into()),
    };
    let retained = Retained {
        frame,
        value,
        projection,
        local_record: Box::new(local_record),
    };
    retained.confirm(connection).await?;
    Ok(retained)
}
impl Retained {
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
            envelope: None,
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
            local_record: Some(self.local_record),
        }
    }
}

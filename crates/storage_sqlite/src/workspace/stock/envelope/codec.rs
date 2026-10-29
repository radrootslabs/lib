//! Fixed original source references and bounded exact outer bytes.
use super::*;
use nostr::JsonUtil;
use std::num::NonZeroU64;
const MAGIC: &[u8; 8] = b"RDSTE01\0";
const PREFIX: usize = 213;
const MAX_RAW: usize = 65536;

pub(super) struct Original {
    pub installation: inventory::installation::Retained,
    pub evidence: issuance::evidence::Retained,
}
impl Original {
    pub(super) async fn load(
        turn: &mut Turn<'_>,
        request: &StockEnvelopeRequest,
    ) -> Result<Self, OperationError> {
        let entry:Option<Vec<u8>>=sqlx::query_scalar("SELECT installation_entry FROM stock_publication_work WHERE actor=? AND operation_id=? AND recipient=?")
            .bind(turn.selected.participant().as_bytes().as_slice()).bind(request.operation_id().as_bytes().as_slice()).bind(request.recipient().as_bytes().as_slice())
            .fetch_optional(&mut *turn.connection).await.map_err(|_|OperationError::RecoveryRequired)?;
        let entry = OperationId::from_bytes(
            entry
                .ok_or(OperationError::MissingHistory)?
                .try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        );
        let installation = Box::pin(inventory::installation::read_entry(
            turn.connection,
            turn.selected,
            turn.crypto,
            entry,
        ))
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
        let summary = Box::pin(issuance::evidence::selection::Summary::read(
            turn.connection,
            turn.selected.participant(),
            request.operation_id(),
            request.recipient(),
        ))
        .await?;
        let selected = summary.first.ok_or(OperationError::MissingHistory)?;
        let evidence = Box::pin(issuance::evidence::selection::open(
            turn.connection,
            turn.selected,
            turn.crypto,
            request.operation_id(),
            request.recipient(),
            selected,
        ))
        .await?;
        let value = Self {
            installation,
            evidence,
        };
        value.validate()?;
        if value.installation.receipt.attempt().operation_id() != request.operation_id()
            || value.evidence.value.receipt().attempt().command_digest() != request.command_digest()
        {
            return Err(OperationError::InvalidEvidence);
        }
        Ok(value)
    }
    fn validate(&self) -> Result<(), OperationError> {
        let installed = &self.installation;
        let evidence = &self.evidence;
        let source = evidence.value.receipt();
        if installed.receipt.attempt().operation_id() != source.attempt().operation_id()
            || installed.frame.header.digest != source.attempt().command_digest()
            || installed.frame.header.order != evidence.frame.header.order
            || installed.frame.header.actor.participant()
                != evidence.frame.header.actor.participant()
        {
            return Err(OperationError::RecoveryRequired);
        }
        let work = installed
            .receipt
            .publication_work()
            .iter()
            .find(|work| work.recipient() == source.recipient())
            .ok_or(OperationError::RecoveryRequired)?;
        let event = evidence
            .value
            .with_json(|raw| nostr::Event::from_json(raw))
            .map_err(|_| OperationError::RecoveryRequired)?;
        if work.seal_id() != &event.id.to_bytes()
            || work
                .evidence_digest()
                .is_some_and(|digest| digest != source.digest())
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}
fn reference(out: &mut Vec<u8>, header: &ledger::Header) {
    out.extend_from_slice(header.operation.as_bytes());
    out.extend_from_slice(header.transaction.as_bytes());
    out.extend_from_slice(header.head.as_bytes());
}
pub(super) fn encode(
    original: &Original,
    request: &StockEnvelopeRequest,
    candidate: &PrivateEnvelopeCandidate,
) -> Zeroizing<Vec<u8>> {
    let mut bytes = Zeroizing::new(Vec::with_capacity(PREFIX + candidate.raw_json().len()));
    bytes.extend_from_slice(MAGIC);
    reference(&mut bytes, &original.installation.frame.header);
    reference(&mut bytes, &original.evidence.frame.header);
    bytes.push(u8::from(request.observed_at_unix_ms().is_some()));
    bytes.extend_from_slice(
        &request
            .observed_at_unix_ms()
            .map(NonZeroU64::get)
            .unwrap_or(0)
            .to_be_bytes(),
    );
    bytes.extend_from_slice(&(candidate.raw_json().len() as u32).to_be_bytes());
    bytes.extend_from_slice(candidate.raw_json().as_bytes());
    bytes
}
pub(super) fn head(header: &ledger::Header, body: &[u8]) -> CheckpointDigest {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.envelope_chain.v1\0");
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
pub(in crate::workspace::stock) struct Retained {
    pub(in crate::workspace::stock) frame: ledger::Retained,
    pub source: StockEnvelopeSource,
    pub(super) projection: Projection,
    pub(super) installation: OperationId,
}
impl Retained {
    pub(in crate::workspace::stock) fn installation_entry(&self) -> OperationId {
        self.installation
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
            delivery: None,
            envelope: Some(self.projection),
            aborted: None,
            preparation: None,
            header: self.frame.header,
            protected: self.frame.protected,
            parts_witness: self.frame.parts_witness,
            pools: vec![],
            mapping: None,
            movements: vec![],
            signing: None,
            installation: None,
            local_record: None,
        }
    }
}
pub(in crate::workspace::stock) async fn read_entry(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    entry: OperationId,
) -> Result<Option<Retained>, OperationError> {
    let Some(frame) = Box::pin(ledger::read(connection, input, crypto, entry)).await? else {
        return Ok(None);
    };
    if frame.header.family == ledger::Family::RewrappedEnvelope {
        return Ok(Some(
            Box::pin(super::rewrap::decode_entry(
                connection, input, crypto, binding, frame,
            ))
            .await?,
        ));
    }
    Ok(Some(
        Box::pin(decode_entry(connection, input, crypto, binding, frame)).await?,
    ))
}
pub(super) async fn read_original_entry(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    entry: OperationId,
) -> Result<Retained, OperationError> {
    let frame = Box::pin(ledger::read(connection, input, crypto, entry))
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
    Box::pin(decode_entry(connection, input, crypto, binding, frame)).await
}
async fn decode_entry(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    frame: ledger::Retained,
) -> Result<Retained, OperationError> {
    let body = frame.body.as_slice();
    if frame.header.family != ledger::Family::Envelope
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
    let installation = Box::pin(inventory::installation::read_entry(
        connection,
        input,
        crypto,
        OperationId::from_bytes(array(8, 40)?),
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    let evidence = Box::pin(issuance::evidence::read_entry(
        connection,
        input,
        crypto,
        OperationId::from_bytes(array(104, 136)?),
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    let original = Original {
        installation,
        evidence,
    };
    original.validate()?;
    let h = &frame.header;
    if original.installation.frame.header.transaction != TransactionId::from_bytes(array(40, 72)?)
        || original.installation.frame.header.head != CheckpointDigest::from_bytes(array(72, 104)?)
        || original.evidence.frame.header.transaction != TransactionId::from_bytes(array(136, 168)?)
        || original.evidence.frame.header.head != CheckpointDigest::from_bytes(array(168, 200)?)
        || h.actor != original.evidence.frame.header.actor
        || h.digest != original.evidence.frame.header.digest
        || h.related_operation != original.evidence.frame.header.related_operation
        || h.order != original.evidence.frame.header.order
        || h.revision <= original.evidence.frame.header.revision
        || h.revision <= original.installation.frame.header.revision
        || head(h, body) != h.head
    {
        return Err(OperationError::RecoveryRequired);
    }
    let at = u64::from_be_bytes(
        body[201..209]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    );
    let observed = match (body[200], at) {
        (0, 0) => None,
        (1, n) if n > 0 => NonZeroU64::new(n),
        _ => return Err(OperationError::RecoveryRequired),
    };
    let length = u32::from_be_bytes(
        body[209..213]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    ) as usize;
    if length != body.len() - PREFIX {
        return Err(OperationError::RecoveryRequired);
    }
    let raw = std::str::from_utf8(&body[PREFIX..]).map_err(|_| OperationError::RecoveryRequired)?;
    let event = nostr::Event::from_json(raw).map_err(|_| OperationError::RecoveryRequired)?;
    let evidence = original.evidence.value.receipt();
    let operation = h
        .related_operation
        .ok_or(OperationError::RecoveryRequired)?;
    // This is actual caller context plus decoded selectors, never a fabricated
    // authenticated binding. The public receipt remains metadata, not authority.
    let report = StockEnvelopeRequest::new(
        binding.clone(),
        InventoryCheckpointRef::from_host_metadata(
            input.authority(),
            Sequence::new(0).map_err(|_| OperationError::RecoveryRequired)?,
            None,
        )?,
        operation,
        h.digest,
        evidence.recipient(),
        observed,
    )?;
    let receipt = StockEnvelopeReceipt::from_committed_store(
        &report,
        CommitAttempt::new(
            input.workspace_id(),
            operation,
            h.digest,
            Some(h.transaction),
        ),
        evidence,
        original.installation.frame.header.transaction,
        EnvelopeEventId::from_bytes(event.id.to_bytes()),
        Sha256::digest(raw.as_bytes()).into(),
        InventoryCheckpointRef::from_host_metadata(input.authority(), h.revision, Some(h.head))?,
    )?;
    let source = StockEnvelopeSource::from_retained_wire(receipt, raw)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let retained = Retained {
        installation: original.installation.frame.header.operation,
        projection: Projection::Original {
            operation,
            recipient: receipt.recipient(),
        },
        frame,
        source,
    };
    retained.confirm(connection).await?;
    Ok(retained)
}

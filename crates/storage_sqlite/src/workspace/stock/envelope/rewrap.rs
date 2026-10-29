//! Additional obtained outers reference immutable original F10/S/T2.
use super::*;
use nostr::JsonUtil;
use radroots_storage::inventory::{StockRewrapPage, StockRewrapRequest, StockRewrapState};
use std::num::NonZeroU64;
#[path = "rewrap_capture.rs"]
mod capture;
#[path = "rewrap_selection.rs"]
pub(super) mod selection;
pub(super) use capture::capture;
use selection::SelectionWitness;
const MAGIC: &[u8; 8] = b"RDSTRW1\0";
const PREFIX: usize = 117;
const COUNT_MAX: i64 = 192;

fn head(header: &ledger::Header, body: &[u8]) -> CheckpointDigest {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.rewrap_chain.v1\0");
    hash.update([14]);
    hash.update(codec::head(header, body).as_bytes());
    CheckpointDigest::from_bytes(hash.finalize().into())
}
fn encode(
    original: &rewrap_admission::Admitted,
    request: &StockEnvelopeRequest,
    raw: &str,
) -> Zeroizing<Vec<u8>> {
    let mut body = Zeroizing::new(Vec::with_capacity(PREFIX + raw.len()));
    body.extend_from_slice(MAGIC);
    body.extend_from_slice(original.frame.header.operation.as_bytes());
    body.extend_from_slice(original.frame.header.transaction.as_bytes());
    body.extend_from_slice(original.frame.header.head.as_bytes());
    body.push(u8::from(request.observed_at_unix_ms().is_some()));
    body.extend_from_slice(
        &request
            .observed_at_unix_ms()
            .map(NonZeroU64::get)
            .unwrap_or(0)
            .to_be_bytes(),
    );
    body.extend_from_slice(&(raw.len() as u32).to_be_bytes());
    body.extend_from_slice(raw.as_bytes());
    body
}
pub(super) async fn lookup(
    turn: &mut Turn<'_>,
    request: &StockEnvelopeRequest,
    event: EnvelopeEventId,
) -> Result<Option<Retained>, OperationError> {
    let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *turn.connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
    if catalog != 18 {
        return Err(OperationError::UnsupportedOperation);
    }
    let entry: Option<Vec<u8>> = sqlx::query_scalar("SELECT CASE WHEN length(entry_id)=32 THEN entry_id END FROM stock_rewraps WHERE actor=? AND operation_id=? AND recipient=? AND event_id=?")
        .bind(turn.selected.participant().as_bytes().as_slice())
        .bind(request.operation_id().as_bytes().as_slice()).bind(request.recipient().as_bytes().as_slice())
        .bind(event.as_bytes().as_slice()).fetch_optional(&mut *turn.connection).await.map_err(|_| OperationError::RecoveryRequired)?;
    let Some(entry) = entry else { return Ok(None) };
    let entry = OperationId::from_bytes(
        entry
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    );
    let retained = Box::pin(read_entry(
        turn.connection,
        turn.selected,
        turn.crypto,
        request.binding(),
        entry,
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    let receipt = retained.source.receipt();
    if retained.frame.header.family != ledger::Family::RewrappedEnvelope
        || receipt.attempt().operation_id() != request.operation_id()
        || receipt.attempt().command_digest() != request.command_digest()
        || receipt.recipient() != request.recipient()
        || receipt.event_id() != event
    {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(Some(retained))
}
pub(super) async fn decode_entry(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    frame: ledger::Retained,
) -> Result<Retained, OperationError> {
    let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
    let body = frame.body.as_slice();
    if catalog != 18
        || frame.header.family != ledger::Family::RewrappedEnvelope
        || !(PREFIX..=PREFIX + 65536).contains(&body.len())
        || &body[..8] != MAGIC
    {
        return Err(OperationError::RecoveryRequired);
    }
    let array = |start, end| -> Result<[u8; 32], OperationError> {
        body[start..end]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)
    };
    // F14 references a strict F13, whose strict F10 link cannot recurse.
    let admission = Box::pin(rewrap_admission::read_entry(
        connection,
        input,
        crypto,
        binding,
        OperationId::from_bytes(array(8, 40)?),
    ))
    .await?;
    let h = &frame.header;
    let original_header = &admission.frame.header;
    if original_header.transaction != TransactionId::from_bytes(array(40, 72)?)
        || original_header.head != CheckpointDigest::from_bytes(array(72, 104)?)
        || h.actor != original_header.actor
        || h.digest != original_header.digest
        || h.related_operation != original_header.related_operation
        || h.order != original_header.order
        || h.revision <= original_header.revision
        || head(h, body) != h.head
    {
        return Err(OperationError::RecoveryRequired);
    }
    let at = u64::from_be_bytes(
        body[105..113]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    );
    let observed = match (body[104], at) {
        (0, 0) => None,
        (1, value) if value > 0 => NonZeroU64::new(value),
        _ => return Err(OperationError::RecoveryRequired),
    };
    let length = u32::from_be_bytes(
        body[113..117]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    ) as usize;
    if length != body.len() - PREFIX {
        return Err(OperationError::RecoveryRequired);
    }
    let raw = std::str::from_utf8(&body[PREFIX..]).map_err(|_| OperationError::RecoveryRequired)?;
    let event = nostr::Event::from_json(raw).map_err(|_| OperationError::RecoveryRequired)?;
    let event = EnvelopeEventId::from_bytes(event.id.to_bytes());
    let first = admission.report.original();
    if first.event_id() == event {
        return Err(OperationError::RecoveryRequired);
    }
    let operation = h
        .related_operation
        .ok_or(OperationError::RecoveryRequired)?;
    let request = StockEnvelopeRequest::new(
        binding.clone(),
        first.checkpoint(),
        operation,
        h.digest,
        first.recipient(),
        observed,
    )?
    .with_envelope(event);
    let receipt = StockEnvelopeReceipt::from_committed_store(
        &request,
        CommitAttempt::new(
            input.workspace_id(),
            operation,
            h.digest,
            Some(h.transaction),
        ),
        first.evidence(),
        first.installation_transaction_id(),
        event,
        Sha256::digest(raw.as_bytes()).into(),
        InventoryCheckpointRef::from_host_metadata(input.authority(), h.revision, Some(h.head))?,
    )?
    .with_rewrap_admission(&admission.report)?;
    let source = StockEnvelopeSource::from_retained_wire(receipt, raw)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let retained = Retained {
        installation: codec::read_original_entry(
            connection,
            input,
            crypto,
            binding,
            admission.original_entry,
        )
        .await?
        .installation_entry(),
        projection: Projection::Rewrap {
            operation,
            recipient: receipt.recipient(),
            admission: admission.frame.header.operation,
            event,
        },
        frame,
        source,
    };
    retained.confirm(connection).await?;
    Ok(retained)
}
pub(in crate::workspace) async fn page(
    turn: &mut Turn<'_>,
    request: StockEnvelopeRequest,
    after: Option<Sequence>,
    proof: AttachmentProof,
) -> Result<StockRewrapPage, OperationError> {
    let (entries, more, checkpoint) = query(turn, &request, after, None, &proof).await?;
    StockRewrapPage::from_store_report(&request, entries, more, checkpoint)
}
pub(in crate::workspace) async fn attempt(
    turn: &mut Turn<'_>,
    request: StockRewrapRequest,
    proof: AttachmentProof,
) -> Result<Option<StockRewrapState>, OperationError> {
    let (mut entries, _, _) = query(
        turn,
        request.source(),
        None,
        Some(request.rewrap_id()),
        &proof,
    )
    .await?;
    Ok(entries.pop())
}
async fn state(
    turn: &mut Turn<'_>,
    binding: &WorkspaceBinding,
    admission: rewrap_admission::Admitted,
) -> Result<StockRewrapState, OperationError> {
    let entry: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT CASE WHEN length(entry_id)=32 THEN entry_id END FROM stock_rewraps WHERE actor=? AND admission_entry=?",
    )
    .bind(turn.selected.participant().as_bytes().as_slice())
    .bind(admission.frame.header.operation.as_bytes().as_slice())
    .fetch_optional(&mut *turn.connection)
    .await
    .map_err(|_| OperationError::RecoveryRequired)?;
    let captured = if let Some(entry) = entry {
        let retained = Box::pin(read_entry(
            turn.connection,
            turn.selected,
            turn.crypto,
            binding,
            OperationId::from_bytes(
                entry
                    .try_into()
                    .map_err(|_| OperationError::RecoveryRequired)?,
            ),
        ))
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
        if retained.frame.header.family != ledger::Family::RewrappedEnvelope {
            return Err(OperationError::RecoveryRequired);
        }
        Some(retained.source.receipt())
    } else {
        None
    };
    StockRewrapState::from_store_report(admission.report, captured)
}
async fn query(
    turn: &mut Turn<'_>,
    request: &StockEnvelopeRequest,
    after: Option<Sequence>,
    id: Option<OperationId>,
    proof: &AttachmentProof,
) -> Result<(Vec<StockRewrapState>, bool, InventoryCheckpointRef), OperationError> {
    if *turn.uncertain {
        return Err(OperationError::RecoveryRequired);
    }
    if request.envelope_event_id().is_some() {
        return Err(OperationError::InvalidCommand);
    }
    let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *turn.connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
    if catalog != 18 {
        return Err(OperationError::UnsupportedOperation);
    }
    let auth = Box::pin(super::super::signing_plan::load(
        turn,
        request.binding(),
        request.minimum_checkpoint(),
        request.operation_id(),
        request.command_digest(),
        proof,
    ))
    .await?;
    if auth.access.authority.catalog != 18
        || !auth
            .original
            .receipt
            .rumor()
            .recipients()
            .contains(&request.recipient())
    {
        return Err(OperationError::InvalidEvidence);
    }
    let current = auth
        .access
        .authority
        .inventory
        .ok_or(OperationError::RecoveryRequired)?;
    if after.is_some_and(|cursor| cursor > current.revision) {
        return Err(OperationError::StaleState);
    }
    let witness = Box::pin(full_proof(turn, request, &auth.access.authority)).await?;
    let selection = SelectionWitness::read(turn.connection, request).await?;
    let rows = if let Some(id) = id {
        sqlx::query("SELECT CASE WHEN length(entry_id)=32 THEN entry_id END FROM stock_rewrap_admissions WHERE actor=? AND operation_id=? AND recipient=? AND request_id=?")
            .bind(turn.selected.participant().as_bytes().as_slice()).bind(request.operation_id().as_bytes().as_slice()).bind(request.recipient().as_bytes().as_slice())
            .bind(id.as_bytes().as_slice()).fetch_all(&mut *turn.connection).await
    } else {
        sqlx::query("SELECT CASE WHEN length(r.entry_id)=32 THEN r.entry_id END FROM stock_rewrap_admissions r JOIN stock_operations o ON o.actor=r.actor AND o.operation_id=r.entry_id WHERE r.actor=? AND r.operation_id=? AND r.recipient=? AND o.revision>? ORDER BY o.revision LIMIT 17")
            .bind(turn.selected.participant().as_bytes().as_slice()).bind(request.operation_id().as_bytes().as_slice()).bind(request.recipient().as_bytes().as_slice())
            .bind(after.map(|value| value.get() as i64).unwrap_or(0)).fetch_all(&mut *turn.connection).await
    }.map_err(|_| OperationError::RecoveryRequired)?;
    let more = rows.len() > 16;
    let mut entries = Vec::with_capacity(rows.len().min(16));
    for row in rows.into_iter().take(16) {
        let entry: Vec<u8> = row
            .try_get(0)
            .map_err(|_| OperationError::RecoveryRequired)?;
        let admission = Box::pin(rewrap_admission::read_entry(
            turn.connection,
            turn.selected,
            turn.crypto,
            request.binding(),
            OperationId::from_bytes(
                entry
                    .try_into()
                    .map_err(|_| OperationError::RecoveryRequired)?,
            ),
        ))
        .await?;
        let first = admission.report.original();
        if first.recipient() != request.recipient()
            || first.attempt().operation_id() != request.operation_id()
            || first.attempt().command_digest() != request.command_digest()
        {
            return Err(OperationError::RecoveryRequired);
        }
        entries.push(Box::pin(state(turn, request.binding(), admission)).await?);
    }
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let checked: Result<(), OperationError> = async {
        auth.confirm(&mut tx, turn.namespace, turn.clock, proof, false)
            .await?;
        inventory::check_head(&mut tx, current).await?;
        witness.confirm(&mut tx).await?;
        selection.confirm(&mut tx, request).await?;
        turn.namespace.validate()?;
        Ok(())
    }
    .await;
    if let Err(error) = checked {
        if tx.rollback().await.is_err() {
            *turn.uncertain = true;
        }
        return Err(error);
    }
    if tx.rollback().await.is_err() {
        *turn.uncertain = true;
        return Err(OperationError::RecoveryRequired);
    }
    Ok((
        entries,
        more,
        InventoryCheckpointRef::from_host_metadata(
            turn.selected.authority(),
            current.revision,
            current.head,
        )?
        .with_optional_authorization(auth.access.authority.authorization),
    ))
}

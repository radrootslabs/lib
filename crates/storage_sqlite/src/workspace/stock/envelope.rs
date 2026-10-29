//! Obtained original outers on the stock owner. Capture grants no transport right.
use super::*;
use inventory::{Turn, checkpoint};
use radroots_domain::{CheckpointDigest, EnvelopeEventId, OperationId, PublicKey};
use radroots_storage::{
    command::{CommitAttempt, OperationError, PrivateEnvelopeCandidate},
    inventory::{
        InventoryCheckpointRef, StockEnvelopeOutcome, StockEnvelopeReceipt, StockEnvelopeRequest,
        StockEnvelopeSource,
    },
};
use sha2::{Digest, Sha256};
use sqlx::Row;
use zeroize::Zeroizing;
mod codec;
mod projection;
mod rewrap;
mod rewrap_admission;
use codec::Original;
pub(in crate::workspace::stock) use codec::Retained;
pub(super) use codec::read_entry;
pub(super) use projection::{
    Projection, raw_admission_projection, raw_projection, raw_rewrap_projection,
};
pub(in crate::workspace) use rewrap_admission::prepare as prepare_rewrap;
pub(in crate::workspace::stock) async fn admission_history(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    entry: OperationId,
) -> Result<ledger::HistoryEntry, OperationError> {
    Ok(Box::pin(rewrap_admission::read_entry(
        connection, input, crypto, binding, entry,
    ))
    .await?
    .into_history())
}
pub(in crate::workspace) use rewrap::{attempt as rewrap_attempt, page};
pub(in crate::workspace) use rewrap_admission::reconcile as reconcile_rewrap_admission;
pub(in crate::workspace) enum CaptureRequest {
    Original(Box<StockEnvelopeRequest>),
    Rewrap(Box<radroots_storage::inventory::StockRewrapRequest>),
}
impl CaptureRequest {
    pub(in crate::workspace) fn source(&self) -> &StockEnvelopeRequest {
        match self {
            Self::Original(request) => request,
            Self::Rewrap(request) => request.source(),
        }
    }
}
pub(in crate::workspace) async fn capture_obtained(
    turn: &mut Turn<'_>,
    request: CaptureRequest,
    proof: AttachmentProof,
    candidate: PrivateEnvelopeCandidate,
) -> Result<StockEnvelopeOutcome, OperationError> {
    match request {
        CaptureRequest::Original(request) => {
            Box::pin(capture(turn, *request, proof, candidate)).await
        }
        CaptureRequest::Rewrap(request) => {
            Box::pin(rewrap::capture(turn, *request, proof, candidate)).await
        }
    }
}

async fn selected_entry(
    connection: &mut SqliteConnection,
    actor: PublicKey,
    operation: OperationId,
    recipient: PublicKey,
) -> Result<Option<OperationId>, OperationError> {
    let row: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT CASE WHEN length(entry_id)=32 THEN entry_id END FROM stock_envelopes WHERE actor=? AND operation_id=? AND recipient=?",
    )
    .bind(actor.as_bytes().as_slice())
    .bind(operation.as_bytes().as_slice())
    .bind(recipient.as_bytes().as_slice())
    .fetch_optional(connection)
    .await
    .map_err(|_| OperationError::RecoveryRequired)?;
    row.map(|bytes| {
        bytes
            .try_into()
            .map(OperationId::from_bytes)
            .map_err(|_| OperationError::RecoveryRequired)
    })
    .transpose()
}
async fn lookup_original(
    turn: &mut Turn<'_>,
    request: &StockEnvelopeRequest,
) -> Result<Option<Retained>, OperationError> {
    let Some(entry) = selected_entry(
        turn.connection,
        turn.selected.participant(),
        request.operation_id(),
        request.recipient(),
    )
    .await?
    else {
        return Ok(None);
    };
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
    if receipt.attempt().operation_id() != request.operation_id()
        || receipt.attempt().command_digest() != request.command_digest()
        || receipt.recipient() != request.recipient()
    {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(Some(retained))
}
pub(in crate::workspace::stock) async fn lookup(
    turn: &mut Turn<'_>,
    request: &StockEnvelopeRequest,
) -> Result<Option<Retained>, OperationError> {
    let original = Box::pin(lookup_original(turn, request)).await?;
    let Some(event) = request.envelope_event_id() else {
        return Ok(original);
    };
    if original
        .as_ref()
        .is_some_and(|value| value.source.receipt().event_id() == event)
    {
        return Ok(original);
    }
    Box::pin(rewrap::lookup(turn, request, event)).await
}
pub(in crate::workspace::stock) async fn full_proof(
    turn: &mut Turn<'_>,
    request: &StockEnvelopeRequest,
    authority: &Authority,
) -> Result<checkpoint::MinimumProof, OperationError> {
    let requested = Box::pin(checkpoint::authenticate_minimum(
        turn.connection,
        turn.selected,
        turn.crypto,
        request.binding(),
        authority,
        request.minimum_checkpoint(),
    ))
    .await?;
    if request.minimum_checkpoint().revision().get() == 0 {
        return Ok(requested);
    }
    let zero = InventoryCheckpointRef::from_host_metadata(
        turn.selected.authority(),
        Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?,
        None,
    )?;
    Box::pin(checkpoint::authenticate_minimum(
        turn.connection,
        turn.selected,
        turn.crypto,
        request.binding(),
        authority,
        zero,
    ))
    .await
}

pub(in crate::workspace) async fn capture(
    turn: &mut Turn<'_>,
    request: StockEnvelopeRequest,
    proof: AttachmentProof,
    candidate: PrivateEnvelopeCandidate,
) -> Result<StockEnvelopeOutcome, OperationError> {
    if request.envelope_event_id().is_some() {
        return Err(OperationError::InvalidCommand);
    }
    if *turn.uncertain {
        return Err(OperationError::RecoveryRequired);
    }
    turn.namespace.validate()?;
    let input = request.binding().input();
    if !SellerAuthoritySelection::from_attachment(turn.selected).matches_attachment(input) {
        return Err(OperationError::Unauthorized);
    }
    let (authority, record, root, grant) = super::read(turn.connection, input, turn.crypto).await?;
    if !matches!(authority.catalog, 16..=18) {
        return Err(OperationError::UnsupportedOperation);
    }
    record.authenticate_retained_capture(input, &proof)?;
    let witness = Box::pin(full_proof(turn, &request, &authority)).await?;
    let original = Box::pin(Original::load(turn, &request)).await?;
    if original.evidence.frame.header.actor != request.binding().actor()
        || candidate.recipient() != request.recipient()
        || original
            .installation
            .receipt
            .publication_work()
            .iter()
            .any(|work| work.recipient() == candidate.author())
    {
        return Err(OperationError::InvalidEvidence);
    }
    candidate.with_wrapping_key(|key| {
        original.evidence.value.with_json(|seal| {
            super::super::delivery::containment::verify(
                candidate.raw_json(),
                candidate.author(),
                key,
                request.recipient(),
                seal,
                original.evidence.value.receipt().digest(),
            )
        })
    })?;
    // The complete protected chain proves natural index absence as well as
    // presence. A missing projection is recovery evidence, never recapture.
    let existing = Box::pin(lookup(turn, &request)).await?;
    let before = authority
        .inventory
        .ok_or(OperationError::RecoveryRequired)?;
    let (frame, updated, receipt) = if let Some(old) = &existing {
        if old.source.with_json(|raw| raw != candidate.raw_json())
            || old.source.receipt().evidence() != original.evidence.value.receipt()
            || old.source.receipt().installation_transaction_id()
                != original
                    .installation
                    .receipt
                    .attempt()
                    .transaction_id()
                    .ok_or(OperationError::RecoveryRequired)?
        {
            return Err(OperationError::IdempotencyConflict);
        }
        (None, None, old.source.receipt())
    } else {
        let revision = Sequence::new(
            before
                .revision
                .get()
                .checked_add(1)
                .ok_or(WorkspaceError::Capacity)?,
        )
        .map_err(|_| WorkspaceError::Capacity)?;
        let body = codec::encode(&original, &request, &candidate);
        let mut header = ledger::Header {
            version: 5,
            family: ledger::Family::Envelope,
            namespace: SellerAuthoritySelection::from_attachment(turn.selected),
            actor: original.evidence.frame.header.actor,
            operation: OperationId::from_bytes(super::super::random()?),
            digest: request.command_digest(),
            transaction: TransactionId::from_bytes(super::super::random()?),
            revision,
            previous: before.head,
            head: CheckpointDigest::from_bytes([0; 32]),
            pool: None,
            order: original.installation.frame.header.order,
            related_operation: Some(request.operation_id()),
            subject_grant: None,
            bytes: 0,
            parts: 0,
            body_digest: [0; 32],
        };
        header.head = codec::head(&header, &body);
        let receipt = StockEnvelopeReceipt::from_committed_store(
            &request,
            CommitAttempt::new(
                input.workspace_id(),
                request.operation_id(),
                request.command_digest(),
                Some(header.transaction),
            ),
            original.evidence.value.receipt(),
            original.installation.frame.header.transaction,
            candidate.event_id(),
            *candidate.digest(),
            InventoryCheckpointRef::from_host_metadata(
                input.authority(),
                revision,
                Some(header.head),
            )?,
        )?;
        let frame = ledger::prepare(turn.crypto, turn.selected, header, &body).await?;
        let mut next = authority.clone();
        next.inventory = Some(InventoryCheckpoint {
            revision,
            head: Some(frame.header.head),
            ..before
        });
        let updated = turn
            .crypto
            .seal_stock_root(turn.selected, next.catalog, &next.encode()?)
            .await?;
        (Some(frame), Some(updated), receipt)
    };
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let changed: Result<(), OperationError> = async {
        turn.namespace.validate()?;
        super::unchanged(&mut tx, input, authority.catalog, &root, &grant).await?;
        record.authenticate_retained_capture(input, &proof)?;
        witness.confirm(&mut tx).await?;
        if let Some(old) = &existing {
            old.confirm(&mut tx).await?;
        }
        if let Some(frame) = &frame {
            frame.insert_record(&mut tx).await?;
            Projection::Original {
                operation: request.operation_id(),
                recipient: request.recipient(),
            }
            .insert(&mut tx, input.participant(), frame.header.operation)
            .await?;
            let result = sqlx::query(
                "UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?",
            )
            .bind(updated.as_ref().ok_or(OperationError::RecoveryRequired)?)
            .bind(&root)
            .execute(&mut *tx)
            .await
            .map_err(|_| OperationError::StorageFailure)?;
            if result.rows_affected() != 1 {
                return Err(OperationError::RecoveryRequired);
            }
        }
        turn.namespace.validate()?;
        Ok(())
    }
    .await;
    if let Err(error) = changed {
        if tx.rollback().await.is_err() {
            *turn.uncertain = true;
            return Ok(StockEnvelopeOutcome::Unknown(receipt.attempt()));
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
        return Ok(StockEnvelopeOutcome::Unknown(receipt.attempt()));
    }
    Ok(StockEnvelopeOutcome::Committed(Box::new(receipt)))
}

pub(in crate::workspace) async fn read(
    turn: &mut Turn<'_>,
    request: StockEnvelopeRequest,
    proof: AttachmentProof,
) -> Result<Option<StockEnvelopeSource>, OperationError> {
    if *turn.uncertain {
        return Err(OperationError::RecoveryRequired);
    }
    turn.namespace.validate()?;
    let input = request.binding().input();
    let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *turn.connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
    if !matches!(catalog, 16..=18) {
        return Err(OperationError::UnsupportedOperation);
    }
    let auth = Box::pin(super::signing_plan::load(
        turn,
        request.binding(),
        request.minimum_checkpoint(),
        request.operation_id(),
        request.command_digest(),
        &proof,
    ))
    .await?;
    if !matches!(auth.access.authority.catalog, 16..=18) {
        return Err(OperationError::UnsupportedOperation);
    }
    let witness = Box::pin(full_proof(turn, &request, &auth.access.authority)).await?;
    let retained = Box::pin(lookup(turn, &request)).await?;
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let checked: Result<(), OperationError> = async {
        turn.namespace.validate()?;
        auth.confirm(&mut tx, turn.namespace, turn.clock, &proof, false)
            .await?;
        witness.confirm(&mut tx).await?;
        if let Some(retained) = &retained {
            retained.confirm(&mut tx).await?;
        } else {
            let count: i64 = if let Some(event) = request.envelope_event_id() {
                sqlx::query_scalar("SELECT count(*) FROM stock_rewraps WHERE actor=? AND operation_id=? AND recipient=? AND event_id=?")
                    .bind(input.participant().as_bytes().as_slice())
                    .bind(request.operation_id().as_bytes().as_slice()).bind(request.recipient().as_bytes().as_slice())
                    .bind(event.as_bytes().as_slice()).fetch_one(&mut *tx).await.map_err(|_| OperationError::RecoveryRequired)?
            } else {
                sqlx::query_scalar("SELECT count(*) FROM stock_envelopes WHERE actor=? AND operation_id=? AND recipient=?")
                    .bind(input.participant().as_bytes().as_slice())
                    .bind(request.operation_id().as_bytes().as_slice()).bind(request.recipient().as_bytes().as_slice())
                    .fetch_one(&mut *tx).await.map_err(|_| OperationError::RecoveryRequired)?
            };
            if count != 0 { return Err(OperationError::RecoveryRequired); }
        }
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
    Ok(retained.map(|value| value.source))
}

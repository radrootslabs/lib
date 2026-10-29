//! Confirm original obtained rewrap bytes without provider IO inside SQL.
use super::*;

pub(in crate::workspace::stock::envelope) async fn capture(
    turn: &mut Turn<'_>,
    request: StockRewrapRequest,
    proof: AttachmentProof,
    candidate: PrivateEnvelopeCandidate,
) -> Result<StockEnvelopeOutcome, OperationError> {
    if *turn.uncertain {
        return Err(OperationError::RecoveryRequired);
    }
    turn.namespace.validate()?;
    let rewrap = request;
    let request = rewrap.source();
    let event = request
        .envelope_event_id()
        .ok_or(OperationError::InvalidCommand)?;
    if event != candidate.event_id() || candidate.recipient() != request.recipient() {
        return Err(OperationError::InvalidEvidence);
    }
    let input = request.binding().input();
    if !SellerAuthoritySelection::from_attachment(turn.selected).matches_attachment(input) {
        return Err(OperationError::Unauthorized);
    }
    let (authority, record, root, grant) =
        super::super::super::read(turn.connection, input, turn.crypto).await?;
    if authority.catalog != 18 {
        return Err(OperationError::UnsupportedOperation);
    }
    record.authenticate_retained_capture(input, &proof)?;
    let witness = Box::pin(full_proof(turn, request, &authority)).await?;
    let selection = SelectionWitness::read(turn.connection, request).await?;
    let admission = Box::pin(rewrap_admission::lookup(turn, &rewrap))
        .await?
        .ok_or(OperationError::MissingHistory)?;
    if admission.frame.header.actor != request.binding().actor() {
        return Err(OperationError::Unauthorized);
    }
    let first = Box::pin(lookup_original(turn, request))
        .await?
        .ok_or(OperationError::MissingHistory)?;
    if first.source.receipt() != admission.report.original()
        || first.source.receipt().event_id() == event
    {
        return Err(OperationError::InvalidEvidence);
    }
    let original = Box::pin(Original::load(turn, request)).await?;
    if original.evidence.frame.header.actor.participant() != request.binding().actor().participant()
        || first.source.receipt().evidence() != original.evidence.value.receipt()
        || first.source.receipt().installation_transaction_id()
            != original.installation.frame.header.transaction
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
            super::super::super::super::delivery::containment::verify(
                candidate.raw_json(),
                candidate.author(),
                key,
                request.recipient(),
                seal,
                original.evidence.value.receipt().digest(),
            )
        })
    })?;
    let existing = Box::pin(lookup(turn, request, event)).await?;
    let captured: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT CASE WHEN length(event_id)=32 THEN event_id END FROM stock_rewraps WHERE actor=? AND admission_entry=?",
    )
    .bind(input.participant().as_bytes().as_slice())
    .bind(admission.frame.header.operation.as_bytes().as_slice())
    .fetch_optional(&mut *turn.connection)
    .await
    .map_err(|_| OperationError::RecoveryRequired)?;
    if captured
        .as_deref()
        .is_some_and(|old| old != event.as_bytes().as_slice())
    {
        return Err(OperationError::IdempotencyConflict);
    }
    let before = authority
        .inventory
        .ok_or(OperationError::RecoveryRequired)?;
    let (frame, updated, receipt) = if let Some(old) = &existing {
        if old.source.with_json(|raw| raw != candidate.raw_json())
            || old.source.receipt().evidence() != first.source.receipt().evidence()
            || old.source.receipt().rewrap_admission_transaction_id()
                != Some(admission.report.transaction_id())
            || old.source.receipt().installation_transaction_id()
                != first.source.receipt().installation_transaction_id()
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
        let body = encode(&admission, request, candidate.raw_json());
        let mut header = ledger::Header {
            version: 5,
            family: ledger::Family::RewrappedEnvelope,
            namespace: SellerAuthoritySelection::from_attachment(turn.selected),
            actor: request.binding().actor(),
            operation: OperationId::from_bytes(super::super::super::super::random()?),
            digest: request.command_digest(),
            transaction: TransactionId::from_bytes(super::super::super::super::random()?),
            revision,
            previous: before.head,
            head: CheckpointDigest::from_bytes([0; 32]),
            pool: None,
            order: first.frame.header.order,
            related_operation: Some(request.operation_id()),
            subject_grant: None,
            bytes: 0,
            parts: 0,
            body_digest: [0; 32],
        };
        header.head = head(&header, &body);
        let receipt = StockEnvelopeReceipt::from_committed_store(
            request,
            CommitAttempt::new(
                input.workspace_id(),
                request.operation_id(),
                request.command_digest(),
                Some(header.transaction),
            ),
            first.source.receipt().evidence(),
            first.source.receipt().installation_transaction_id(),
            event,
            *candidate.digest(),
            InventoryCheckpointRef::from_host_metadata(
                input.authority(),
                revision,
                Some(header.head),
            )?,
        )?;
        let receipt = receipt.with_rewrap_admission(&admission.report)?;
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
        super::super::super::unchanged(&mut tx, input, authority.catalog, &root, &grant).await?;
        record.authenticate_retained_capture(input, &proof)?;
        witness.confirm(&mut tx).await?;
        selection.confirm(&mut tx, request).await?;
        first.confirm(&mut tx).await?;
        admission.confirm(&mut tx).await?;
        if let Some(old) = &existing {
            old.confirm(&mut tx).await?;
        }
        if let Some(frame) = &frame {
            frame.insert_record(&mut tx).await?;
            Projection::Rewrap {
                operation: request.operation_id(),
                recipient: request.recipient(),
                admission: admission.frame.header.operation,
                event,
            }
            .insert(&mut tx, input.participant(), frame.header.operation)
            .await?;
            let changed = sqlx::query(
                "UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?",
            )
            .bind(updated.as_ref().ok_or(OperationError::RecoveryRequired)?)
            .bind(&root)
            .execute(&mut *tx)
            .await
            .map_err(|_| OperationError::StorageFailure)?;
            if changed.rows_affected() != 1 {
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

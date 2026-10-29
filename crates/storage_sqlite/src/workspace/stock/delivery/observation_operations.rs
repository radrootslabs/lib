//! Capture obtained original outcomes, including after clock loss or revocation.
use super::*;
use graph::{Graph, SelectObservation};

pub(in crate::workspace) async fn capture_observation(
    turn: &mut Turn<'_>,
    binding: WorkspaceBinding,
    proof: AttachmentProof,
    supplied: StockDeliveryObservation,
) -> Result<StockDeliveryObservationOutcome, OperationError> {
    if *turn.uncertain {
        return Err(OperationError::RecoveryRequired);
    }
    turn.namespace.validate()?;
    let input = binding.input();
    if !SellerAuthoritySelection::from_attachment(turn.selected).matches_attachment(input) {
        return Err(OperationError::Unauthorized);
    }
    let (authority, credential, root, grant) =
        super::super::read(turn.connection, input, turn.crypto).await?;
    if !matches!(authority.catalog, 17 | 18) {
        return Err(OperationError::UnsupportedOperation);
    }
    credential.authenticate_retained_capture(input, &proof)?;
    let original = supplied.source().attempt();
    let source = StockEnvelopeRequest::new(
        binding.clone(),
        InventoryCheckpointRef::from_host_metadata(
            input.authority(),
            Sequence::new(0).map_err(|_| OperationError::RecoveryRequired)?,
            None,
        )?,
        original.operation_id(),
        original.command_digest(),
        supplied.source().recipient(),
        None,
    )?;
    let source = if authority.catalog >= 18 {
        source.with_envelope(supplied.source().event_id())
    } else {
        source
    };
    let witness = Box::pin(envelope::full_proof(turn, &source, &authority)).await?;
    let mut graph = Box::pin(Graph::read(
        turn,
        &source,
        &authority,
        Some(supplied.request_id()),
        Some(SelectObservation {
            request: supplied.request_id(),
            target: supplied.target_receipt().target().fingerprint(),
            kind: supplied.kind(),
        }),
    ))
    .await?;
    let attempt = graph
        .found_attempt
        .as_ref()
        .ok_or(OperationError::InvalidEvidence)?;
    if attempt.frame.header.actor != binding.actor()
        || attempt.frame.header.actor.grant_id() != Some(input.grant_id())
    {
        return Err(OperationError::Unauthorized);
    }
    let checked = StockDeliveryObservation::from_store_report(
        &attempt.report,
        supplied.target_receipt(),
        supplied.observed_at_unix_ms(),
    )?;
    if checked != supplied {
        return Err(OperationError::InvalidEvidence);
    }
    let prepared = if graph.found_observation.is_none() {
        let body = wire::encode(&observation::Wire::new(attempt, &supplied), OBSERVATION_MAX)?;
        let projection = Projection::Observation {
            operation: source.operation_id(),
            recipient: source.recipient(),
            request: request_key(supplied.request_id()),
            target: target_key(supplied.target_receipt().target().fingerprint()),
            kind: supplied.kind().code(),
        };
        let prepared = Box::pin(Prepared::new(
            turn,
            &authority,
            Append {
                actor: attempt.frame.header.actor,
                original: source.operation_id(),
                digest: source.command_digest(),
                order: attempt
                    .frame
                    .header
                    .order
                    .ok_or(OperationError::RecoveryRequired)?,
                family: ledger::Family::DeliveryObservation,
                projection,
            },
            &body,
        ))
        .await?;
        let receipt = StockDeliveryObservationReceipt::from_store_report(
            supplied,
            prepared.frame.header.transaction,
            prepared.checkpoint(&authority)?,
        )?;
        Some((prepared, receipt))
    } else {
        None
    };
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let changed: Result<(), OperationError> = async {
        turn.namespace.validate()?;
        super::super::unchanged(&mut tx, input, authority.catalog, &root, &grant).await?;
        credential.authenticate_retained_capture(input, &proof)?;
        inventory::check_head(
            &mut tx,
            authority
                .inventory
                .ok_or(OperationError::RecoveryRequired)?,
        )
        .await?;
        witness.confirm(&mut tx).await?;
        attempt.confirm(&mut tx).await?;
        if let Some(old) = &graph.found_observation {
            old.confirm(&mut tx).await?;
        }
        if let Some((prepared, _)) = &prepared {
            prepared.insert(&mut tx, &root).await?;
        }
        turn.namespace.validate()?;
        Ok(())
    }
    .await;
    let unknown = || {
        StockDeliveryObservationOutcome::Unknown(radroots_storage::command::CommitAttempt::new(
            input.workspace_id(),
            source.operation_id(),
            source.command_digest(),
            prepared
                .as_ref()
                .map(|(prepared, _)| prepared.frame.header.transaction),
        ))
    };
    if let Err(error) = changed {
        if tx.rollback().await.is_err() {
            *turn.uncertain = true;
            return Ok(unknown());
        }
        return Err(error);
    }
    let completed = if prepared.is_some() {
        tx.commit().await
    } else {
        tx.rollback().await
    };
    if completed.is_err() {
        *turn.uncertain = true;
        return Ok(unknown());
    }
    match prepared {
        Some((_, receipt)) => Ok(StockDeliveryObservationOutcome::Committed(Box::new(
            receipt,
        ))),
        None => Ok(StockDeliveryObservationOutcome::Committed(Box::new(
            graph
                .found_observation
                .take()
                .ok_or(OperationError::RecoveryRequired)?
                .receipt,
        ))),
    }
}

pub(in crate::workspace) async fn page(
    turn: &mut Turn<'_>,
    source: StockEnvelopeRequest,
    after: Option<Sequence>,
    proof: AttachmentProof,
) -> Result<StockDeliveryObservationPage, OperationError> {
    let (auth, witness, graph) = Box::pin(operations::read(turn, &source, &proof, None)).await?;
    let selected_event = graph
        .source(source.recipient())
        .ok()
        .map(|value| value.source.receipt().event_id());
    let current = auth
        .access
        .authority
        .inventory
        .ok_or(OperationError::RecoveryRequired)?;
    if after.is_some_and(|after| after > current.revision) {
        return Err(WorkspaceError::StaleCheckpoint.into());
    }
    let rows = sqlx::query("SELECT d.entry_id FROM stock_delivery_observations d JOIN stock_operations o ON o.actor=d.actor AND o.operation_id=d.entry_id WHERE d.actor=? AND d.operation_id=? AND d.recipient=? AND o.revision>? ORDER BY o.revision LIMIT 2305")
        .bind(turn.selected.participant().as_bytes().as_slice()).bind(source.operation_id().as_bytes().as_slice())
        .bind(source.recipient().as_bytes().as_slice()).bind(after.map_or(0, |value| value.get()) as i64)
        .fetch_all(&mut *turn.connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    if rows.len() > OBSERVATION_COUNT_MAX {
        return Err(OperationError::RecoveryRequired);
    }
    // The protected history is globally bounded and already authenticated.
    // Filter by the actual attempt source, never an unauthenticated event index.
    // Retain at most one extra matching descriptor to establish continuation.
    let mut entries = Vec::with_capacity(PAGE_MAX + 1);
    for row in rows {
        let bytes: Vec<u8> = row
            .try_get(0)
            .map_err(|_| OperationError::RecoveryRequired)?;
        let entry = OperationId::from_bytes(
            bytes
                .try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        );
        let value = Box::pin(observation::read_entry(
            turn.connection,
            turn.selected,
            turn.crypto,
            source.binding(),
            auth.access.authority.input.grant_id(),
            entry,
        ))
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
        if selected_event == Some(value.receipt.observation().source().event_id()) {
            entries.push(value.receipt);
            if entries.len() > PAGE_MAX {
                break;
            }
        }
    }
    let more = entries.len() > PAGE_MAX;
    entries.truncate(PAGE_MAX);
    Box::pin(operations::confirm_read(turn, &auth, &witness, &proof)).await?;
    StockDeliveryObservationPage::from_store_report(
        &source,
        entries,
        more,
        InventoryCheckpointRef::from_host_metadata(
            turn.selected.authority(),
            current.revision,
            current.head,
        )?
        .with_optional_authorization(auth.access.authority.authorization),
    )
}

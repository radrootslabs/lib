//! Concrete D admission and current-authorized reads on the stock connection.
use super::*;
use graph::Graph;

async fn authorized(
    turn: &mut Turn<'_>,
    source: &StockEnvelopeRequest,
    proof: &AttachmentProof,
) -> Result<signing_plan::Authorized, OperationError> {
    let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *turn.connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
    if !matches!(catalog, 17..=19) {
        return Err(OperationError::UnsupportedOperation);
    }
    let loading = signing_plan::load(
        turn,
        source.binding(),
        source.minimum_checkpoint(),
        source.operation_id(),
        source.command_digest(),
        proof,
    );
    let auth = Box::pin(loading).await?;
    if !matches!(auth.access.authority.catalog, 17..=19) {
        return Err(OperationError::UnsupportedOperation);
    }
    if !auth
        .original
        .receipt
        .rumor()
        .recipients()
        .contains(&source.recipient())
    {
        return Err(OperationError::InvalidEvidence);
    }
    Ok(auth)
}
pub(in crate::workspace) async fn prepare(
    turn: &mut Turn<'_>,
    request: StockDeliveryRequest,
    proof: AttachmentProof,
) -> Result<StockDeliveryAttemptOutcome, OperationError> {
    if *turn.uncertain {
        return Err(OperationError::RecoveryRequired);
    }
    let source = request.source();
    validate_request(request.intent().request_id())?;
    let auth = Box::pin(authorized(turn, source, &proof)).await?;
    Box::pin(prepare_authorized(turn, request, proof, auth)).await
}
// Authorization has its own polling frame, as in the stock snapshot reader.
async fn prepare_authorized(
    turn: &mut Turn<'_>,
    request: StockDeliveryRequest,
    proof: AttachmentProof,
    auth: signing_plan::Authorized,
) -> Result<StockDeliveryAttemptOutcome, OperationError> {
    let source = request.source();
    let witness = Box::pin(envelope::full_proof(turn, source, &auth.access.authority)).await?;
    let mut graph = Box::pin(Graph::read(
        turn,
        source,
        &auth.access.authority,
        Some(request.intent().request_id()),
        None,
    ))
    .await?;

    let envelope = graph.source(source.recipient())?;
    if request.intent().event_id() != envelope.source.receipt().event_id() {
        return Err(OperationError::InvalidEvidence);
    }
    if graph
        .found_attempt
        .as_ref()
        .is_some_and(|old| old.report.intent().digest() != request.intent().digest())
    {
        return Err(OperationError::IdempotencyConflict);
    }
    let fresh = graph.found_attempt.is_none();
    if fresh
        && request
            .expected_attempt_count()
            .is_some_and(|expected| graph.attempt_count(source.recipient()) != Ok(expected))
    {
        return Err(OperationError::StaleState);
    }
    let now = auth.access.current.observed_at();
    let prepared = if fresh {
        let installed = Box::pin(inventory::installation::read_entry(
            turn.connection,
            turn.selected,
            turn.crypto,
            envelope.installation_entry(),
        ))
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
        if now < installed.receipt.installed_at() {
            return Err(WorkspaceError::Unavailable.into());
        }
        if graph.last_at.is_some_and(|at| now < at) || now < auth.consent.observed_at {
            return Err(WorkspaceError::Unavailable.into());
        }
        if now
            .get()
            .checked_mul(1000)
            .is_none_or(|at| at >= request.intent().deadline_unix_ms().get())
        {
            return Err(OperationError::DeadlineExceeded);
        }
        let ordinals = graph.ordinals(source.recipient(), request.intent())?;
        let body = wire::encode(
            &attempt::Wire::new(envelope, &auth, request.intent(), now, ordinals.clone())?,
            ATTEMPT_MAX,
        )?;
        let projection = Projection::Attempt {
            operation: source.operation_id(),
            recipient: source.recipient(),
            request: request_key(request.intent().request_id()),
        };
        let prepared = Box::pin(Prepared::new(
            turn,
            &auth.access.authority,
            Append {
                actor: auth.access.current.actor(),
                original: source.operation_id(),
                digest: source.command_digest(),
                order: auth.original.receipt.order_id(),
                family: ledger::Family::DeliveryAttempt,
                projection,
            },
            &body,
        ))
        .await?;

        let copied = envelope.source.with_json(|raw| {
            radroots_storage::inventory::StockEnvelopeSource::from_retained_wire(
                envelope.source.receipt(),
                raw,
            )
        })?;

        let report = StockDeliveryAttempt::from_store_report(
            Arc::new(copied),
            request.intent().clone(),
            prepared.frame.header.transaction,
            now,
            prepared.checkpoint(&auth.access.authority)?,
            ordinals,
        )?;

        Some((prepared, report))
    } else {
        None
    };
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let changed: Result<(), OperationError> = async {
        let current = auth
            .confirm(&mut tx, turn.namespace, turn.clock, &proof, false)
            .await?;
        inventory::check_head(
            &mut tx,
            auth.access
                .authority
                .inventory
                .ok_or(OperationError::RecoveryRequired)?,
        )
        .await?;
        witness.confirm(&mut tx).await?;
        if let Some(old) = &graph.found_attempt {
            old.confirm(&mut tx).await?;
        }
        if let Some((prepared, _)) = &prepared {
            if current.observed_at() < now {
                return Err(WorkspaceError::Unavailable.into());
            }
            if current
                .observed_at()
                .get()
                .checked_mul(1000)
                .is_none_or(|at| at >= request.intent().deadline_unix_ms().get())
            {
                return Err(OperationError::DeadlineExceeded);
            }
            prepared.insert(&mut tx, &auth.access.root).await?;
        }
        turn.namespace.validate()?;
        Ok(())
    }
    .await;

    let unknown = || StockDeliveryAttemptOutcome::Unknown {
        operation: radroots_storage::command::CommitAttempt::new(
            source.binding().input().workspace_id(),
            source.operation_id(),
            source.command_digest(),
            prepared
                .as_ref()
                .map(|(prepared, _)| prepared.frame.header.transaction),
        ),
        request: request.intent().request_id().clone(),
    };
    if let Err(error) = changed {
        if tx.rollback().await.is_err() {
            *turn.uncertain = true;
            return Ok(unknown());
        }
        return Err(error);
    }
    let completed = if fresh {
        tx.commit().await
    } else {
        tx.rollback().await
    };
    if completed.is_err() {
        *turn.uncertain = true;
        return Ok(unknown());
    }
    match prepared {
        Some((_, report)) => Ok(StockDeliveryAttemptOutcome::Claimed(Box::new(report))),
        None => Ok(StockDeliveryAttemptOutcome::Retained(Box::new(
            graph
                .found_attempt
                .take()
                .ok_or(OperationError::RecoveryRequired)?
                .report,
        ))),
    }
}
pub(in crate::workspace) async fn summary(
    turn: &mut Turn<'_>,
    source: StockEnvelopeRequest,
    proof: AttachmentProof,
) -> Result<StockDeliverySummary, OperationError> {
    let (auth, witness, graph) = Box::pin(read(turn, &source, &proof, None)).await?;
    Box::pin(confirm_read(turn, &auth, &witness, &proof)).await?;
    let current = auth
        .access
        .authority
        .inventory
        .ok_or(OperationError::RecoveryRequired)?;
    graph.summary(
        &source,
        InventoryCheckpointRef::from_host_metadata(
            turn.selected.authority(),
            current.revision,
            current.head,
        )?
        .with_optional_authorization(auth.access.authority.authorization),
    )
}
pub(in crate::workspace) async fn query_attempt(
    turn: &mut Turn<'_>,
    source: StockEnvelopeRequest,
    request: DeliveryRequestId,
    proof: AttachmentProof,
) -> Result<Option<StockDeliveryAttempt>, OperationError> {
    validate_request(&request)?;
    let (auth, witness, mut graph) = Box::pin(read(turn, &source, &proof, Some(&request))).await?;
    if graph.found_attempt.as_ref().is_some_and(|value| {
        graph.source(source.recipient()).is_ok_and(|selected| {
            selected.source.receipt().event_id() != value.report.source().receipt().event_id()
        })
    }) {
        return Err(OperationError::InvalidEvidence);
    }
    Box::pin(confirm_read(turn, &auth, &witness, &proof)).await?;
    Ok(graph.found_attempt.take().map(|value| value.report))
}
pub(super) async fn read(
    turn: &mut Turn<'_>,
    source: &StockEnvelopeRequest,
    proof: &AttachmentProof,
    request: Option<&DeliveryRequestId>,
) -> Result<(signing_plan::Authorized, checkpoint::MinimumProof, Graph), OperationError> {
    if *turn.uncertain {
        return Err(OperationError::RecoveryRequired);
    }
    let auth = Box::pin(authorized(turn, source, proof)).await?;
    let witness = Box::pin(envelope::full_proof(turn, source, &auth.access.authority)).await?;
    let graph = Box::pin(Graph::read(
        turn,
        source,
        &auth.access.authority,
        request,
        None,
    ))
    .await?;
    Ok((auth, witness, graph))
}
pub(super) async fn confirm_read(
    turn: &mut Turn<'_>,
    auth: &signing_plan::Authorized,
    witness: &checkpoint::MinimumProof,
    proof: &AttachmentProof,
) -> Result<(), OperationError> {
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let checked: Result<(), OperationError> = async {
        auth.confirm(&mut tx, turn.namespace, turn.clock, proof, false)
            .await?;
        inventory::check_head(
            &mut tx,
            auth.access
                .authority
                .inventory
                .ok_or(OperationError::RecoveryRequired)?,
        )
        .await?;
        witness.confirm(&mut tx).await?;
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
    Ok(())
}

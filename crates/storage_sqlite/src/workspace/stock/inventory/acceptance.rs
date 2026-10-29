//! Native T0: retained command, complete preparatory holds and one order fence.
//! This boundary performs no signing, installation, delivery or physical claim.
use super::super::{consent, ledger, source};
use super::*;
use radroots_domain::{CheckpointRef, OrderId, RecordId, TermsDigest};
use radroots_protocol::command::CapturedOrderAcceptance;
use radroots_storage::inventory::InventoryCheckpointRef;
use radroots_storage::workspace::seller_stock::{
    StockListingCheckpointRef, StockSourceCheckpointRef,
};
use std::collections::{BTreeMap, BTreeSet};
pub(in crate::workspace::stock) mod codec;
mod plan;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Body<'a> {
    version: u8,
    #[serde(borrow)]
    command: &'a RawValue,
    #[serde(borrow)]
    terms: &'a RawValue,
    #[serde(borrow)]
    original_request: &'a RawValue,
    #[serde(borrow, deserialize_with = "super::super::required_nullable")]
    original_quote: Option<&'a RawValue>,
    #[serde(deserialize_with = "super::super::required_nullable")]
    seller_author_head: Option<RecordId>,
    seller_author_sequence: Sequence,
    minimum_revision: Sequence,
    #[serde(deserialize_with = "super::super::required_nullable")]
    minimum_head: Option<CheckpointDigest>,
    authorization: StockAuthorizationRef,
    source_minimum: Progress,
    listing_minimum: Progress,
    coverage: CheckpointRef,
    buyer_head: Option<RecordId>,
    observed_at: UnixSeconds,
    expires: UnixSeconds,
    issue_before: UnixSeconds,
    consent: OperationId,
    mappings: Vec<radroots_domain::stock::OptionPoolMapping>,
    pools: Vec<PoolSnapshot>,
    allocations: Vec<plan::Allocation>,
    effects: Vec<plan::Effect>,
    movements: Vec<[u8; 32]>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Progress {
    authority: radroots_domain::AuthorityRef,
    revision: Sequence,
    #[serde(deserialize_with = "super::super::required_nullable")]
    head: Option<CheckpointDigest>,
    authorization: StockAuthorizationRef,
}
impl Progress {
    fn source(value: StockSourceCheckpointRef) -> Result<Self, OperationError> {
        Ok(Self {
            authority: value.authority(),
            revision: value.revision(),
            head: value.head(),
            authorization: value
                .authorization()
                .ok_or(OperationError::RecoveryRequired)?,
        })
    }
    fn listing(value: StockListingCheckpointRef) -> Result<Self, OperationError> {
        Ok(Self {
            authority: value.authority(),
            revision: value.revision(),
            head: value.head(),
            authorization: value
                .authorization()
                .ok_or(OperationError::RecoveryRequired)?,
        })
    }
}
#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Fence {
    version: u8,
    order: OrderId,
    operation: OperationId,
    digest: CommandDigest,
    transaction: TransactionId,
    revision: Sequence,
    request: RecordId,
    terms: TermsDigest,
}
fn fence(
    header: &ledger::Header,
    command: &CapturedOrderAcceptance,
) -> Result<Fence, OperationError> {
    Ok(Fence {
        version: 1,
        order: header.order.ok_or(OperationError::RecoveryRequired)?,
        operation: header.operation,
        digest: header.digest,
        transaction: header.transaction,
        revision: Sequence::new(1).map_err(|_| OperationError::RecoveryRequired)?,
        request: command.input().request(),
        terms: command.input().terms_digest(),
    })
}
fn bytes(value: &impl Serialize) -> Result<zeroize::Zeroizing<Vec<u8>>, OperationError> {
    let bytes = zeroize::Zeroizing::new(
        serde_json::to_vec(value).map_err(|_| OperationError::StorageFailure)?,
    );
    if bytes.is_empty() || bytes.len() > ledger::BODY_MAX_BYTES {
        return Err(WorkspaceError::Capacity.into());
    }
    Ok(bytes)
}
fn head(header: &ledger::Header, body: &[u8]) -> CheckpointDigest {
    let mut h = Sha256::new();
    h.update(b"radroots.seller_stock.acceptance_chain.v1\0");
    h.update(header.namespace.workspace_id().as_bytes());
    h.update(header.namespace.authority().executor_id().as_bytes());
    h.update(
        header
            .namespace
            .authority()
            .generation()
            .get()
            .to_be_bytes(),
    );
    h.update(header.namespace.route_digest().as_bytes());
    h.update([u8::from(header.previous.is_some())]);
    h.update(
        header
            .previous
            .map(|value| *value.as_bytes())
            .unwrap_or([0; 32]),
    );
    h.update(header.revision.get().to_be_bytes());
    h.update(header.actor.participant().as_bytes());
    h.update(
        header
            .actor
            .grant_id()
            .map(|value| *value.as_bytes())
            .unwrap_or([0; 32]),
    );
    h.update(header.operation.as_bytes());
    h.update(header.digest.as_bytes());
    h.update(header.transaction.as_bytes());
    h.update(Sha256::digest(body));
    CheckpointDigest::from_bytes(h.finalize().into())
}
pub(in crate::workspace::stock) fn check_consent(
    access: &Access,
    scope: &consent::Retained,
    command: &CapturedOrderAcceptance,
    fresh: bool,
    at: UnixSeconds,
) -> Result<(), OperationError> {
    if scope.command.digest() != command.digest() {
        return Err(OperationError::IdempotencyConflict);
    }
    if scope.subject != access.current.input().grant_id()
        || scope.authorization
            != access
                .authority
                .authorization
                .ok_or(OperationError::RecoveryRequired)?
        || scope.subject_digest != <[u8; 32]>::from(Sha256::digest(&access.grant))
        || (fresh && scope.mode != consent::StockConsentMode::ExecuteAndRecover)
    {
        return Err(OperationError::Unauthorized);
    }
    if at >= scope.expires {
        return Err(OperationError::DeadlineExceeded);
    }
    if fresh {
        if command.context().actor() != access.current.actor() {
            return Err(OperationError::Unauthorized);
        }
        if at >= command.context().not_after()
            || command
                .context()
                .approval()
                .is_none_or(|approval| at >= approval.expires_at())
        {
            return Err(OperationError::DeadlineExceeded);
        }
    }
    Ok(())
}
pub(in crate::workspace::stock) async fn current_consent(
    turn: &mut Turn<'_>,
    access: &Access,
    command: &CapturedOrderAcceptance,
) -> Result<(consent::Retained, MinimumProof), OperationError> {
    let operation = consent::natural(
        turn.connection,
        turn.selected.participant(),
        command.context().operation_id(),
        access.current.input().grant_id(),
    )
    .await?;
    let retained = if let Some(operation) = operation {
        consent::read(
            turn.connection,
            turn.selected,
            turn.crypto,
            access.authority.input.grant_id(),
            operation,
        )
        .await?
    } else {
        None
    };
    let proof = checkpoint::authenticate_identity(
        turn.connection,
        turn.selected,
        turn.crypto,
        &access.current,
        &access.authority,
        checkpoint::IdentityTarget::Consent {
            operation: command.context().operation_id(),
            subject: access.current.input().grant_id(),
            expected: retained.as_ref().map(consent::Retained::identity),
        },
    )
    .await?;
    Ok((retained.ok_or(OperationError::Unauthorized)?, proof))
}

/// Preserves one original operation across lost observers and store reopen.
pub(in crate::workspace) async fn admit(
    turn: &mut Turn<'_>,
    request: StockAcceptanceRequest,
    proof: AttachmentProof,
) -> Result<StockAcceptanceOutcome, OperationError> {
    let access = turn
        .access(request.binding(), request.minimum_checkpoint(), &proof)
        .await?;
    if !matches!(access.authority.catalog, 9..=19) {
        return Err(OperationError::UnsupportedOperation);
    }
    let command = request.command();
    let operation = command.context().operation_id();
    let original = codec::read(turn.connection, turn.selected, turn.crypto, operation).await?;
    let identity = checkpoint::authenticate_identity(
        turn.connection,
        turn.selected,
        turn.crypto,
        &access.current,
        &access.authority,
        checkpoint::IdentityTarget::Operation(
            operation,
            original.as_ref().map(|value| value.identity()),
        ),
    )
    .await?;
    let (consent, consent_proof) = current_consent(turn, &access, command).await?;
    check_consent(
        &access,
        &consent,
        command,
        original.is_none(),
        access.current.observed_at(),
    )?;
    if let Some(original) = original {
        if original.frame.header.digest != command.digest() {
            return Err(OperationError::IdempotencyConflict);
        }
        for pool in original.receipt.held_pools() {
            if !consent.resources.contains(pool.id()) {
                return Err(OperationError::Unauthorized);
            }
        }
        let aborted = if access.authority.catalog >= 13 {
            Some(Box::pin(super::abort::existing(turn, &access, operation)).await?)
        } else {
            None
        };
        let mut tx = turn
            .connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        let checked: Result<(), OperationError> = async {
            identity.confirm(&mut tx).await?;
            consent_proof.confirm(&mut tx).await?;
            original.confirm(&mut tx).await?;
            if let Some((retained, witness)) = &aborted {
                witness.confirm(&mut tx).await?;
                if let Some(retained) = retained {
                    retained.confirm(&mut tx).await?;
                }
            }
            let current = access
                .confirm(&mut tx, turn.namespace, turn.clock, &proof)
                .await?;
            check_consent(&access, &consent, command, false, current.observed_at())?;
            Ok(())
        }
        .await;
        if tx.rollback().await.is_err() {
            *turn.uncertain = true;
            return Err(OperationError::RecoveryRequired);
        }
        checked?;
        if let Some((Some(aborted), _)) = aborted {
            return Ok(StockAcceptanceOutcome::Aborted(Box::new(aborted.receipt)));
        }
        return Ok(StockAcceptanceOutcome::Prepared(Box::new(original.receipt)));
    }
    // All fresh source/mapping/pool planning follows original identity recovery.
    super::super::check_source_counts(
        turn.connection,
        access
            .authority
            .sources
            .ok_or(OperationError::RecoveryRequired)?,
    )
    .await?;
    if request.source_checkpoint().authorization() != access.authority.authorization {
        return Err(WorkspaceError::StaleCheckpoint.into());
    }
    let source_minimum = source::checkpoint::authenticate(
        turn.connection,
        turn.selected,
        turn.crypto,
        access
            .authority
            .sources
            .ok_or(OperationError::RecoveryRequired)?,
        source::checkpoint::Target::Minimum(request.source_checkpoint()),
    )
    .await?;
    let admitted = source::request::prepare(
        turn.connection,
        turn.selected,
        turn.crypto,
        source::history::InventoryRead {
            binding: &access.current,
            authority: &access.authority,
            minimum: request.minimum_checkpoint(),
        },
        command.input().request(),
        request.listing_checkpoint(),
        access.current.observed_at(),
    )
    .await?;
    let expected = command
        .context()
        .expected()
        .ok_or(OperationError::InvalidCommand)?;
    if expected.order_id() != admitted.scope.order_id()
        || expected.agreement().is_some()
        || expected.seller_sequence().get() != 0
        || expected.buyer_head() != admitted.history.analysis.buyer_head()
        || Some(expected.checkpoint()) != admitted.history.checkpoint.as_ref()
        || command.input().terms_digest() != admitted.digest
        || command.context().workspace_id() != turn.selected.workspace_id()
        || command.context().authority() != turn.selected.authority()
        || command.context().route_digest() != turn.selected.route_digest()
    {
        return Err(OperationError::StaleState);
    }
    let current_order = if access.authority.catalog >= 13 {
        let checked = Box::pin(checkpoint::authenticate_order_state(
            turn.connection,
            turn.selected,
            turn.crypto,
            &access.current,
            &access.authority,
            expected.order_id(),
        ))
        .await?;
        if checked
            .witness
            .expected
            .is_some_and(|value| value.phase != super::super::order_state::Phase::Aborted)
        {
            return Err(OperationError::StaleState);
        }
        Some(checked)
    } else {
        None
    };
    let order_proof = if current_order.is_none() {
        Some(
            checkpoint::authenticate_order_absence(
                turn.connection,
                turn.selected,
                turn.crypto,
                &access.current,
                &access.authority,
                expected.order_id(),
            )
            .await?,
        )
    } else {
        None
    };
    let mut mappings = Vec::new();
    let mut mapping_reads = Vec::new();
    let mut pool_ids = BTreeSet::new();
    if matches!(
        admitted.terms()?.supply(),
        radroots_domain::SupplyCommitment::StockBacked
    ) {
        for line in admitted.terms()?.lines() {
            let key = radroots_domain::stock::OptionMappingKey::new(
                line.source().coordinate(),
                line.option_id(),
            );
            let retained = mapping::read(turn.connection, turn.selected, turn.crypto, key).await?;
            let witness = checkpoint::authenticate_mapping_projection(
                turn.connection,
                turn.selected,
                turn.crypto,
                &access.current,
                &access.authority,
                key,
                retained.as_ref().map(|value| value.value),
            )
            .await?;
            let value = retained.as_ref().ok_or(OperationError::StaleState)?.value;
            pool_ids.insert(value.pool_id());
            mappings.push(value);
            mapping_reads.push(mapping::MappingRead {
                key,
                retained,
                proof: witness,
            });
        }
        if let Some(explicit) = command.input().allocation().explicit_allocations() {
            pool_ids.extend(explicit.iter().map(|value| value.pool_id()));
        }
    }
    let mut pool_reads = BTreeMap::new();
    let mut pool_proofs = Vec::new();
    let mut before_pools = Vec::new();
    for id in pool_ids {
        if !consent.resources.contains(id) {
            return Err(OperationError::Unauthorized);
        }
        let retained = read_pool(turn.connection, turn.selected, turn.crypto, id).await?;
        pool_proofs.push(
            checkpoint::authenticate_projection(
                turn.connection,
                turn.selected,
                turn.crypto,
                &access.current,
                &access.authority,
                id,
                retained.as_ref().map(|value| value.pool),
            )
            .await?,
        );
        let retained = retained.ok_or(OperationError::StaleState)?;
        before_pools.push(retained.pool);
        pool_reads.insert(id, retained);
    }
    let (allocations, effects) =
        plan::resolve(admitted.terms()?, command, &mappings, &before_pools)?;
    let before = access
        .authority
        .inventory
        .ok_or(OperationError::RecoveryRequired)?;
    let transaction = TransactionId::from_bytes(super::super::super::random()?);
    let revision = Sequence::new(
        before
            .revision
            .get()
            .checked_add(1)
            .ok_or(WorkspaceError::Capacity)?,
    )
    .map_err(|_| WorkspaceError::Capacity)?;
    let expires = admitted
        .expires
        .min(command.context().not_after())
        .min(consent.expires)
        .min(
            command
                .context()
                .approval()
                .ok_or(OperationError::UnverifiedApproval)?
                .expires_at(),
        )
        .min(access.current.expires_at());
    let issue_before = UnixSeconds::new(
        access
            .current
            .observed_at()
            .get()
            .checked_add(120)
            .ok_or(WorkspaceError::Capacity)?,
    )
    .map_err(|_| WorkspaceError::Capacity)?
    .min(expires);
    let mut movements = Vec::new();
    for _ in &effects {
        movements.push(super::super::super::random()?);
    }
    let terms = radroots_protocol::terms::EncodedTerms::capture(admitted.terms()?)
        .map_err(|_| OperationError::InvalidEvidence)?;
    let original_request = radroots_protocol::commerce::EncodedRecord::capture(
        admitted
            .history
            .records
            .get(&admitted.request)
            .ok_or(OperationError::RecoveryRequired)?
            .content(),
    )
    .map_err(|_| OperationError::RecoveryRequired)?;
    let original_quote = admitted
        .quote
        .map(|id| {
            radroots_protocol::commerce::EncodedRecord::capture(
                admitted
                    .history
                    .records
                    .get(&id)
                    .ok_or(OperationError::RecoveryRequired)?
                    .content(),
            )
            .map_err(|_| OperationError::RecoveryRequired)
        })
        .transpose()?;
    let seller_author_head = admitted.history.analysis.seller_head();
    let seller_author_sequence = seller_author_head
        .map(|id| {
            admitted
                .history
                .records
                .get(&id)
                .map(|record| record.content().header().author_sequence())
                .ok_or(OperationError::RecoveryRequired)
        })
        .transpose()?
        .unwrap_or(Sequence::new(0).map_err(|_| OperationError::RecoveryRequired)?);
    let body = Body {
        version: 1,
        command: serde_json::from_slice(command.protected_wire())
            .map_err(|_| OperationError::InvalidCommand)?,
        terms: serde_json::from_slice(terms.protected_wire())
            .map_err(|_| OperationError::InvalidEvidence)?,
        original_request: serde_json::from_slice(original_request.protected_wire())
            .map_err(|_| OperationError::RecoveryRequired)?,
        original_quote: original_quote
            .as_ref()
            .map(|quote| {
                serde_json::from_slice(quote.protected_wire())
                    .map_err(|_| OperationError::RecoveryRequired)
            })
            .transpose()?,
        seller_author_head,
        seller_author_sequence,
        minimum_revision: request.minimum_checkpoint().revision(),
        minimum_head: request.minimum_checkpoint().head(),
        authorization: access
            .authority
            .authorization
            .ok_or(OperationError::RecoveryRequired)?,
        source_minimum: Progress::source(request.source_checkpoint())?,
        listing_minimum: Progress::listing(request.listing_checkpoint())?,
        coverage: expected.checkpoint().clone(),
        buyer_head: expected.buyer_head(),
        observed_at: access.current.observed_at(),
        expires,
        issue_before,
        consent: consent.frame.header.operation,
        mappings,
        pools: before_pools,
        allocations,
        effects,
        movements,
    };
    let raw = bytes(&body)?;
    let mut header = ledger::Header {
        version: 5,
        family: ledger::Family::Acceptance,
        namespace: SellerAuthoritySelection::from_attachment(turn.selected),
        actor: command.context().actor(),
        operation,
        digest: command.digest(),
        transaction,
        revision,
        previous: before.head,
        head: CheckpointDigest::from_bytes([0; 32]),
        pool: None,
        order: Some(expected.order_id()),
        related_operation: None,
        subject_grant: None,
        bytes: 0,
        parts: 0,
        body_digest: [0; 32],
    };
    header.head = head(&header, &raw);
    let framed = ledger::prepare(
        turn.crypto,
        turn.selected,
        header_for_prepare(&header),
        &raw,
    )
    .await?;
    let fence = fence(&header, command)?;
    let order_cipher = turn
        .crypto
        .seal_stock_record(
            turn.selected,
            "order",
            fence.order.as_bytes(),
            &bytes(&fence)?,
        )
        .await?;
    let mut changed_pools = Vec::new();
    let mut changed_movements = Vec::new();
    for (effect, movement) in body.effects.iter().zip(&body.movements) {
        let original = &pool_reads[&effect.before().id()];
        changed_pools.push(
            turn.crypto
                .seal_stock_record(
                    turn.selected,
                    "pool",
                    effect.after().id().as_bytes(),
                    &encode(&PoolWire {
                        version: 1,
                        label: original.view.label(),
                        pool: effect.after(),
                    })?,
                )
                .await?,
        );
        changed_movements.push(
            turn.crypto
                .seal_stock_record(
                    turn.selected,
                    "acceptance_movement",
                    movement,
                    &bytes(&codec::Hold {
                        version: 1,
                        operation,
                        digest: command.digest(),
                        order: fence.order,
                        effect: *effect,
                    })?,
                )
                .await?,
        );
    }
    let mut next = access.authority.clone();
    next.inventory = Some(InventoryCheckpoint {
        revision,
        head: Some(header.head),
        orders: Some(
            Sequence::new(
                before
                    .orders
                    .ok_or(OperationError::RecoveryRequired)?
                    .get()
                    .checked_add(1)
                    .ok_or(WorkspaceError::Capacity)?,
            )
            .map_err(|_| WorkspaceError::Capacity)?,
        ),
        movements: Sequence::new(
            before
                .movements
                .get()
                .checked_add(body.effects.len() as u64)
                .ok_or(WorkspaceError::Capacity)?,
        )
        .map_err(|_| WorkspaceError::Capacity)?,
        ..before
    });
    let root = turn
        .crypto
        .seal_stock_root(turn.selected, next.catalog, &next.encode()?)
        .await?;
    let receipt = codec::receipt(turn.selected, &header, &body, command)?;
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let changed: Result<(), OperationError> = async {
        identity.confirm(&mut tx).await?; consent_proof.confirm(&mut tx).await?; source_minimum.confirm(&mut tx).await?;
        if let Some(order_proof) = &order_proof {order_proof.confirm(&mut tx).await?;} admitted.confirm(&mut tx).await?;
        super::super::check_source_counts(&mut tx, access.authority.sources.ok_or(OperationError::RecoveryRequired)?).await?;
        for mapping in &mapping_reads { mapping.confirm(&mut tx).await?; }
        for witness in &pool_proofs { witness.confirm(&mut tx).await?; }
        for retained in pool_reads.values() {
            let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_pools WHERE pool_id=? AND revision=? AND protected=?").bind(retained.pool.id().as_bytes().as_slice()).bind(retained.pool.version().get() as i64).bind(&retained.protected).fetch_one(&mut *tx).await.map_err(|_| OperationError::RecoveryRequired)?;
            if count != 1 { return Err(OperationError::RecoveryRequired); }
        }
        let current = access.confirm(&mut tx, turn.namespace, turn.clock, &proof).await?;
        check_consent(&access, &consent, command, true, current.observed_at())?; admitted.validate_at(current.observed_at())?;
        if current.observed_at() >= body.issue_before { return Err(OperationError::DeadlineExceeded); }
        if current_order.is_none() {
            let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_orders WHERE order_id=?").bind(fence.order.as_bytes().as_slice()).fetch_one(&mut *tx).await.map_err(|_| OperationError::StorageFailure)?;
            if count != 0 { return Err(OperationError::StaleState); }
        }
        sqlx::query("INSERT INTO stock_operations(actor,grant_id,operation_id,command_digest,transaction_id,pool_id,revision,chain_head,protected,family,order_id,related_operation,subject_grant) VALUES(?,?,?,?,?,NULL,?,?,?,2,?,NULL,NULL)")
            .bind(header.actor.participant().as_bytes().as_slice()).bind(access.current.input().grant_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(header.digest.as_bytes().as_slice()).bind(transaction.as_bytes().as_slice()).bind(revision.get() as i64).bind(header.head.as_bytes().as_slice()).bind(&framed.protected).bind(fence.order.as_bytes().as_slice()).execute(&mut *tx).await.map_err(|_| OperationError::StorageFailure)?;
        framed.insert_parts(&mut tx, header.actor, operation).await?;
        for (index, effect) in body.effects.iter().enumerate() {
            let original = &pool_reads[&effect.before().id()];
            let updated = sqlx::query("UPDATE stock_pools SET revision=?,protected=? WHERE pool_id=? AND revision=? AND protected=?").bind(effect.after().version().get() as i64).bind(&changed_pools[index]).bind(effect.before().id().as_bytes().as_slice()).bind(effect.before().version().get() as i64).bind(&original.protected).execute(&mut *tx).await.map_err(|_| OperationError::StorageFailure)?;
            if updated.rows_affected() != 1 { return Err(OperationError::StaleState); }
            sqlx::query("INSERT INTO stock_movements(movement_id,actor,grant_id,operation_id,pool_id,protected) VALUES(?,?,?,?,?,?)").bind(body.movements[index].as_slice()).bind(header.actor.participant().as_bytes().as_slice()).bind(access.current.input().grant_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(effect.before().id().as_bytes().as_slice()).bind(&changed_movements[index]).execute(&mut *tx).await.map_err(|_| OperationError::StorageFailure)?;
        }
        if let Some(current_order) = &current_order {
            sqlx::query("INSERT INTO stock_orders(actor,operation_id,order_id,revision,protected) VALUES(?,?,?,?,?)")
                .bind(header.actor.participant().as_bytes().as_slice()).bind(operation.as_bytes().as_slice())
                .bind(fence.order.as_bytes().as_slice()).bind(fence.revision.get() as i64).bind(order_cipher)
                .execute(&mut *tx).await.map_err(|_| OperationError::StorageFailure)?;
            current_order.advance(&mut tx, super::super::order_state::Marker::from_header(&framed.header, super::super::order_state::Phase::Prepared)?).await?;
        } else {
            sqlx::query("INSERT INTO stock_orders(order_id,revision,protected) VALUES(?,?,?)").bind(fence.order.as_bytes().as_slice()).bind(fence.revision.get() as i64).bind(order_cipher).execute(&mut *tx).await.map_err(|_| OperationError::StorageFailure)?;
        }
        let updated = sqlx::query("UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?").bind(root).bind(&access.root).execute(&mut *tx).await.map_err(|_| OperationError::StorageFailure)?;
        if updated.rows_affected() != 1 { return Err(OperationError::RecoveryRequired); }
        turn.namespace.validate()?; Ok(())
    }.await;
    let attempt = receipt.attempt();
    if let Err(error) = changed {
        if tx.rollback().await.is_err() {
            *turn.uncertain = true;
            return Ok(StockAcceptanceOutcome::Unknown(attempt));
        }
        return Err(error);
    }
    if tx.commit().await.is_err() {
        *turn.uncertain = true;
        return Ok(StockAcceptanceOutcome::Unknown(attempt));
    }
    Ok(StockAcceptanceOutcome::Prepared(Box::new(receipt)))
}
fn header_for_prepare(header: &ledger::Header) -> ledger::Header {
    ledger::Header {
        version: header.version,
        family: header.family,
        namespace: header.namespace.clone(),
        actor: header.actor,
        operation: header.operation,
        digest: header.digest,
        transaction: header.transaction,
        revision: header.revision,
        previous: header.previous,
        head: header.head,
        pool: header.pool,
        order: header.order,
        related_operation: header.related_operation,
        subject_grant: header.subject_grant,
        bytes: header.bytes,
        parts: header.parts,
        body_digest: header.body_digest,
    }
}
impl NativeWorkspace {
    /// Atomically prepares an original acceptance and its physical holds. This
    /// never invokes a signer or installs a bilateral agreement.
    pub async fn prepare_stock_acceptance(
        &self,
        request: StockAcceptanceRequest,
        proof: &AttachmentProof,
    ) -> Result<StockAcceptanceOutcome, OperationError> {
        let proof = proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?;
        self.worker.prepare_stock_acceptance(request, proof).await
    }
}

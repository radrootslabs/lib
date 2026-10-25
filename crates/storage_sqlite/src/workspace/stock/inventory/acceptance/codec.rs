//! Exact original preparation validation, independent of fresh source readiness.
use super::*;

#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Hold {
    pub version: u8,
    pub operation: OperationId,
    pub digest: CommandDigest,
    pub order: OrderId,
    pub effect: plan::Effect,
}
pub(in crate::workspace::stock) struct Retained {
    pub frame: ledger::Retained,
    pub receipt: StockAcceptanceReceipt,
    pub movements: Vec<ledger::MovementWitness>,
    order_cipher: Vec<u8>,
}
impl Retained {
    pub fn command(&self) -> Result<CapturedOrderAcceptance, OperationError> {
        let body: Body = super::super::super::closed_record(&self.frame.body)
            .map_err(|_| OperationError::RecoveryRequired)?;
        CapturedOrderAcceptance::parse(body.command.get().as_bytes())
            .map_err(|_| OperationError::RecoveryRequired)
    }
    pub fn identity(&self) -> checkpoint::EntryIdentity {
        checkpoint::EntryIdentity {
            operation: self.frame.header.operation,
            revision: self.frame.header.revision,
            head: self.frame.header.head,
            protected_digest: Sha256::digest(&self.frame.protected).into(),
        }
    }
    pub async fn confirm(&self, connection: &mut SqliteConnection) -> Result<(), OperationError> {
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
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
        if count != self.movements.len() as i64 {
            return Err(OperationError::RecoveryRequired);
        }
        for movement in &self.movements {
            let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_movements WHERE movement_id=? AND actor=? AND grant_id=? AND operation_id=? AND pool_id=? AND protected=?").bind(movement.id.as_slice()).bind(self.frame.header.actor.participant().as_bytes().as_slice()).bind(movement.grant.as_bytes().as_slice()).bind(self.frame.header.operation.as_bytes().as_slice()).bind(movement.pool.as_bytes().as_slice()).bind(&movement.protected).fetch_one(&mut *connection).await.map_err(|_| OperationError::RecoveryRequired)?;
            if count != 1 {
                return Err(OperationError::RecoveryRequired);
            }
        }
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM stock_orders WHERE order_id=? AND revision=1 AND protected=?",
        )
        .bind(self.receipt.order.as_bytes().as_slice())
        .bind(&self.order_cipher)
        .fetch_one(connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
        if count != 1 {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
    pub fn into_history(self) -> ledger::HistoryEntry {
        ledger::HistoryEntry {
            signing: None,
            header: self.frame.header,
            protected: self.frame.protected,
            parts_witness: self.frame.parts_witness,
            pools: self.receipt.pools,
            mapping: None,
            movements: self.movements,
        }
    }
}
pub(super) fn receipt(
    input: &AttachmentInput,
    header: &ledger::Header,
    body: &Body,
    command: &CapturedOrderAcceptance,
) -> Result<StockAcceptanceReceipt, OperationError> {
    let request = radroots_protocol::commerce::decode(body.original_request.get().as_bytes())
        .map_err(|_| OperationError::RecoveryRequired)?;
    if body.seller_author_head.is_none() != (body.seller_author_sequence.get() == 0) {
        return Err(OperationError::RecoveryRequired);
    }
    let rumor = radroots_protocol::commerce::initial_commitment_rumor(
        command,
        &request,
        body.seller_author_head
            .map(|head| (head, body.seller_author_sequence)),
        body.observed_at,
    )
    .map_err(|_| OperationError::RecoveryRequired)?;
    Ok(StockAcceptanceReceipt {
        rumor: std::sync::Arc::new(rumor),
        attempt: CommitAttempt::new(
            input.workspace_id(),
            header.operation,
            header.digest,
            Some(header.transaction),
        ),
        order: header.order.ok_or(OperationError::RecoveryRequired)?,
        request: command.input().request(),
        terms: command.input().terms_digest(),
        checkpoint: InventoryCheckpointRef::from_host_metadata(
            input.authority(),
            header.revision,
            Some(header.head),
        )?
        .with_authorization(body.authorization),
        pools: body.effects.iter().map(|value| value.after()).collect(),
        allocations: body.allocations.len(),
        observed: body.observed_at,
        issue_before: body.issue_before,
    })
}
pub(in crate::workspace::stock) async fn read(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    operation: OperationId,
) -> Result<Option<Retained>, OperationError> {
    let Some(frame) = ledger::read(connection, input, crypto, operation).await? else {
        return Ok(None);
    };
    if frame.header.family != ledger::Family::Acceptance {
        return Err(OperationError::IdempotencyConflict);
    }
    let body: Body = super::super::super::closed_record(&frame.body)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let command = CapturedOrderAcceptance::parse(body.command.get().as_bytes())
        .map_err(|_| OperationError::RecoveryRequired)?;
    let terms = radroots_protocol::terms::decode(body.terms.get().as_bytes())
        .map_err(|_| OperationError::RecoveryRequired)?;
    let encoded = radroots_protocol::terms::EncodedTerms::capture(&terms)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let original = radroots_protocol::commerce::decode(body.original_request.get().as_bytes())
        .map_err(|_| OperationError::RecoveryRequired)?;
    let radroots_domain::CommercePayload::Request(original_request) = original.content().payload()
    else {
        return Err(OperationError::RecoveryRequired);
    };
    if original.record_id() != command.input().request()
        || original_request.terms() != &terms
        || original.content().header().order_id()
            != command.context().expected().map(|value| value.order_id())
        || original.content().header().author() != terms.buyer()
        || original.content().header().created_at() > body.observed_at
        || body.expires > original_request.expires_at()
        || original_request.quote().is_some() != body.original_quote.is_some()
    {
        return Err(OperationError::RecoveryRequired);
    }
    if let Some(raw) = body.original_quote {
        let quoted = radroots_protocol::commerce::decode(raw.get().as_bytes())
            .map_err(|_| OperationError::RecoveryRequired)?;
        let radroots_domain::CommercePayload::Quote(quote) = quoted.content().payload() else {
            return Err(OperationError::RecoveryRequired);
        };
        if Some(quoted.record_id()) != original_request.quote()
            || quote.terms() != &terms
            || body.expires > quote.expires_at()
            || quoted.content().header().thread_id() != original.content().header().thread_id()
            || quoted.content().header().buyer() != terms.buyer()
            || quoted.content().header().seller() != terms.seller()
            || quoted.content().header().created_at() > body.observed_at
        {
            return Err(OperationError::RecoveryRequired);
        }
    }
    let expected = command
        .context()
        .expected()
        .ok_or(OperationError::RecoveryRequired)?;
    let header = &frame.header;
    if body.version != 1
        || (body.seller_author_sequence.get() == 0) != body.seller_author_head.is_none()
        || body.minimum_revision >= header.revision
        || (body.minimum_revision.get() == 0) != body.minimum_head.is_none()
        || header.operation != command.context().operation_id()
        || header.actor != command.context().actor()
        || header.digest != command.digest()
        || command.context().workspace_id() != input.workspace_id()
        || command.context().authority() != input.authority()
        || command.context().route_digest() != input.route_digest()
        || header.order != Some(expected.order_id())
        || expected.agreement().is_some()
        || expected.seller_sequence().get() != 0
        || expected.checkpoint() != &body.coverage
        || expected.buyer_head() != body.buyer_head
        || encoded.digest() != command.input().terms_digest()
        || terms.seller() != header.actor.participant()
        || body.observed_at.get() == 0
        || body.expires <= body.observed_at
        || body.issue_before <= body.observed_at
        || body.issue_before > body.expires
        || body.issue_before.get() - body.observed_at.get() > 120
        || body.expires > command.context().not_after()
        || command.context().approval().is_none_or(|approval| {
            approval.scope() != "orders.accept"
                || approval.intent_digest() != command.approval_intent_digest()
                || body.expires > approval.expires_at()
        })
        || body.source_minimum.authorization != body.authorization
        || body.listing_minimum.authorization != body.authorization
        || body.source_minimum.authority != input.authority()
        || body.listing_minimum.authority != input.authority()
        || (body.source_minimum.revision.get() == 0) != body.source_minimum.head.is_none()
        || (body.listing_minimum.revision.get() == 0) != body.listing_minimum.head.is_none()
        || body.movements.len() != body.effects.len()
        || body
            .movements
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != body.movements.len()
        || body.effects.len() > 64
        || body.allocations.len() > 64
        || body.mappings.len() > 32
        || body.pools.len() > 96
        || head(header, &frame.body) != header.head
    {
        return Err(OperationError::RecoveryRequired);
    }
    let (allocations, effects) = plan::resolve(&terms, &command, &body.mappings, &body.pools)
        .map_err(|_| OperationError::RecoveryRequired)?;
    if allocations != body.allocations || effects != body.effects {
        return Err(OperationError::RecoveryRequired);
    }
    let expected_fence = fence(header, &command)?;
    let row = sqlx::query("SELECT revision,protected FROM stock_orders WHERE order_id=? AND length(protected) BETWEEN 1 AND 262144").bind(expected_fence.order.as_bytes().as_slice()).fetch_optional(&mut *connection).await.map_err(|_| OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
    let revision: i64 = row
        .try_get(0)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let order_cipher: Vec<u8> = row
        .try_get(1)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let clear = crypto
        .open_stock_record(
            input,
            "order",
            expected_fence.order.as_bytes(),
            &order_cipher,
        )
        .await?;
    let order: Fence = clear
        .expose_secret(|bytes| super::super::super::closed_record(bytes))
        .map_err(|_| OperationError::RecoveryRequired)?;
    if revision != 1 || order != expected_fence {
        return Err(OperationError::RecoveryRequired);
    }
    let mut movements = Vec::new();
    let grant = header
        .actor
        .grant_id()
        .ok_or(OperationError::RecoveryRequired)?;
    for (id, effect) in body.movements.iter().zip(&body.effects) {
        let protected: Vec<u8> = sqlx::query_scalar("SELECT protected FROM stock_movements WHERE movement_id=? AND actor=? AND grant_id=? AND operation_id=? AND pool_id=? AND length(protected) BETWEEN 1 AND 262144").bind(id.as_slice()).bind(header.actor.participant().as_bytes().as_slice()).bind(grant.as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(effect.before().id().as_bytes().as_slice()).fetch_optional(&mut *connection).await.map_err(|_| OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
        let clear = crypto
            .open_stock_record(input, "acceptance_movement", id, &protected)
            .await?;
        let hold: Hold = clear
            .expose_secret(|bytes| super::super::super::closed_record(bytes))
            .map_err(|_| OperationError::RecoveryRequired)?;
        if hold
            != (Hold {
                version: 1,
                operation,
                digest: header.digest,
                order: expected_fence.order,
                effect: *effect,
            })
        {
            return Err(OperationError::RecoveryRequired);
        }
        movements.push(ledger::MovementWitness {
            id: *id,
            pool: effect.before().id(),
            grant,
            protected,
        });
    }
    movements.sort_by_key(|value| value.id);
    let retained = Retained {
        receipt: receipt(input, header, &body, &command)?,
        frame,
        movements,
        order_cipher,
    };
    retained.confirm(connection).await?;
    Ok(Some(retained))
}

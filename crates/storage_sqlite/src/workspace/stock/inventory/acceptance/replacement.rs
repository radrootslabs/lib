//! Original installed draws and frozen complete replacement preparation data.
use super::*;
use radroots_domain::stock::{ObligationBacking, StockAllocation, StockTicks};
use radroots_protocol::commerce::{DecodedRecord, EncodedRecord};

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::workspace::stock) struct Draw {
    pub acceptance: OperationId,
    pub installation: OperationId,
    pub allocation: StockAllocation,
    pub current: ObligationBacking,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Basis<'a> {
    pub base_operation: OperationId,
    pub base_installation: OperationId,
    #[serde(borrow)]
    pub base: &'a RawValue,
    #[serde(borrow)]
    pub proposal: &'a RawValue,
    #[serde(borrow)]
    pub authorization: &'a RawValue,
    pub originals: Vec<Draw>,
    pub desired: Vec<StockAllocation>,
    pub releases: Vec<StockAllocation>,
}
pub(super) struct Base {
    pub operation: OperationId,
    pub installation: OperationId,
    pub preparation: codec::Retained,
    pub record: DecodedRecord,
    pub originals: Vec<Draw>,
}
pub(super) fn record(receipt: &StockAcceptanceReceipt) -> Result<DecodedRecord, OperationError> {
    let rumor = radroots_protocol::private_rumor::DecodedRumor::decode(
        receipt.rumor().protected_wire().as_bytes(),
    )
    .map_err(|_| OperationError::RecoveryRequired)?;
    radroots_protocol::commerce::decode(rumor.protected_content().as_bytes())
        .map_err(|_| OperationError::RecoveryRequired)
}
pub(super) async fn load(
    turn: &mut Turn<'_>,
    access: &Access,
    command: &CapturedStockCommitment,
) -> Result<Base, OperationError> {
    let expected = command
        .context()
        .expected()
        .ok_or(OperationError::InvalidCommand)?;
    let row=sqlx::query("SELECT operation_id,entry_id,commit_sequence FROM stock_agreements WHERE actor=? AND order_id=? ORDER BY commit_sequence DESC LIMIT 1")
        .bind(turn.selected.participant().as_bytes().as_slice()).bind(expected.order_id().as_bytes().as_slice()).fetch_optional(&mut *turn.connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::StaleState)?;
    let id = |index| -> Result<OperationId, OperationError> {
        Ok(OperationId::from_bytes(
            row.try_get::<Vec<u8>, _>(index)
                .map_err(|_| OperationError::RecoveryRequired)?
                .try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        ))
    };
    let operation = id(0)?;
    let installation = id(1)?;
    let sequence: i64 = row
        .try_get(2)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let installed = Box::pin(super::super::installation::read_entry(
        turn.connection,
        turn.selected,
        turn.crypto,
        installation,
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    let preparation = Box::pin(codec::read(
        turn.connection,
        turn.selected,
        turn.crypto,
        operation,
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    let record = record(&preparation.receipt)?;
    if sequence <= 0
        || sequence as u64 != expected.seller_sequence().get()
        || expected.agreement() != Some(record.record_id())
        || installed.receipt.record_id() != record.record_id()
        || installed.receipt.order_id() != expected.order_id()
    {
        return Err(OperationError::StaleState);
    }
    let before_revision = access
        .authority
        .inventory
        .ok_or(OperationError::RecoveryRequired)?
        .revision
        .get()
        .checked_add(1)
        .ok_or(WorkspaceError::Capacity)?;
    let mut originals = Vec::new();
    let mut cursor = 0_i64;
    loop {
        let row=sqlx::query("SELECT operation_id,entry_id,commit_sequence FROM stock_agreements WHERE actor=? AND order_id=? AND commit_sequence>? ORDER BY commit_sequence LIMIT 1")
            .bind(turn.selected.participant().as_bytes().as_slice()).bind(expected.order_id().as_bytes().as_slice()).bind(cursor).fetch_optional(&mut *turn.connection).await.map_err(|_|OperationError::RecoveryRequired)?;
        let Some(row) = row else {
            break;
        };
        let id = |index| -> Result<OperationId, OperationError> {
            Ok(OperationId::from_bytes(
                row.try_get::<Vec<u8>, _>(index)
                    .map_err(|_| OperationError::RecoveryRequired)?
                    .try_into()
                    .map_err(|_| OperationError::RecoveryRequired)?,
            ))
        };
        let acceptance = id(0)?;
        let installation = id(1)?;
        let sequence: i64 = row
            .try_get(2)
            .map_err(|_| OperationError::RecoveryRequired)?;
        if sequence != cursor + 1 || sequence as usize > radroots_domain::causal::MAX_CAUSAL_RECORDS
        {
            return Err(OperationError::RecoveryRequired);
        }
        cursor = sequence;
        let installed = Box::pin(super::super::installation::read_entry(
            turn.connection,
            turn.selected,
            turn.crypto,
            installation,
        ))
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
        let prepared = Box::pin(codec::read(
            turn.connection,
            turn.selected,
            turn.crypto,
            acceptance,
        ))
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
        if installed.receipt.order_id() != expected.order_id()
            || prepared.receipt.order_id() != expected.order_id()
            || installed.frame.header.related_operation != Some(acceptance)
        {
            return Err(OperationError::RecoveryRequired);
        }
        for allocation in prepared.original_allocations()? {
            let original = super::super::allocations::InstalledAllocation {
                acceptance,
                installation,
                allocation,
            };
            let current = original
                .backing(turn.connection, turn.selected, turn.crypto, before_revision)
                .await?;
            if current.remaining() != StockTicks::ZERO {
                originals.push(Draw {
                    acceptance,
                    installation,
                    allocation,
                    current,
                });
            }
            if originals.len() > 64 {
                return Err(WorkspaceError::Capacity.into());
            }
        }
    }
    if cursor != sequence {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(Base {
        operation,
        installation,
        preparation,
        record,
        originals,
    })
}
pub(super) fn aggregate(originals: &[Draw]) -> Result<Vec<StockAllocation>, OperationError> {
    if originals.len() > 64 {
        return Err(WorkspaceError::Capacity.into());
    }
    let mut pairs = BTreeSet::new();
    let mut quantities = BTreeMap::new();
    for draw in originals {
        if draw.current.remaining() == StockTicks::ZERO
            || draw.current.remaining() > draw.allocation.ticks()
            || !pairs.insert((
                draw.acceptance,
                draw.allocation.line_id(),
                draw.allocation.pool_id(),
            ))
        {
            return Err(OperationError::RecoveryRequired);
        }
        let quantity = quantities
            .entry((draw.allocation.pool_id(), draw.allocation.line_id()))
            .or_insert(StockTicks::ZERO);
        *quantity = quantity
            .checked_add(draw.current.remaining())
            .map_err(|_| WorkspaceError::Capacity)?;
    }
    quantities
        .into_iter()
        .map(|((pool, line), quantity)| {
            StockAllocation::new(line, pool, quantity).map_err(|_| OperationError::RecoveryRequired)
        })
        .collect()
}
pub(super) struct Resolution {
    pub allocations: Vec<plan::Allocation>,
    pub effects: Vec<plan::Effect>,
    pub desired: Vec<StockAllocation>,
    pub releases: Vec<StockAllocation>,
}
pub(super) fn resolve(
    terms: &radroots_domain::Terms,
    command: &CapturedStockCommitment,
    mappings: &[radroots_domain::stock::OptionPoolMapping],
    pools: &[PoolSnapshot],
    originals: &[Draw],
) -> Result<Resolution, OperationError> {
    let current = aggregate(originals)?;
    let explicit = plan::claims(command)?;
    let desired = radroots_domain::stock::resolve_replacement_allocations(
        terms,
        mappings,
        pools,
        explicit.as_deref(),
        &current,
    )
    .map_err(plan::error)?;
    let delta = radroots_domain::stock::plan_replacement_allocations(&current, &desired, pools)
        .map_err(plan::error)?;
    Ok(Resolution {
        allocations: delta.additional_allocations().to_vec(),
        effects: delta.preparatory_holds().to_vec(),
        desired,
        releases: delta.pending_releases().to_vec(),
    })
}
pub(super) fn validate(
    body: &Body,
    command: &CapturedStockCommitment,
) -> Result<(), OperationError> {
    let basis = body
        .replacement
        .as_ref()
        .ok_or(OperationError::RecoveryRequired)?;
    if body.version != 2
        || body.original_request.is_some()
        || body.original_quote.is_some()
        || basis.base_operation == command.context().operation_id()
        || basis.base_installation == command.context().operation_id()
    {
        return Err(OperationError::RecoveryRequired);
    }
    let terms = radroots_protocol::terms::decode(body.terms.get().as_bytes())
        .map_err(|_| OperationError::RecoveryRequired)?;
    let proposal = radroots_protocol::commerce::decode(basis.proposal.get().as_bytes())
        .map_err(|_| OperationError::RecoveryRequired)?;
    let authorization = radroots_protocol::commerce::decode(basis.authorization.get().as_bytes())
        .map_err(|_| OperationError::RecoveryRequired)?;
    let radroots_domain::CommercePayload::TermsProposed(proposed) = proposal.content().payload()
    else {
        return Err(OperationError::RecoveryRequired);
    };
    let radroots_domain::CommercePayload::TermsAuthorized(authorized) =
        authorization.content().payload()
    else {
        return Err(OperationError::RecoveryRequired);
    };
    if proposed.terms() != &terms
        || body.expires > proposed.expires_at()
        || body.expires > authorized.expires_at()
    {
        return Err(OperationError::RecoveryRequired);
    }
    let planned = resolve(
        &terms,
        command,
        &body.mappings,
        &body.pools,
        &basis.originals,
    )?;
    if planned.allocations != body.allocations
        || planned.effects != body.effects
        || planned.desired != basis.desired
        || planned.releases != basis.releases
    {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}
pub(super) fn encoded(
    records: &BTreeMap<RecordId, DecodedRecord>,
    id: RecordId,
) -> Result<EncodedRecord, OperationError> {
    EncodedRecord::capture(
        records
            .get(&id)
            .ok_or(OperationError::RecoveryRequired)?
            .content(),
    )
    .map_err(|_| OperationError::RecoveryRequired)
}
pub(super) async fn confirm_residual(
    body: &Body<'_>,
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    before_revision: u64,
) -> Result<(), OperationError> {
    if let Some(basis) = &body.replacement {
        for draw in &basis.originals {
            let original = super::super::allocations::InstalledAllocation {
                acceptance: draw.acceptance,
                installation: draw.installation,
                allocation: draw.allocation,
            };
            if original
                .backing(connection, input, crypto, before_revision)
                .await?
                != draw.current
            {
                return Err(OperationError::StaleState);
            }
        }
    }
    Ok(())
}
pub(super) async fn releases(
    body: &Body<'_>,
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    before_revision: u64,
) -> Result<Vec<super::super::changes::Release>, OperationError> {
    let Some(basis) = &body.replacement else {
        return Ok(Vec::new());
    };
    let mut needed: BTreeMap<_, _> = basis
        .releases
        .iter()
        .map(|value| ((value.pool_id(), value.line_id()), value.ticks()))
        .collect();
    if needed.len() != basis.releases.len() {
        return Err(OperationError::RecoveryRequired);
    }
    let mut changes = Vec::new();
    for draw in &basis.originals {
        let Some(needed) = needed.get_mut(&(draw.allocation.pool_id(), draw.allocation.line_id()))
        else {
            continue;
        };
        if *needed == StockTicks::ZERO {
            continue;
        }
        let original = super::super::allocations::InstalledAllocation {
            acceptance: draw.acceptance,
            installation: draw.installation,
            allocation: draw.allocation,
        };
        let before = original
            .backing(connection, input, crypto, before_revision)
            .await?;
        if before.remaining() != draw.current.remaining() {
            return Err(OperationError::StaleState);
        }
        let quantity = (*needed).min(before.remaining());
        let deficit = quantity.min(before.deficit());
        let backed = quantity
            .checked_sub(deficit)
            .map_err(|_| OperationError::RecoveryRequired)?;
        let after = before
            .release_residual(backed, deficit)
            .map_err(|_| OperationError::RecoveryRequired)?;
        changes.push(super::super::changes::Release {
            acceptance: draw.acceptance,
            installation: draw.installation,
            line: draw.allocation.line_id(),
            pool: draw.allocation.pool_id(),
            before,
            after,
            backed,
            deficit,
        });
        *needed = needed
            .checked_sub(quantity)
            .map_err(|_| OperationError::RecoveryRequired)?;
    }
    if needed.values().any(|value| *value != StockTicks::ZERO) {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(changes)
}

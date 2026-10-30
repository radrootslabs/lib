//! Net amendment claims. Negative changes cannot finance preparatory holds.
use super::{
    AllocationClaim, AllocationPlanError, OptionPoolMapping, PoolHoldProposal, PoolSnapshot,
    StockAllocation, StockTicks,
};
use crate::{Decimal, Quantity, SupplyCommitment, Terms};
use alloc::{collections::BTreeMap, vec::Vec};
use core::fmt;

/// Resolves complete replacement draws against retained original line/pool
/// quantities. Automatic allocation keeps an existing single pool, even when
/// a listing mapping now names another pool. An unchanged multi-pool draw is
/// retained exactly; changing its split requires explicit complete allocation.
/// Mappings for existing lines must come from the retained preparation, with
/// consumed quantities and original provenance separately checked by execution.
pub fn resolve_replacement_allocations(
    terms: &Terms,
    mappings: &[OptionPoolMapping],
    pools: &[PoolSnapshot],
    explicit: Option<&[AllocationClaim]>,
    current: &[StockAllocation],
) -> Result<Vec<StockAllocation>, AllocationPlanError> {
    plan_replacement_allocations(current, current, pools)?;
    let observed = if matches!(terms.supply(), SupplyCommitment::FuturePromise(_)) {
        &[][..]
    } else {
        pools
    };
    let desired = super::resolve_allocations(terms, mappings, observed, explicit)?;
    if current.len() > 64 {
        return Err(AllocationPlanError::Capacity);
    }
    if explicit.is_some() {
        return Ok(desired);
    }
    let pools: BTreeMap<_, _> = pools.iter().map(|pool| (pool.id(), *pool)).collect();
    let mut result = Vec::new();
    for target in desired {
        let originals: Vec<_> = current
            .iter()
            .filter(|old| old.line_id() == target.line_id())
            .copied()
            .collect();
        let target_pool = pools
            .get(&target.pool_id())
            .ok_or(AllocationPlanError::StaleState)?;
        let quantity = Quantity::new(
            Decimal::from_parts(target.ticks().get(), target_pool.unit().scale())
                .map_err(|_| AllocationPlanError::Capacity)?,
            target_pool.unit().unit(),
        )
        .map_err(|_| AllocationPlanError::InvalidInput)?;
        match originals.as_slice() {
            [] => result.push(target),
            [original] => {
                let original_pool = pools
                    .get(&original.pool_id())
                    .ok_or(AllocationPlanError::StaleState)?;
                let converted = quantity
                    .convert(original_pool.unit().unit())
                    .map_err(|_| AllocationPlanError::InvalidInput)?;
                result.push(StockAllocation::new(
                    target.line_id(),
                    original.pool_id(),
                    original_pool
                        .unit()
                        .ticks_for(converted)
                        .map_err(|_| AllocationPlanError::InvalidInput)?,
                )?);
            }
            _ => {
                let mut quantity = Quantity::new(
                    Decimal::from_parts(0, 0).map_err(|_| AllocationPlanError::InvalidInput)?,
                    target_pool.unit().unit(),
                )
                .map_err(|_| AllocationPlanError::InvalidInput)?;
                for original in &originals {
                    let original_pool = pools
                        .get(&original.pool_id())
                        .ok_or(AllocationPlanError::StaleState)?;
                    let converted = Quantity::new(
                        Decimal::from_parts(original.ticks().get(), original_pool.unit().scale())
                            .map_err(|_| AllocationPlanError::Capacity)?,
                        original_pool.unit().unit(),
                    )
                    .and_then(|value| value.convert(target_pool.unit().unit()))
                    .map_err(|_| AllocationPlanError::InvalidInput)?;
                    quantity = quantity
                        .checked_add(converted)
                        .map_err(|_| AllocationPlanError::Capacity)?;
                }
                if target_pool
                    .unit()
                    .ticks_for(quantity)
                    .map_err(|_| AllocationPlanError::InvalidInput)?
                    != target.ticks()
                {
                    return Err(AllocationPlanError::Unsupported);
                }
                result.extend(originals);
            }
        }
    }
    if result.len() > 64 {
        return Err(AllocationPlanError::Capacity);
    }
    result.sort_by_key(|value| (value.pool_id(), value.line_id()));
    Ok(result)
}

/// Checked net changes over claimed current and desired residual draws.
/// Original provenance, retained mappings, already consumed quantities and
/// deficit/backing composition must be authenticated by the executor. This
/// value neither acquires positives nor releases negatives or heals deficits.
pub struct ReplacementAllocationPlan {
    additional: Vec<StockAllocation>,
    releases: Vec<StockAllocation>,
    holds: Vec<PoolHoldProposal>,
}
impl ReplacementAllocationPlan {
    /// Positive residual differences to acquire before replacement issuance.
    pub fn additional_allocations(&self) -> &[StockAllocation] {
        &self.additional
    }
    /// Negative residual differences to resolve against original allocations
    /// only when the mutually authorized replacement actually installs.
    pub fn pending_releases(&self) -> &[StockAllocation] {
        &self.releases
    }
    /// Atomic available-to-preparatory proposals, without spending any pending
    /// negative delta. Current pool/permission checks still belong to the store.
    pub fn preparatory_holds(&self) -> &[PoolHoldProposal] {
        &self.holds
    }
}
impl fmt::Debug for ReplacementAllocationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReplacementAllocationPlan(<private proposal>)")
    }
}

/// Compares complete aggregate current and desired residual line/pool draws.
/// Inputs contain at most64 unique positive pairs each and96 distinct versioned
/// pools. They are already normalized to each pool's fixed ticks; resolving
/// full new terms and subtracting consumed quantities happens before this call.
/// Current includes deficits as outstanding obligations: unchanged quantities
/// never receive replacement backing implicitly. All positive pool holds must
/// succeed together; negative deltas remain data and never alter these holds.
pub fn plan_replacement_allocations(
    current: &[StockAllocation],
    desired: &[StockAllocation],
    pools: &[PoolSnapshot],
) -> Result<ReplacementAllocationPlan, AllocationPlanError> {
    if current.len() > 64 || desired.len() > 64 || pools.len() > 96 {
        return Err(AllocationPlanError::Capacity);
    }
    let mut observed = BTreeMap::new();
    for pool in pools {
        if pool.version().get() == 0 || observed.insert(pool.id(), *pool).is_some() {
            return Err(AllocationPlanError::InvalidInput);
        }
    }
    let mut before = BTreeMap::new();
    let mut after = BTreeMap::new();
    for (values, target) in [(current, &mut before), (desired, &mut after)] {
        for value in values {
            if !observed.contains_key(&value.pool_id()) {
                return Err(AllocationPlanError::StaleState);
            }
            if target
                .insert((value.pool_id(), value.line_id()), value.ticks())
                .is_some()
            {
                return Err(AllocationPlanError::InvalidInput);
            }
        }
    }
    let mut keys: Vec<_> = before.keys().chain(after.keys()).copied().collect();
    keys.sort();
    keys.dedup();
    let mut additional = Vec::new();
    let mut releases = Vec::new();
    let mut quantities = BTreeMap::new();
    for (pool, line) in keys {
        let old = before
            .get(&(pool, line))
            .copied()
            .unwrap_or(StockTicks::ZERO);
        let new = after
            .get(&(pool, line))
            .copied()
            .unwrap_or(StockTicks::ZERO);
        if new > old {
            let quantity = new
                .checked_sub(old)
                .map_err(|_| AllocationPlanError::InvalidInput)?;
            additional.push(StockAllocation::new(line, pool, quantity)?);
            let total = quantities.entry(pool).or_insert(StockTicks::ZERO);
            *total = total
                .checked_add(quantity)
                .map_err(|_| AllocationPlanError::Capacity)?;
        } else if old > new {
            releases.push(StockAllocation::new(
                line,
                pool,
                old.checked_sub(new)
                    .map_err(|_| AllocationPlanError::InvalidInput)?,
            )?);
        }
    }
    let mut holds = Vec::with_capacity(quantities.len());
    for (pool, quantity) in quantities {
        holds.push(PoolHoldProposal::new(observed[&pool], quantity)?);
    }
    Ok(ReplacementAllocationPlan {
        additional,
        releases,
        holds,
    })
}

//! Portable complete stock-allocation proposals. These are unauthenticated
//! values; only a qualified executor can acquire actual stock in a transaction.
use super::{OptionMappingKey, OptionPoolMapping, PoolSnapshot, StockTicks};
use crate::{AgreementLineId, AgreementPrice, PoolId, Quantity, Sequence, SupplyCommitment, Terms};
use alloc::{collections::BTreeMap, vec, vec::Vec};
use core::fmt;

/// A positive seller allocation claim before current pool/version validation.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct AllocationClaim {
    line: AgreementLineId,
    pool: PoolId,
    quantity: Quantity,
    expected: Sequence,
}
impl AllocationClaim {
    /// Captures data only; no participant, mapping or stock is authenticated.
    pub fn new(
        line: AgreementLineId,
        pool: PoolId,
        quantity: Quantity,
        expected: Sequence,
    ) -> Result<Self, AllocationPlanError> {
        if quantity.amount().is_zero() || expected.get() == 0 {
            return Err(AllocationPlanError::InvalidInput);
        }
        Ok(Self {
            line,
            pool,
            quantity,
            expected,
        })
    }
    /// Exact claimed agreement line.
    pub const fn line_id(self) -> AgreementLineId {
        self.line
    }
    /// Exact claimed private pool.
    pub const fn pool_id(self) -> PoolId {
        self.pool
    }
    /// Explicit positive quantity with its declared unit.
    pub const fn quantity(self) -> Quantity {
        self.quantity
    }
    /// Positive expected current pool version.
    pub const fn expected_pool_version(self) -> Sequence {
        self.expected
    }
}
/// Exact positive proposed line/pool draw, without a store hold or consent.
#[derive(Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct StockAllocation {
    line: AgreementLineId,
    pool: PoolId,
    ticks: StockTicks,
}
impl StockAllocation {
    /// Captures only a positive proposed draw. Execution is always separate.
    pub fn new(
        line: AgreementLineId,
        pool: PoolId,
        ticks: StockTicks,
    ) -> Result<Self, AllocationPlanError> {
        if ticks == StockTicks::ZERO {
            return Err(AllocationPlanError::InvalidInput);
        }
        Ok(Self { line, pool, ticks })
    }
    /// Exact proposed line identity.
    pub const fn line_id(self) -> AgreementLineId {
        self.line
    }
    /// Exact proposed private pool identity.
    pub const fn pool_id(self) -> PoolId {
        self.pool
    }
    /// Exact positive pool ticks to acquire.
    pub const fn ticks(self) -> StockTicks {
        self.ticks
    }
}
/// Conservation-checked before/after data; construction never acquires stock.
#[derive(Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct PoolHoldProposal {
    before: PoolSnapshot,
    after: PoolSnapshot,
    quantity: StockTicks,
}
impl PoolHoldProposal {
    /// Proposes available-to-preparatory transfer and one positive version step.
    pub fn new(before: PoolSnapshot, quantity: StockTicks) -> Result<Self, AllocationPlanError> {
        if before.version().get() == 0 || quantity == StockTicks::ZERO {
            return Err(AllocationPlanError::InvalidInput);
        }
        let balances = before
            .balances()
            .hold_available(quantity)
            .map_err(|_| AllocationPlanError::StaleState)?;
        let version = Sequence::new(
            before
                .version()
                .get()
                .checked_add(1)
                .ok_or(AllocationPlanError::Capacity)?,
        )
        .map_err(|_| AllocationPlanError::Capacity)?;
        Ok(Self {
            before,
            after: PoolSnapshot::new(before.id(), before.unit(), balances, version),
            quantity,
        })
    }
    /// Exact observed data claim before the proposed transfer.
    pub const fn before(self) -> PoolSnapshot {
        self.before
    }
    /// Exact proposed after-image, not a confirmed database state.
    pub const fn after(self) -> PoolSnapshot {
        self.after
    }
    /// Positive quantity proposed for preparatory backing.
    pub const fn quantity(self) -> StockTicks {
        self.quantity
    }
}
/// Input-free complete-allocation failure; no source or private values retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AllocationPlanError {
    /// Missing, ambiguous, incompatible, inexact or incomplete allocation data.
    InvalidInput,
    /// A supplied current version/unit or available quantity cannot support it.
    StaleState,
    /// A future supply or price profile is not supported.
    Unsupported,
    /// Exact quantity, aggregate, collection or version capacity is exceeded.
    Capacity,
}
impl fmt::Display for AllocationPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "invalid complete allocation",
            Self::StaleState => "allocation state differs",
            Self::Unsupported => "unsupported allocation profile",
            Self::Capacity => "allocation capacity exceeded",
        })
    }
}
impl core::error::Error for AllocationPlanError {}
macro_rules! redacted { ($($name:ident),+) => { $(impl fmt::Debug for $name { fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result { f.write_str(concat!(stringify!($name),"(<private proposal>)")) } })+ }; }
redacted!(AllocationClaim, StockAllocation, PoolHoldProposal);

#[cfg(feature = "serde")]
mod serialization {
    use super::*;
    use serde::Deserialize;
    impl<'de> Deserialize<'de> for StockAllocation {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Wire {
                line: AgreementLineId,
                pool: PoolId,
                ticks: StockTicks,
            }
            let wire: Wire = super::super::serialization::object(deserializer)?;
            Self::new(wire.line, wire.pool, wire.ticks).map_err(serde::de::Error::custom)
        }
    }
    impl<'de> Deserialize<'de> for PoolHoldProposal {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Wire {
                before: PoolSnapshot,
                after: PoolSnapshot,
                quantity: StockTicks,
            }
            let wire: Wire = super::super::serialization::object(deserializer)?;
            let value = Self::new(wire.before, wire.quantity).map_err(serde::de::Error::custom)?;
            if value.after != wire.after {
                return Err(serde::de::Error::custom("pool hold proposal differs"));
            }
            Ok(value)
        }
    }
}

/// Resolves complete deterministic allocation data with checked exact units and
/// versioned before/after proposals. `None` selects current mappings; `Some`
/// requires one through64 distinct positive line/pool claims with complete line
/// coverage. At most96 distinct pool observations are accepted. Only explicit
/// future-promise terms allow no physical draws. This function performs no IO,
/// authenticates no source/grant/version and acquires no stock.
pub fn plan_allocations(
    terms: &Terms,
    mappings: &[OptionPoolMapping],
    pools: &[PoolSnapshot],
    explicit: Option<&[AllocationClaim]>,
) -> Result<(Vec<StockAllocation>, Vec<PoolHoldProposal>), AllocationPlanError> {
    let result = resolve_allocations(terms, mappings, pools, explicit)?;
    let available: BTreeMap<_, _> = pools.iter().map(|pool| (pool.id(), *pool)).collect();
    let mut totals = BTreeMap::new();
    for allocation in &result {
        let total = totals.entry(allocation.pool).or_insert(StockTicks::ZERO);
        *total = total
            .checked_add(allocation.ticks)
            .map_err(|_| AllocationPlanError::Capacity)?;
    }
    let mut effects = Vec::new();
    for (id, quantity) in totals {
        effects.push(PoolHoldProposal::new(available[&id], quantity)?);
    }
    Ok((result, effects))
}

/// Resolves complete line/pool quantities using the supplied exact mappings,
/// units and versions, without requiring or proposing new available stock.
/// An amendment executor supplies retained mappings for existing draws. It
/// must separately compare current residual/original allocations and acquire
/// positive deltas; successful resolution alone grants no physical backing.
pub fn resolve_allocations(
    terms: &Terms,
    mappings: &[OptionPoolMapping],
    pools: &[PoolSnapshot],
    explicit: Option<&[AllocationClaim]>,
) -> Result<Vec<StockAllocation>, AllocationPlanError> {
    match terms.supply() {
        SupplyCommitment::FuturePromise(_) => {
            if explicit.is_some() || !mappings.is_empty() || !pools.is_empty() {
                return Err(AllocationPlanError::InvalidInput);
            }
            return Ok(vec![]);
        }
        SupplyCommitment::StockBacked => {}
    }
    if mappings.len() != terms.lines().len() || pools.len() > 96 {
        return Err(AllocationPlanError::InvalidInput);
    }
    if let Some(choices) = explicit {
        if choices.is_empty() || choices.len() > 64 {
            return Err(AllocationPlanError::InvalidInput);
        }
        for (index, choice) in choices.iter().enumerate() {
            if choices[..index]
                .iter()
                .any(|prior| prior.line == choice.line && prior.pool == choice.pool)
            {
                return Err(AllocationPlanError::InvalidInput);
            }
        }
    }
    let mut available = BTreeMap::new();
    for pool in pools {
        if pool.version().get() == 0 || available.insert(pool.id(), *pool).is_some() {
            return Err(AllocationPlanError::InvalidInput);
        }
    }
    let mut result = Vec::new();
    for (line, mapping) in terms.lines().iter().zip(mappings) {
        if mapping.key() != OptionMappingKey::new(line.source().coordinate(), line.option_id())
            || available
                .get(&mapping.pool_id())
                .is_none_or(|pool| pool.unit() != mapping.pool_unit())
        {
            return Err(AllocationPlanError::StaleState);
        }
        let required = match line.price() {
            AgreementPrice::Fixed(_) => mapping
                .draw_for_sales(line.amount())
                .map_err(|_| AllocationPlanError::InvalidInput)?,
            AgreementPrice::VariableWeight(weight) => {
                // Bounds describe this entire line. Never multiply by package
                // count or infer a mass/volume conversion from its label.
                let quantity = Quantity::new(weight.maximum().amount(), weight.measurement_unit())
                    .and_then(|value| value.convert(mapping.pool_unit().unit()))
                    .map_err(|_| AllocationPlanError::InvalidInput)?;
                mapping
                    .pool_unit()
                    .ticks_for(quantity)
                    .map_err(|_| AllocationPlanError::InvalidInput)?
            }
        };
        if required == StockTicks::ZERO {
            return Err(AllocationPlanError::InvalidInput);
        }
        if let Some(explicit) = explicit {
            let mut covered = StockTicks::ZERO;
            for choice in explicit.iter().filter(|value| value.line_id() == line.id()) {
                let pool = available
                    .get(&choice.pool_id())
                    .ok_or(AllocationPlanError::StaleState)?;
                if pool.version() != choice.expected_pool_version() {
                    return Err(AllocationPlanError::StaleState);
                }
                let normalized = choice
                    .quantity()
                    .convert(mapping.pool_unit().unit())
                    .map_err(|_| AllocationPlanError::InvalidInput)?;
                covered = covered
                    .checked_add(
                        mapping
                            .pool_unit()
                            .ticks_for(normalized)
                            .map_err(|_| AllocationPlanError::InvalidInput)?,
                    )
                    .map_err(|_| AllocationPlanError::Capacity)?;
                let quantity = choice
                    .quantity()
                    .convert(pool.unit().unit())
                    .map_err(|_| AllocationPlanError::InvalidInput)?;
                let ticks = pool
                    .unit()
                    .ticks_for(quantity)
                    .map_err(|_| AllocationPlanError::InvalidInput)?;
                result.push(StockAllocation {
                    line: line.id(),
                    pool: pool.id(),
                    ticks,
                });
            }
            if covered != required {
                return Err(AllocationPlanError::InvalidInput);
            }
        } else {
            result.push(StockAllocation {
                line: line.id(),
                pool: mapping.pool_id(),
                ticks: required,
            });
        }
    }
    if let Some(explicit) = explicit
        && result.len() != explicit.len()
    {
        return Err(AllocationPlanError::InvalidInput);
    }
    if result.is_empty() || result.len() > 64 {
        return Err(AllocationPlanError::InvalidInput);
    }
    result.sort_by_key(|value| (value.pool, value.line));
    Ok(result)
}

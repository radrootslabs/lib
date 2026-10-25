//! Store-neutral planning adapter. Domain owns all allocation/conservation rules.
use super::*;
pub(super) use radroots_domain::stock::{
    PoolHoldProposal as Effect, StockAllocation as Allocation,
};
pub(super) fn resolve(
    terms: &radroots_domain::Terms,
    command: &CapturedOrderAcceptance,
    mappings: &[radroots_domain::stock::OptionPoolMapping],
    pools: &[PoolSnapshot],
) -> Result<(Vec<Allocation>, Vec<Effect>), OperationError> {
    use radroots_domain::stock::{AllocationClaim, AllocationPlanError};
    let explicit = command
        .input()
        .allocation()
        .explicit_allocations()
        .map(|values| {
            values
                .iter()
                .map(|value| {
                    AllocationClaim::new(
                        value.line_id(),
                        value.pool_id(),
                        value.quantity(),
                        value.expected_pool_version(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()
        .map_err(|_| OperationError::InvalidCommand)?;
    radroots_domain::stock::plan_allocations(terms, mappings, pools, explicit.as_deref()).map_err(
        |error| match error {
            AllocationPlanError::InvalidInput => OperationError::InvalidCommand,
            AllocationPlanError::StaleState => OperationError::StaleState,
            AllocationPlanError::Unsupported => OperationError::UnsupportedOperation,
            AllocationPlanError::Capacity => WorkspaceError::Capacity.into(),
            _ => OperationError::RecoveryRequired,
        },
    )
}

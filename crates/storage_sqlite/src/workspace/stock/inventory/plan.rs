use super::*;

pub(super) fn record_version(catalog: i64) -> Result<u8, OperationError> {
    match catalog {
        2 => Ok(1),
        3..=15 => Ok(2),
        _ => Err(OperationError::RecoveryRequired),
    }
}
pub(super) fn plan_effect(
    command: &radroots_storage::inventory::CapturedInventoryCommand,
    before: Option<PoolSnapshot>,
) -> Result<
    (
        PoolSnapshot,
        radroots_domain::stock::StockTicks,
        MovementKind,
    ),
    OperationError,
> {
    use radroots_domain::stock::{AvailableStockCorrection, StockError};
    use radroots_storage::inventory::CapturedInventoryCommand;
    match command {
        CapturedInventoryCommand::MapOption(command) => {
            let pool = before.ok_or(OperationError::StaleState)?;
            if pool.id() != command.input().pool_id() {
                return Err(OperationError::RecoveryRequired);
            }
            Ok((
                pool,
                radroots_domain::stock::StockTicks::ZERO,
                MovementKind::Introduced,
            ))
        }
        CapturedInventoryCommand::CreatePool(command) => {
            if before.is_some() {
                return Err(OperationError::StaleState);
            }
            let input = command.input();
            let quantity = input.initial_ticks();
            let balances = PoolBalances::new(
                quantity,
                PhysicalBalances {
                    available: quantity,
                    ..PhysicalBalances::ZERO
                },
            )
            .map_err(|_| OperationError::InvalidCommand)?;
            Ok((
                PoolSnapshot::new(
                    input.id(),
                    input.unit(),
                    balances,
                    Sequence::new(1).map_err(|_| OperationError::RecoveryRequired)?,
                ),
                quantity,
                MovementKind::Introduced,
            ))
        }
        CapturedInventoryCommand::CorrectPool(command) => {
            let input = command.input();
            let before = before.ok_or(OperationError::StaleState)?;
            if before.id() != input.pool_id() || before.version() != input.expected_version() {
                return Err(OperationError::StaleState);
            }
            let quantity = before
                .unit()
                .ticks_for(
                    radroots_domain::Quantity::new(input.amount().amount(), before.unit().unit())
                        .map_err(|_| OperationError::InvalidCommand)?,
                )
                .map_err(|_| OperationError::InvalidCommand)?;
            let balances = before
                .balances()
                .correct_available(input.kind(), quantity)
                .map_err(|error| match error {
                    StockError::Underflow => OperationError::StaleState,
                    StockError::OutOfRange => WorkspaceError::Capacity.into(),
                    _ => OperationError::InvalidCommand,
                })?;
            let version = Sequence::new(
                before
                    .version()
                    .get()
                    .checked_add(1)
                    .ok_or(WorkspaceError::Capacity)?,
            )
            .map_err(|_| WorkspaceError::Capacity)?;
            let movement = match input.kind() {
                AvailableStockCorrection::Add => MovementKind::Introduced,
                AvailableStockCorrection::Remove => MovementKind::Removed,
                AvailableStockCorrection::Spoil => MovementKind::Wasted,
            };
            Ok((
                PoolSnapshot::new(before.id(), before.unit(), balances, version),
                quantity,
                movement,
            ))
        }
    }
}

//! Current order selectors are derived cache data, never business authority.
use super::*;
use radroots_domain::{OperationId, OrderId, PublicKey};
use radroots_storage::command::OperationError;
use sqlx::Row;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Phase {
    Prepared,
    Installed,
    Aborted,
}
impl Phase {
    fn code(self) -> i64 {
        match self {
            Self::Prepared => 0,
            Self::Installed => 1,
            Self::Aborted => 2,
        }
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct Marker {
    pub order: OrderId,
    pub actor: PublicKey,
    pub operation: OperationId,
    pub entry: OperationId,
    pub revision: Sequence,
    pub phase: Phase,
}
impl Marker {
    /// Called only with a completely authenticated family-specific entry.
    pub fn from_history(entry: &ledger::HistoryEntry) -> Result<Option<Self>, OperationError> {
        let phase = match entry.header.family {
            ledger::Family::Acceptance => Phase::Prepared,
            ledger::Family::Installation => Phase::Installed,
            ledger::Family::AbortedPreparation => Phase::Aborted,
            _ => return Ok(None),
        };
        Self::from_header(&entry.header, phase).map(Some)
    }
    pub fn from_header(header: &ledger::Header, phase: Phase) -> Result<Self, OperationError> {
        let family = match phase {
            Phase::Prepared => 2,
            Phase::Installed => 6,
            Phase::Aborted => 7,
        };
        if header.family.code() != family {
            return Err(OperationError::RecoveryRequired);
        }
        let operation = match phase {
            Phase::Prepared => header.operation,
            Phase::Installed | Phase::Aborted => header
                .related_operation
                .ok_or(OperationError::RecoveryRequired)?,
        };
        Ok(Self {
            order: header.order.ok_or(OperationError::RecoveryRequired)?,
            actor: header.actor.participant(),
            operation,
            entry: header.operation,
            revision: header.revision,
            phase,
        })
    }
}

pub(super) struct Witness {
    pub order: OrderId,
    pub expected: Option<Marker>,
}
impl Witness {
    /// Compare the complete selected raw cache row after all provider IO.
    pub async fn confirm(&self, connection: &mut SqliteConnection) -> Result<(), OperationError> {
        let row = sqlx::query("SELECT actor,operation_id,entry_id,revision,phase FROM stock_order_states WHERE order_id=?")
            .bind(self.order.as_bytes().as_slice()).fetch_optional(connection).await
            .map_err(|_| OperationError::RecoveryRequired)?;
        let actual = if let Some(row) = row {
            let bytes = |column| {
                row.try_get::<Vec<u8>, _>(column)
                    .map_err(|_| OperationError::RecoveryRequired)?
                    .try_into()
                    .map_err(|_| OperationError::RecoveryRequired)
            };
            let revision: i64 = row
                .try_get(3)
                .map_err(|_| OperationError::RecoveryRequired)?;
            let phase: i64 = row
                .try_get(4)
                .map_err(|_| OperationError::RecoveryRequired)?;
            if revision <= 0 {
                return Err(OperationError::RecoveryRequired);
            }
            Some(Marker {
                order: self.order,
                actor: PublicKey::from_bytes(bytes(0)?)
                    .map_err(|_| OperationError::RecoveryRequired)?,
                operation: OperationId::from_bytes(bytes(1)?),
                entry: OperationId::from_bytes(bytes(2)?),
                revision: Sequence::new(revision as u64)
                    .map_err(|_| OperationError::RecoveryRequired)?,
                phase: match phase {
                    0 => Phase::Prepared,
                    1 => Phase::Installed,
                    2 => Phase::Aborted,
                    _ => return Err(OperationError::RecoveryRequired),
                },
            })
        } else {
            None
        };
        if actual != self.expected {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
    /// The owning BEGIN IMMEDIATE transaction must retain the bound history
    /// and root witnesses as well. This row supplies no independent authority.
    pub async fn advance(
        &self,
        connection: &mut SqliteConnection,
        next: Marker,
    ) -> Result<(), OperationError> {
        if next.order != self.order
            || self
                .expected
                .is_some_and(|old| next.revision <= old.revision)
        {
            return Err(OperationError::RecoveryRequired);
        }
        let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&mut *connection)
            .await
            .map_err(|_| OperationError::RecoveryRequired)?;
        let legal = match self.expected {
            None => next.phase == Phase::Prepared,
            Some(old) if old.actor == next.actor => match (old.phase, next.phase) {
                (Phase::Prepared, Phase::Installed | Phase::Aborted) => {
                    old.operation == next.operation
                }
                (Phase::Aborted, Phase::Prepared) => old.operation != next.operation,
                (Phase::Installed, Phase::Prepared) if catalog >= 20 => {
                    old.operation != next.operation
                }
                _ => false,
            },
            Some(_) => false,
        };
        if !legal {
            return Err(OperationError::RecoveryRequired);
        }
        self.confirm(connection).await?;
        let result = if let Some(old) = self.expected {
            sqlx::query("UPDATE stock_order_states SET actor=?,operation_id=?,entry_id=?,revision=?,phase=? WHERE order_id=? AND actor=? AND operation_id=? AND entry_id=? AND revision=? AND phase=?")
                .bind(next.actor.as_bytes().as_slice()).bind(next.operation.as_bytes().as_slice())
                .bind(next.entry.as_bytes().as_slice()).bind(next.revision.get() as i64)
                .bind(next.phase.code()).bind(self.order.as_bytes().as_slice())
                .bind(old.actor.as_bytes().as_slice()).bind(old.operation.as_bytes().as_slice())
                .bind(old.entry.as_bytes().as_slice()).bind(old.revision.get() as i64)
                .bind(old.phase.code()).execute(connection).await
        } else {
            sqlx::query("INSERT INTO stock_order_states(order_id,actor,operation_id,entry_id,revision,phase) VALUES(?,?,?,?,?,?)")
                .bind(next.order.as_bytes().as_slice()).bind(next.actor.as_bytes().as_slice())
                .bind(next.operation.as_bytes().as_slice()).bind(next.entry.as_bytes().as_slice())
                .bind(next.revision.get() as i64).bind(next.phase.code()).execute(connection).await
        }.map_err(|_| OperationError::StorageFailure)?;
        if result.rows_affected() != 1 {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}

/// Constant-memory backward validation of retained decision transitions. Each
/// original preparation remains independently recoverable after a later abort.
pub(super) struct Chain {
    pub latest: Option<Marker>,
    previous: Previous,
    replacements: bool,
}
enum Previous {
    Latest,
    Preparation(OperationId),
    AbortOrGenesis(OperationId),
}
impl Chain {
    pub const fn new(catalog: i64) -> Self {
        Self {
            latest: None,
            previous: Previous::Latest,
            replacements: catalog >= 20,
        }
    }
    pub fn observe(&mut self, marker: Marker) -> Result<(), OperationError> {
        if self.latest.is_none() {
            self.latest = Some(marker);
        }
        match self.previous {
            Previous::Latest => {}
            Previous::Preparation(operation)
                if marker.phase == Phase::Prepared && marker.operation == operation => {}
            Previous::AbortOrGenesis(operation)
                if (marker.phase == Phase::Aborted
                    || (self.replacements && marker.phase == Phase::Installed))
                    && marker.operation != operation => {}
            _ => return Err(OperationError::RecoveryRequired),
        }
        self.previous = match marker.phase {
            Phase::Prepared => Previous::AbortOrGenesis(marker.operation),
            Phase::Installed | Phase::Aborted => Previous::Preparation(marker.operation),
        };
        Ok(())
    }
    pub fn complete(&self) -> Result<(), OperationError> {
        match self.previous {
            Previous::Latest | Previous::AbortOrGenesis(_) => Ok(()),
            Previous::Preparation(_) => Err(OperationError::RecoveryRequired),
        }
    }
}

pub(super) struct Checked {
    pub witness: Witness,
    pub ancestry: inventory::checkpoint::MinimumProof,
}
impl Checked {
    pub async fn confirm(&self, connection: &mut SqliteConnection) -> Result<(), OperationError> {
        Box::pin(self.ancestry.confirm(connection)).await?;
        self.witness.confirm(connection).await
    }
    pub async fn advance(
        &self,
        connection: &mut SqliteConnection,
        next: Marker,
    ) -> Result<(), OperationError> {
        self.confirm(connection).await?;
        self.witness.advance(connection, next).await
    }
}

/// Order history eligibility only. The caller still confirms current consent,
/// exact intent, deadline, provider capability and this proof before dispatch.
pub(super) async fn signing_state(
    turn: &mut inventory::Turn<'_>,
    access: &inventory::Access,
    order: OrderId,
    operation: OperationId,
    allow_installed_copy: bool,
) -> Result<Option<Checked>, OperationError> {
    if access.authority.catalog < 13 {
        return Ok(None);
    }
    let checked = Box::pin(inventory::checkpoint::authenticate_order_state(
        turn.connection,
        turn.selected,
        turn.crypto,
        &access.current,
        &access.authority,
        order,
    ))
    .await?;
    if checked.witness.expected.is_none_or(|value| {
        value.operation != operation
            || !(value.phase == Phase::Prepared
                || (allow_installed_copy && value.phase == Phase::Installed))
    }) {
        return Err(OperationError::StaleState);
    }
    Ok(Some(checked))
}

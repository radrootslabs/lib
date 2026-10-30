//! Bounded traversal of actual original T0/T2 allocations, shared by loss
//! planning and current inspection. SQL pointers never establish the claim.
use super::*;
use radroots_domain::stock::{ObligationBacking, StockAllocation};

pub(super) struct InstalledAllocation {
    pub acceptance: OperationId,
    pub installation: OperationId,
    pub allocation: StockAllocation,
}
impl InstalledAllocation {
    pub async fn backing(
        &self,
        connection: &mut SqliteConnection,
        input: &AttachmentInput,
        crypto: &WorkspaceCrypto,
        before_revision: u64,
    ) -> Result<ObligationBacking, OperationError> {
        let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&mut *connection)
            .await
            .map_err(|_| OperationError::RecoveryRequired)?;
        if catalog >= 20 {
            return super::changes::current(connection, input, crypto, self, before_revision).await;
        }
        let prior = super::deficit::prior_quantity(
            connection,
            input,
            crypto,
            self.acceptance,
            self.installation,
            self.allocation.line_id(),
            self.allocation.pool_id(),
            before_revision,
        )
        .await?;
        let backed = self
            .allocation
            .ticks()
            .checked_sub(prior)
            .map_err(|_| OperationError::RecoveryRequired)?;
        ObligationBacking::new(self.allocation.ticks(), backed, prior)
            .map_err(|_| OperationError::RecoveryRequired)
    }
}

pub(super) struct InstalledCursor {
    pool: PoolId,
    before_revision: u64,
    revision: u64,
    acceptance: Option<OperationId>,
    installation: Option<OperationId>,
    pending: std::vec::IntoIter<StockAllocation>,
}
impl InstalledCursor {
    pub fn new(pool: PoolId, before_revision: u64) -> Result<Self, OperationError> {
        if before_revision == 0 || before_revision > i64::MAX as u64 {
            return Err(WorkspaceError::Capacity.into());
        }
        Ok(Self {
            pool,
            before_revision,
            revision: 0,
            acceptance: None,
            installation: None,
            pending: Vec::new().into_iter(),
        })
    }
    pub async fn next(
        &mut self,
        connection: &mut SqliteConnection,
        input: &AttachmentInput,
        crypto: &WorkspaceCrypto,
    ) -> Result<Option<InstalledAllocation>, OperationError> {
        loop {
            if let Some(allocation) = self.pending.next() {
                if allocation.pool_id() != self.pool {
                    continue;
                }
                return Ok(Some(InstalledAllocation {
                    acceptance: self.acceptance.ok_or(OperationError::RecoveryRequired)?,
                    installation: self.installation.ok_or(OperationError::RecoveryRequired)?,
                    allocation,
                }));
            }
            let row = sqlx::query("SELECT revision,CASE WHEN length(operation_id)=32 THEN operation_id END FROM stock_operations WHERE family=6 AND revision>? AND revision<? ORDER BY revision LIMIT 1")
                .bind(self.revision as i64).bind(self.before_revision as i64)
                .fetch_optional(&mut *connection).await.map_err(|_| OperationError::RecoveryRequired)?;
            let Some(row) = row else {
                return Ok(None);
            };
            let revision: i64 = row
                .try_get(0)
                .map_err(|_| OperationError::RecoveryRequired)?;
            let entry: Vec<u8> = row
                .try_get::<Option<Vec<u8>>, _>(1)
                .map_err(|_| OperationError::RecoveryRequired)?
                .ok_or(OperationError::RecoveryRequired)?;
            if revision <= 0 {
                return Err(OperationError::RecoveryRequired);
            }
            self.revision = revision as u64;
            let entry = OperationId::from_bytes(
                entry
                    .try_into()
                    .map_err(|_| OperationError::RecoveryRequired)?,
            );
            let installed = Box::pin(super::installation::read_entry(
                connection, input, crypto, entry,
            ))
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
            if installed.frame.header.revision.get() != self.revision
                || installed.frame.header.actor.participant() != input.participant()
            {
                return Err(OperationError::RecoveryRequired);
            }
            let acceptance = installed
                .frame
                .header
                .related_operation
                .ok_or(OperationError::RecoveryRequired)?;
            let original = Box::pin(super::acceptance::codec::read(
                connection, input, crypto, acceptance,
            ))
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
            self.pending = original.original_allocations()?.into_iter();
            self.acceptance = Some(acceptance);
            self.installation = Some(entry);
        }
    }
}

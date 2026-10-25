//! Fresh native stock authority. No allocation or issuance is implied by enrollment.
use super::{
    AttachmentInput, AttachmentProof, NativeWorkspace, WorkspaceBinding, WorkspaceClock,
    WorkspaceCrypto, WorkspaceError, WorkspaceScope, fs::Namespace, record::GrantRecord,
};
use radroots_domain::{ExecutorId, GrantId, Sequence, TransactionId, WorkspaceId};
pub use radroots_storage::workspace::seller_stock::SellerStockConfig;
use radroots_storage::workspace::seller_stock::{
    SellerAuthoritySelection, StockAction, StockAuthorizationRef, StockGrantRecord, StockGrantScope,
};
pub(in crate::workspace) mod consent;
pub use consent::{
    StockConsentCapture, StockConsentMode, StockConsentOutcome, StockConsentReceipt,
};
pub(in crate::workspace) mod grants;
pub use inventory::acceptance::{
    StockAcceptanceOutcome, StockAcceptanceReceipt, StockAcceptanceRequest,
};
mod ledger;
pub(in crate::workspace) mod signing_plan;
pub use signing_plan::{StockBoundSigningPlan, StockSigningPlanOutcome, StockSigningPlanRequest};
pub(in crate::workspace) mod public_source;
pub use public_source::{
    StockListingCapture, StockListingOutcome, StockListingReceipt, StockListingSnapshot,
};
pub(in crate::workspace) mod source;
use grants::StockRecord;
pub use grants::{StockGrantEnrollment, StockGrantOutcome, StockGrantReceipt};
use serde::{Deserialize, Serialize};
pub use source::{
    StockHistorySnapshot, StockRequestSnapshot, StockSourceCapture, StockSourceOutcome,
    StockSourceProfile, StockSourceReceipt, StockSourceSnapshot,
};
use sqlx::{Connection, SqliteConnection};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

/// One-shot protected enrollment, retained before starting durable creation. A lost
/// observer must reopen its original selection, never silently create a replacement.
pub struct StockEnrollment {
    input: AttachmentInput,
    proof: AttachmentProof,
    record: Mutex<Option<GrantRecord>>,
    transaction: TransactionId,
}
impl StockEnrollment {
    pub fn prepare(
        config: SellerStockConfig,
        clock: &dyn WorkspaceClock,
    ) -> Result<Self, WorkspaceError> {
        let now = clock.now()?;
        config.validate_at(now)?;
        let secret = zeroize::Zeroizing::new(super::random()?);
        let proof = AttachmentProof::from_protected_input(secret.as_ref())?;
        let (input, proof, record) = config.enroll(
            now,
            WorkspaceId::from_bytes(super::random()?),
            ExecutorId::from_bytes(super::random()?),
            GrantId::from_bytes(super::random()?),
            proof,
        )?;
        Ok(Self {
            input,
            proof,
            record: Mutex::new(Some(record)),
            transaction: TransactionId::from_bytes(super::random()?),
        })
    }
    pub fn input(&self) -> &AttachmentInput {
        &self.input
    }
    pub fn proof(&self) -> &AttachmentProof {
        &self.proof
    }
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
}
impl std::fmt::Debug for StockEnrollment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockEnrollment(<redacted>)")
    }
}
/// A confirmed creation identity read from protected durable state. This is
/// enrollment evidence, not proof of any stock quantity or business operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StockAuthorityReceipt {
    pub(super) input: AttachmentInput,
    pub(super) transaction: TransactionId,
    pub(super) revocation: Sequence,
    pub(super) inventory: Option<radroots_storage::inventory::InventoryCheckpointRef>,
    pub(super) sources: Option<radroots_storage::workspace::seller_stock::StockSourceCheckpointRef>,
    pub(super) public_sources:
        Option<radroots_storage::workspace::seller_stock::StockListingCheckpointRef>,
}
impl StockAuthorityReceipt {
    /// Independently retained public observation progress; this certifies no
    /// global head, profile readiness or business permission.
    pub const fn listing_checkpoint(
        &self,
    ) -> Option<radroots_storage::workspace::seller_stock::StockListingCheckpointRef> {
        self.public_sources
    }
    /// Separately retained source progress; profile recognition proves no order
    /// actionability. Absent for retained catalogs without source retention.
    pub const fn source_checkpoint(
        &self,
    ) -> Option<radroots_storage::workspace::seller_stock::StockSourceCheckpointRef> {
        self.sources
    }
    pub fn input(&self) -> &AttachmentInput {
        &self.input
    }
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
    /// Current protected inventory checkpoint, absent for enrollment-only catalogs.
    pub const fn inventory_checkpoint(
        &self,
    ) -> Option<radroots_storage::inventory::InventoryCheckpointRef> {
        self.inventory
    }
    pub const fn revocation_generation(&self) -> Sequence {
        self.revocation
    }
}
impl NativeWorkspace {
    pub async fn initialize_seller_stock(
        path: &Path,
        enrollment: &StockEnrollment,
        crypto: WorkspaceCrypto,
        clock: Arc<dyn WorkspaceClock>,
    ) -> Result<Self, WorkspaceError> {
        // Taking the sole prepared grant is irreversible, even if this observer
        // disappears. Retained selectors/proof permit reopening, never cloning authority.
        let record = {
            enrollment
                .record
                .lock()
                .map_err(|_| WorkspaceError::Unavailable)?
                .take()
                .ok_or(WorkspaceError::RecoveryRequired)?
        };
        let binding = record.authenticate(&enrollment.input, &enrollment.proof, clock.now()?)?;
        let worker = super::worker::Handle::launch_stock(
            path.to_path_buf(),
            enrollment.input.clone(),
            Some((record, enrollment.transaction, binding.expires_at())),
            crypto,
            clock,
        )
        .await?;
        Ok(Self {
            worker: Arc::new(worker),
        })
    }
    pub async fn open_seller_stock(
        path: &Path,
        expected: AttachmentInput,
        crypto: WorkspaceCrypto,
        clock: Arc<dyn WorkspaceClock>,
    ) -> Result<Self, WorkspaceError> {
        let worker =
            super::worker::Handle::launch_stock(path.to_path_buf(), expected, None, crypto, clock)
                .await?;
        Ok(Self {
            worker: Arc::new(worker),
        })
    }
    /// Query after an interrupted creation using the retained original selection.
    pub async fn stock_authority(
        &self,
        input: AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<StockAuthorityReceipt, WorkspaceError> {
        let proof = proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?;
        self.worker.stock_authority(input, proof).await
    }
}
#[derive(Clone)]
struct Authority {
    catalog: i64,
    input: AttachmentInput,
    transaction: TransactionId,
    revocation: Sequence,
    inventory: Option<InventoryCheckpoint>,
    authorization: Option<StockAuthorizationRef>,
    sources: Option<SourceCheckpoint>,
    public_sources: Option<PublicSourceCheckpoint>,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicSourceCheckpoint {
    revision: Sequence,
    events: Sequence,
    heads: Sequence,
    parts: Sequence,
    #[serde(deserialize_with = "required_nullable")]
    head: Option<radroots_domain::CheckpointDigest>,
}
impl PublicSourceCheckpoint {
    fn validate(self) -> Result<(), WorkspaceError> {
        if self.revision != self.events
            || (self.revision.get() == 0) != self.head.is_none()
            || (self.revision.get() == 0) != (self.heads.get() == 0)
            || self.heads.get() > self.events.get()
            || self.parts.get() < self.events.get()
            || self.parts.get() > self.events.get().saturating_mul(4)
        {
            return Err(WorkspaceError::RecoveryRequired);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceCheckpoint {
    revision: Sequence,
    records: Sequence,
    parts: Sequence,
    #[serde(deserialize_with = "required_nullable")]
    head: Option<radroots_domain::CheckpointDigest>,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InventoryCheckpointLegacy {
    revision: Sequence,
    pools: Sequence,
    movements: Sequence,
    #[serde(deserialize_with = "required_nullable")]
    head: Option<radroots_domain::CheckpointDigest>,
}
#[derive(Clone, Copy)]
struct InventoryCheckpoint {
    revision: Sequence,
    pools: Sequence,
    movements: Sequence,
    head: Option<radroots_domain::CheckpointDigest>,
    mappings: Option<Sequence>,
    orders: Option<Sequence>,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InventoryCheckpointCurrent {
    revision: Sequence,
    pools: Sequence,
    movements: Sequence,
    mappings: Sequence,
    #[serde(deserialize_with = "required_nullable")]
    head: Option<radroots_domain::CheckpointDigest>,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InventoryCheckpointUnified {
    revision: Sequence,
    pools: Sequence,
    movements: Sequence,
    mappings: Sequence,
    orders: Sequence,
    #[serde(deserialize_with = "required_nullable")]
    head: Option<radroots_domain::CheckpointDigest>,
}
impl From<InventoryCheckpointLegacy> for InventoryCheckpoint {
    fn from(value: InventoryCheckpointLegacy) -> Self {
        Self {
            revision: value.revision,
            pools: value.pools,
            movements: value.movements,
            head: value.head,
            mappings: None,
            orders: None,
        }
    }
}
impl From<InventoryCheckpointCurrent> for InventoryCheckpoint {
    fn from(value: InventoryCheckpointCurrent) -> Self {
        Self {
            revision: value.revision,
            pools: value.pools,
            movements: value.movements,
            head: value.head,
            mappings: Some(value.mappings),
            orders: None,
        }
    }
}
impl From<InventoryCheckpointUnified> for InventoryCheckpoint {
    fn from(value: InventoryCheckpointUnified) -> Self {
        Self {
            revision: value.revision,
            pools: value.pools,
            movements: value.movements,
            head: value.head,
            mappings: Some(value.mappings),
            orders: Some(value.orders),
        }
    }
}
impl InventoryCheckpoint {
    fn legacy(self) -> Result<InventoryCheckpointLegacy, WorkspaceError> {
        if self.mappings.is_some() {
            return Err(WorkspaceError::RecoveryRequired);
        }
        Ok(InventoryCheckpointLegacy {
            revision: self.revision,
            pools: self.pools,
            movements: self.movements,
            head: self.head,
        })
    }
    fn current(self) -> Result<InventoryCheckpointCurrent, WorkspaceError> {
        Ok(InventoryCheckpointCurrent {
            revision: self.revision,
            pools: self.pools,
            movements: self.movements,
            head: self.head,
            mappings: self.mappings.ok_or(WorkspaceError::RecoveryRequired)?,
        })
    }
    fn unified(self) -> Result<InventoryCheckpointUnified, WorkspaceError> {
        Ok(InventoryCheckpointUnified {
            revision: self.revision,
            pools: self.pools,
            movements: self.movements,
            head: self.head,
            mappings: self.mappings.ok_or(WorkspaceError::RecoveryRequired)?,
            orders: self.orders.ok_or(WorkspaceError::RecoveryRequired)?,
        })
    }
}
fn required_nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityV1 {
    version: u8,
    input: AttachmentInput,
    transaction: TransactionId,
    revocation: Sequence,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InventoryAuthorityWire {
    version: u8,
    input: AttachmentInput,
    transaction: TransactionId,
    revocation: Sequence,
    inventory: InventoryCheckpointLegacy,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityV4 {
    version: u8,
    namespace: SellerAuthoritySelection,
    administrator: GrantId,
    transaction: TransactionId,
    revocation: Sequence,
    inventory: InventoryCheckpointLegacy,
    authorization: StockAuthorizationRef,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityV5 {
    version: u8,
    namespace: SellerAuthoritySelection,
    administrator: GrantId,
    transaction: TransactionId,
    revocation: Sequence,
    #[serde(deserialize_with = "object")]
    inventory: InventoryCheckpointCurrent,
    authorization: StockAuthorizationRef,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityV6 {
    version: u8,
    namespace: SellerAuthoritySelection,
    administrator: GrantId,
    transaction: TransactionId,
    revocation: Sequence,
    #[serde(deserialize_with = "object")]
    inventory: InventoryCheckpointCurrent,
    #[serde(deserialize_with = "object")]
    sources: SourceCheckpoint,
    authorization: StockAuthorizationRef,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityV7 {
    version: u8,
    #[serde(deserialize_with = "object")]
    namespace: SellerAuthoritySelection,
    administrator: GrantId,
    transaction: TransactionId,
    revocation: Sequence,
    #[serde(deserialize_with = "object")]
    inventory: InventoryCheckpointUnified,
    #[serde(deserialize_with = "object")]
    sources: SourceCheckpoint,
    authorization: StockAuthorizationRef,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityV8 {
    version: u8,
    #[serde(deserialize_with = "object")]
    namespace: SellerAuthoritySelection,
    administrator: GrantId,
    transaction: TransactionId,
    revocation: Sequence,
    #[serde(deserialize_with = "object")]
    inventory: InventoryCheckpointUnified,
    #[serde(deserialize_with = "object")]
    sources: SourceCheckpoint,
    #[serde(deserialize_with = "object")]
    public_sources: PublicSourceCheckpoint,
    authorization: StockAuthorizationRef,
}
impl Authority {
    fn encode(&self) -> Result<zeroize::Zeroizing<Vec<u8>>, WorkspaceError> {
        if !matches!(self.catalog, 8..=10) && self.public_sources.is_some() {
            return Err(WorkspaceError::RecoveryRequired);
        }
        if matches!(self.catalog, 8..=10) {
            let public_sources = self
                .public_sources
                .ok_or(WorkspaceError::RecoveryRequired)?;
            public_sources.validate()?;
            let bytes = zeroize::Zeroizing::new(
                serde_json::to_vec(&AuthorityV8 {
                    version: self.catalog as u8,
                    namespace: SellerAuthoritySelection::from_attachment(&self.input),
                    administrator: self.input.grant_id(),
                    transaction: self.transaction,
                    revocation: self.revocation,
                    inventory: self
                        .inventory
                        .ok_or(WorkspaceError::RecoveryRequired)?
                        .unified()?,
                    sources: self.sources.ok_or(WorkspaceError::RecoveryRequired)?,
                    public_sources,
                    authorization: self.authorization.ok_or(WorkspaceError::RecoveryRequired)?,
                })
                .map_err(|_| WorkspaceError::StorageFailure)?,
            );
            if bytes.len() > 16 * 1024 {
                return Err(WorkspaceError::Capacity);
            }
            return Ok(bytes);
        }
        if self.catalog == 7 {
            let bytes = zeroize::Zeroizing::new(
                serde_json::to_vec(&AuthorityV7 {
                    version: 7,
                    namespace: SellerAuthoritySelection::from_attachment(&self.input),
                    administrator: self.input.grant_id(),
                    transaction: self.transaction,
                    revocation: self.revocation,
                    inventory: self
                        .inventory
                        .ok_or(WorkspaceError::RecoveryRequired)?
                        .unified()?,
                    sources: self.sources.ok_or(WorkspaceError::RecoveryRequired)?,
                    authorization: self.authorization.ok_or(WorkspaceError::RecoveryRequired)?,
                })
                .map_err(|_| WorkspaceError::StorageFailure)?,
            );
            if bytes.len() > 16 * 1024 {
                return Err(WorkspaceError::Capacity);
            }
            return Ok(bytes);
        }
        if self.catalog == 6 {
            let bytes = serde_json::to_vec(&AuthorityV6 {
                version: 6,
                namespace: SellerAuthoritySelection::from_attachment(&self.input),
                administrator: self.input.grant_id(),
                transaction: self.transaction,
                revocation: self.revocation,
                inventory: self
                    .inventory
                    .ok_or(WorkspaceError::RecoveryRequired)?
                    .current()?,
                sources: self.sources.ok_or(WorkspaceError::RecoveryRequired)?,
                authorization: self.authorization.ok_or(WorkspaceError::RecoveryRequired)?,
            })
            .map_err(|_| WorkspaceError::StorageFailure)?;
            if bytes.len() > 16 * 1024 {
                return Err(WorkspaceError::Capacity);
            }
            return Ok(zeroize::Zeroizing::new(bytes));
        }
        if self.catalog == 5 {
            let bytes = serde_json::to_vec(&AuthorityV5 {
                version: 5,
                namespace: SellerAuthoritySelection::from_attachment(&self.input),
                administrator: self.input.grant_id(),
                transaction: self.transaction,
                revocation: self.revocation,
                inventory: self
                    .inventory
                    .ok_or(WorkspaceError::RecoveryRequired)?
                    .current()?,
                authorization: self.authorization.ok_or(WorkspaceError::RecoveryRequired)?,
            })
            .map_err(|_| WorkspaceError::StorageFailure)?;
            if bytes.len() > 16 * 1024 {
                return Err(WorkspaceError::Capacity);
            }
            return Ok(zeroize::Zeroizing::new(bytes));
        }
        if self.catalog == 4 {
            let bytes = serde_json::to_vec(&AuthorityV4 {
                version: 4,
                namespace: SellerAuthoritySelection::from_attachment(&self.input),
                administrator: self.input.grant_id(),
                transaction: self.transaction,
                revocation: self.revocation,
                inventory: self
                    .inventory
                    .ok_or(WorkspaceError::RecoveryRequired)?
                    .legacy()?,
                authorization: self.authorization.ok_or(WorkspaceError::RecoveryRequired)?,
            })
            .map_err(|_| WorkspaceError::StorageFailure)?;
            if bytes.len() > 16 * 1024 {
                return Err(WorkspaceError::Capacity);
            }
            return Ok(zeroize::Zeroizing::new(bytes));
        }
        let bytes = match self.inventory {
            None => serde_json::to_vec(&AuthorityV1 {
                version: 1,
                input: self.input.clone(),
                transaction: self.transaction,
                revocation: self.revocation,
            }),
            Some(inventory) => serde_json::to_vec(&InventoryAuthorityWire {
                version: self.catalog as u8,
                input: self.input.clone(),
                transaction: self.transaction,
                revocation: self.revocation,
                inventory: inventory.legacy()?,
            }),
        }
        .map_err(|_| WorkspaceError::StorageFailure)?;
        Ok(zeroize::Zeroizing::new(bytes))
    }
    fn decode(bytes: &[u8], catalog: i64) -> Result<Self, WorkspaceError> {
        let value = match catalog {
            1 => {
                let v: AuthorityV1 = closed_record(bytes)?;
                if v.version != 1 {
                    return Err(WorkspaceError::RecoveryRequired);
                }
                Self {
                    catalog,
                    input: v.input,
                    transaction: v.transaction,
                    revocation: v.revocation,
                    inventory: None,
                    authorization: None,
                    sources: None,
                    public_sources: None,
                }
            }
            2 | 3 => {
                let v: InventoryAuthorityWire = closed_record(bytes)?;
                if i64::from(v.version) != catalog
                    || (v.inventory.revision.get() == 0) != v.inventory.head.is_none()
                {
                    return Err(WorkspaceError::RecoveryRequired);
                }
                Self {
                    catalog,
                    input: v.input,
                    transaction: v.transaction,
                    revocation: v.revocation,
                    inventory: Some(v.inventory.into()),
                    authorization: None,
                    sources: None,
                    public_sources: None,
                }
            }
            4 => {
                let v: AuthorityV4 = closed_record(bytes)?;
                if v.version != 4
                    || v.authorization.generation() != v.revocation
                    || (v.inventory.revision.get() == 0) != v.inventory.head.is_none()
                {
                    return Err(WorkspaceError::RecoveryRequired);
                }
                Self {
                    catalog,
                    input: v.namespace.attachment(v.administrator)?,
                    transaction: v.transaction,
                    revocation: v.revocation,
                    inventory: Some(v.inventory.into()),
                    authorization: Some(v.authorization),
                    sources: None,
                    public_sources: None,
                }
            }
            5 => {
                let v: AuthorityV5 = closed_record(bytes)?;
                if v.version != 5
                    || v.authorization.generation() != v.revocation
                    || (v.inventory.revision.get() == 0) != v.inventory.head.is_none()
                {
                    return Err(WorkspaceError::RecoveryRequired);
                }
                Self {
                    catalog,
                    input: v.namespace.attachment(v.administrator)?,
                    transaction: v.transaction,
                    revocation: v.revocation,
                    inventory: Some(v.inventory.into()),
                    authorization: Some(v.authorization),
                    sources: None,
                    public_sources: None,
                }
            }
            6 => {
                let value: AuthorityV6 = closed_record(bytes)?;
                if value.version != 6
                    || value.authorization.generation() != value.revocation
                    || (value.inventory.revision.get() == 0) != value.inventory.head.is_none()
                    || (value.sources.revision.get() == 0) != value.sources.head.is_none()
                    || (value.sources.revision.get() == 0) != (value.sources.parts.get() == 0)
                    || value.sources.records.get() > value.sources.revision.get()
                    || value.sources.parts.get() / 3 < value.sources.revision.get()
                    || value.sources.parts.get() > value.sources.revision.get().saturating_mul(12)
                {
                    return Err(WorkspaceError::RecoveryRequired);
                }
                Self {
                    catalog,
                    input: value.namespace.attachment(value.administrator)?,
                    transaction: value.transaction,
                    revocation: value.revocation,
                    inventory: Some(value.inventory.into()),
                    authorization: Some(value.authorization),
                    sources: Some(value.sources),
                    public_sources: None,
                }
            }
            7 => {
                let value: AuthorityV7 = closed_record(bytes)?;
                if value.version != 7
                    || value.authorization.generation() != value.revocation
                    || (value.inventory.revision.get() == 0) != value.inventory.head.is_none()
                    || value.inventory.orders.get() > value.inventory.revision.get()
                    || value.inventory.pools.get() > value.inventory.revision.get()
                    || value.inventory.mappings.get() > value.inventory.revision.get()
                    || value.inventory.movements.get()
                        > value.inventory.revision.get().saturating_mul(64)
                    || (value.sources.revision.get() == 0) != value.sources.head.is_none()
                    || (value.sources.revision.get() == 0) != (value.sources.parts.get() == 0)
                    || value.sources.records.get() > value.sources.revision.get()
                    || value.sources.parts.get() / 3 < value.sources.revision.get()
                    || value.sources.parts.get() > value.sources.revision.get().saturating_mul(12)
                {
                    return Err(WorkspaceError::RecoveryRequired);
                }
                Self {
                    catalog,
                    input: value.namespace.attachment(value.administrator)?,
                    transaction: value.transaction,
                    revocation: value.revocation,
                    inventory: Some(value.inventory.into()),
                    authorization: Some(value.authorization),
                    sources: Some(value.sources),
                    public_sources: None,
                }
            }
            8..=10 => {
                let value: AuthorityV8 = closed_record(bytes)?;
                value.public_sources.validate()?;
                if i64::from(value.version) != catalog
                    || value.authorization.generation() != value.revocation
                    || (value.inventory.revision.get() == 0) != value.inventory.head.is_none()
                    || value.inventory.orders.get() > value.inventory.revision.get()
                    || value.inventory.pools.get() > value.inventory.revision.get()
                    || value.inventory.mappings.get() > value.inventory.revision.get()
                    || value.inventory.movements.get()
                        > value.inventory.revision.get().saturating_mul(64)
                    || (value.sources.revision.get() == 0) != value.sources.head.is_none()
                    || (value.sources.revision.get() == 0) != (value.sources.parts.get() == 0)
                    || value.sources.records.get() > value.sources.revision.get()
                    || value.sources.parts.get() / 3 < value.sources.revision.get()
                    || value.sources.parts.get() > value.sources.revision.get().saturating_mul(12)
                {
                    return Err(WorkspaceError::RecoveryRequired);
                }
                Self {
                    catalog,
                    input: value.namespace.attachment(value.administrator)?,
                    transaction: value.transaction,
                    revocation: value.revocation,
                    inventory: Some(value.inventory.into()),
                    authorization: Some(value.authorization),
                    sources: Some(value.sources),
                    public_sources: Some(value.public_sources),
                }
            }
            _ => return Err(WorkspaceError::RecoveryRequired),
        };
        if value.revocation.get() == 0 {
            return Err(WorkspaceError::RecoveryRequired);
        }
        Ok(value)
    }
    fn version(&self) -> i64 {
        self.catalog
    }
}
pub(super) struct Initial {
    pub grant: GrantRecord,
    pub transaction: TransactionId,
    pub expires_at: radroots_domain::UnixSeconds,
    pub clock: Arc<dyn WorkspaceClock>,
}
pub(super) mod inventory;
mod schema;
pub(super) async fn initialize(
    connection: &mut SqliteConnection,
    namespace: &Namespace,
    input: &AttachmentInput,
    initial: Option<Initial>,
    crypto: &WorkspaceCrypto,
) -> Result<(), WorkspaceError> {
    namespace.validate()?;
    if let Some(initial) = initial {
        let mut authority = Authority {
            public_sources: Some(PublicSourceCheckpoint {
                revision: Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?,
                events: Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?,
                heads: Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?,
                parts: Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?,
                head: None,
            }),
            sources: Some(SourceCheckpoint {
                revision: Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?,
                records: Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?,
                parts: Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?,
                head: None,
            }),
            catalog: schema::CURRENT,
            authorization: None,
            input: input.clone(),
            transaction: initial.transaction,
            revocation: Sequence::new(1).map_err(|_| WorkspaceError::InvalidInput)?,
            inventory: Some(InventoryCheckpoint {
                orders: if schema::CURRENT >= 7 {
                    Some(Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?)
                } else {
                    None
                },
                mappings: Some(Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?),
                revision: Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?,
                head: None,
                pools: Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?,
                movements: Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?,
            }),
        };
        let initial_record = StockGrantRecord::from_enrollment(initial.grant)?;
        let grant = crypto
            .seal_stock_record(
                input,
                "grant",
                input.grant_id().as_bytes(),
                &initial_record.encode()?,
            )
            .await?;
        authority.authorization = Some(grants::single_state(input.grant_id(), &grant)?);
        let clear = authority.encode()?;
        let root = crypto
            .seal_stock_root(input, authority.version(), &clear)
            .await?;
        let now = initial.clock.now()?;
        if now.get() == 0 {
            return Err(WorkspaceError::Unavailable);
        }
        if now >= initial.expires_at {
            return Err(WorkspaceError::GrantExpired);
        }
        let mut tx = connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| WorkspaceError::StorageFailure)?;
        let changed: Result<(), WorkspaceError> = async {
            namespace.validate()?;
            let now = initial.clock.now()?;
            if now.get() == 0 {
                return Err(WorkspaceError::Unavailable);
            }
            if now >= initial.expires_at {
                return Err(WorkspaceError::GrantExpired);
            }
            for (_, _, _, sql) in schema::objects(schema::CURRENT)? {
                sqlx::raw_sql(sql)
                    .execute(&mut *tx)
                    .await
                    .map_err(|_| WorkspaceError::StorageFailure)?;
            }
            sqlx::query("PRAGMA user_version=10")
                .execute(&mut *tx)
                .await
                .map_err(|_| WorkspaceError::StorageFailure)?;
            sqlx::query("INSERT INTO stock_authority(singleton,protected) VALUES(1,?)")
                .bind(root)
                .execute(&mut *tx)
                .await
                .map_err(|_| WorkspaceError::StorageFailure)?;
            sqlx::query("INSERT INTO stock_grants(grant_id,protected) VALUES(?,?)")
                .bind(input.grant_id().as_bytes().as_slice())
                .bind(grant)
                .execute(&mut *tx)
                .await
                .map_err(|_| WorkspaceError::StorageFailure)?;
            Ok(())
        }
        .await;
        if let Err(error) = changed {
            tx.rollback()
                .await
                .map_err(|_| WorkspaceError::CommitUnknown)?;
            return Err(error);
        }
        tx.commit()
            .await
            .map_err(|_| WorkspaceError::CommitUnknown)?;
    }
    schema::admit(connection).await?;
    schema::integrity(connection).await?;
    read(connection, input, crypto).await?;
    namespace.validate()
}
async fn read(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
) -> Result<(Authority, StockRecord, Vec<u8>, Vec<u8>), WorkspaceError> {
    // Bound in SQL before materializing attacker-controlled protected payloads.
    let root: Vec<u8> = sqlx::query_scalar("SELECT protected FROM stock_authority WHERE singleton=1 AND length(protected) BETWEEN 1 AND 262144").fetch_optional(&mut *connection).await.map_err(|_| WorkspaceError::RecoveryRequired)?.ok_or(WorkspaceError::RecoveryRequired)?;
    let grant: Vec<u8> = sqlx::query_scalar("SELECT protected FROM stock_grants WHERE grant_id=? AND length(protected) BETWEEN 1 AND 262144").bind(input.grant_id().as_bytes().as_slice()).fetch_optional(&mut *connection).await.map_err(|_| WorkspaceError::RecoveryRequired)?.ok_or(WorkspaceError::WrongNamespace)?;
    let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    let clear = crypto.open_stock_root(input, catalog, &root).await?;
    let authority = clear.expose_secret(|bytes| Authority::decode(bytes, catalog))?;
    if (if catalog >= 4 {
        !SellerAuthoritySelection::from_attachment(&authority.input).matches_attachment(input)
    } else {
        authority.input != *input
    }) || authority.revocation.get() == 0
    {
        return Err(WorkspaceError::WrongNamespace);
    }
    let record = if catalog >= 4 {
        grants::check_state(
            connection,
            authority
                .authorization
                .ok_or(WorkspaceError::RecoveryRequired)?,
        )
        .await?;
        let clear = crypto
            .open_stock_record(input, "grant", input.grant_id().as_bytes(), &grant)
            .await?;
        let record = clear.expose_secret(StockGrantRecord::decode)?;
        if !record.belongs_to(&SellerAuthoritySelection::from_attachment(input))?
            || record.administrator() != (input.grant_id() == authority.input.grant_id())
        {
            return Err(WorkspaceError::RecoveryRequired);
        }
        StockRecord::Scoped(record)
    } else {
        let clear = crypto.open_stock(input, false, &grant).await?;
        let record = clear.expose_secret(GrantRecord::decode)?;
        if record.scope() != WorkspaceScope::SellerStockOwner {
            return Err(WorkspaceError::RecoveryRequired);
        }
        StockRecord::Legacy(record)
    };
    if let Some(checkpoint) = authority.inventory {
        inventory::check_head(connection, checkpoint).await?;
    }
    if let Some(checkpoint) = authority.sources {
        check_source_counts(connection, checkpoint).await?;
    }
    if let Some(checkpoint) = authority.public_sources {
        public_source::counts(connection, checkpoint).await?;
    }
    Ok((authority, record, root, grant))
}
pub(super) async fn authenticate(
    connection: &mut SqliteConnection,
    namespace: &Namespace,
    expected: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    clock: &dyn WorkspaceClock,
    selected: &AttachmentInput,
    proof: &AttachmentProof,
) -> Result<(StockAuthorityReceipt, WorkspaceBinding), WorkspaceError> {
    namespace.validate()?;
    if !SellerAuthoritySelection::from_attachment(expected).matches_attachment(selected) {
        return Err(WorkspaceError::WrongNamespace);
    }
    let (authority, record, root, grant) = read(connection, selected, crypto).await?;
    record.authenticate(selected, proof, clock.now()?)?;
    let mut tx = connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| WorkspaceError::StorageFailure)?;
    let checked = async {
        namespace.validate()?;
        unchanged(&mut tx, selected, authority.version(), &root, &grant).await?;
        if let Some(state) = authority.authorization {
            grants::check_state(&mut tx, state).await?;
        }
        record.authenticate(selected, proof, clock.now()?)
    }
    .await;
    tx.rollback()
        .await
        .map_err(|_| WorkspaceError::CommitUnknown)?;
    let binding = checked?;
    let binding = if let Some(state) = authority.authorization {
        binding.with_stock_authorization(state)?
    } else {
        binding
    };
    if binding.scope() != WorkspaceScope::SellerStockOwner {
        return Err(WorkspaceError::RecoveryRequired);
    }
    namespace.validate()?;
    Ok((
        StockAuthorityReceipt {
            public_sources:authority.public_sources.map(|value|radroots_storage::workspace::seller_stock::StockListingCheckpointRef::from_host_metadata(expected.authority(),value.revision,value.head).map(|state|state.with_optional_authorization(authority.authorization))).transpose()?,
            sources:authority.sources.map(|value|radroots_storage::workspace::seller_stock::StockSourceCheckpointRef::from_host_metadata(expected.authority(),value.revision,value.head).map(|state|state.with_optional_authorization(authority.authorization))).transpose()?,
            input: authority.input,
            transaction: authority.transaction,
            revocation: authority.revocation,
            inventory: authority
                .inventory
                .map(|v| {
                    radroots_storage::inventory::InventoryCheckpointRef::from_host_metadata(
                        expected.authority(),
                        v.revision,
                        v.head,
                    )
                    .map(|checkpoint| {
                        if let Some(state) = authority.authorization {
                            checkpoint.with_authorization(state)
                        } else {
                            checkpoint
                        }
                    })
                })
                .transpose()?,
        },
        binding,
    ))
}
pub(super) async fn revoke(
    connection: &mut SqliteConnection,
    namespace: &Namespace,
    expected: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    clock: &dyn WorkspaceClock,
    selected: &AttachmentInput,
    proof: &AttachmentProof,
) -> Result<(), WorkspaceError> {
    namespace.validate()?;
    if !SellerAuthoritySelection::from_attachment(expected).matches_attachment(selected) {
        return Err(WorkspaceError::WrongNamespace);
    }
    let (mut authority, mut record, old_root, old_grant) =
        read(connection, selected, crypto).await?;
    if authority.catalog >= 4 {
        return Err(WorkspaceError::Unauthorized);
    }
    let expiry = record
        .authenticate(selected, proof, clock.now()?)?
        .expires_at();
    record.revoke()?;
    authority.revocation = Sequence::new(
        authority
            .revocation
            .get()
            .checked_add(1)
            .ok_or(WorkspaceError::Capacity)?,
    )
    .map_err(|_| WorkspaceError::Capacity)?;
    let clear = authority.encode()?;
    let root = crypto
        .seal_stock_root(expected, authority.version(), &clear)
        .await?;
    let grant = crypto
        .seal_stock(expected, false, &record.encode()?)
        .await?;
    namespace.validate()?;
    let mut tx = connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| WorkspaceError::StorageFailure)?;
    let changed: Result<(), WorkspaceError> = async {
        unchanged(
            &mut tx,
            expected,
            authority.version(),
            &old_root,
            &old_grant,
        )
        .await?;
        // Current clock and both exact protected snapshots fence the write after provider IO.
        record.authenticate_retained_capture(selected, proof)?;
        let now = clock.now()?;
        if now.get() == 0 {
            return Err(WorkspaceError::Unavailable);
        }
        if now >= expiry {
            return Err(WorkspaceError::GrantExpired);
        }
        let a =
            sqlx::query("UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?")
                .bind(root)
                .bind(old_root)
                .execute(&mut *tx)
                .await
                .map_err(|_| WorkspaceError::StorageFailure)?;
        let g = sqlx::query("UPDATE stock_grants SET protected=? WHERE grant_id=? AND protected=?")
            .bind(grant)
            .bind(expected.grant_id().as_bytes().as_slice())
            .bind(old_grant)
            .execute(&mut *tx)
            .await
            .map_err(|_| WorkspaceError::StorageFailure)?;
        if a.rows_affected() != 1 || g.rows_affected() != 1 {
            return Err(WorkspaceError::RecoveryRequired);
        }
        namespace.validate()?;
        Ok(())
    }
    .await;
    if let Err(error) = changed {
        tx.rollback()
            .await
            .map_err(|_| WorkspaceError::CommitUnknown)?;
        return Err(error);
    }
    tx.commit().await.map_err(|_| WorkspaceError::CommitUnknown)
}

async fn unchanged(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    catalog: i64,
    root: &[u8],
    grant: &[u8],
) -> Result<(), WorkspaceError> {
    let current: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if current != catalog {
        return Err(WorkspaceError::RecoveryRequired);
    }
    let matched: i64 = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM stock_authority WHERE singleton=1 AND protected=?) AND EXISTS(SELECT 1 FROM stock_grants WHERE grant_id=? AND protected=?)")
        .bind(root).bind(input.grant_id().as_bytes().as_slice()).bind(grant).fetch_one(connection).await.map_err(|_| WorkspaceError::StorageFailure)?;
    if matched != 1 {
        return Err(WorkspaceError::RecoveryRequired);
    }
    Ok(())
}

impl radroots_storage::inventory::InventoryStore for NativeWorkspace {
    fn option_mapping(
        &self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
        key: radroots_domain::stock::OptionMappingKey,
        proof: AttachmentProof,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<
            Option<radroots_domain::stock::OptionPoolMapping>,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(self.worker.option_mapping(binding, minimum, key, proof))
    }
    fn execute_inventory(
        &self,
        request: radroots_storage::inventory::InventoryRequest,
        proof: AttachmentProof,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<
            radroots_storage::inventory::InventoryOutcome,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(self.worker.execute_inventory(request, proof))
    }
    fn inventory_operation(
        &self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
        operation: radroots_domain::OperationId,
        digest: Option<radroots_domain::CommandDigest>,
        proof: AttachmentProof,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<
            Option<radroots_storage::inventory::InventoryReceipt>,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(
            self.worker
                .inventory_operation(binding, minimum, operation, digest, proof),
        )
    }
    fn pool(
        &self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
        id: radroots_domain::PoolId,
        proof: AttachmentProof,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<Option<radroots_domain::stock::PoolView>, radroots_storage::command::OperationError>,
    > {
        Box::pin(self.worker.pool(binding, minimum, id, proof))
    }
}

#[cfg(test)]
mod tests;

fn closed_record<'de, T: Deserialize<'de>>(bytes: &'de [u8]) -> Result<T, WorkspaceError> {
    if bytes.len() > 16 * 1024
        || bytes.iter().copied().find(|b| !b.is_ascii_whitespace()) != Some(b'{')
    {
        return Err(WorkspaceError::RecoveryRequired);
    }
    serde_json::from_slice(bytes).map_err(|_| WorkspaceError::RecoveryRequired)
}
async fn check_source_counts(
    connection: &mut SqliteConnection,
    checkpoint: SourceCheckpoint,
) -> Result<(), WorkspaceError> {
    use sqlx::Row;
    let row=sqlx::query("SELECT count(*),coalesce(max(revision),0),coalesce(sum(CASE WHEN revision=? AND chain_head=? THEN 1 ELSE 0 END),0) FROM stock_source_evidence").bind(checkpoint.revision.get() as i64).bind(checkpoint.head.map(|id|id.as_bytes().to_vec())).fetch_one(&mut *connection).await.map_err(|_|WorkspaceError::RecoveryRequired)?;
    let count: i64 = row
        .try_get(0)
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    let maximum: i64 = row
        .try_get(1)
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    let matched: i64 = row
        .try_get(2)
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    let records: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_source_records")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    let parts: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_source_parts")
        .fetch_one(connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if count != checkpoint.revision.get() as i64
        || maximum != count
        || matched != i64::from(checkpoint.head.is_some())
        || records != checkpoint.records.get() as i64
        || parts != checkpoint.parts.get() as i64
    {
        return Err(WorkspaceError::RecoveryRequired);
    }
    Ok(())
}
fn object<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    struct Object<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Object<T> {
        type Value = T;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a closed object")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<T, A::Error> {
            T::deserialize(serde::de::value::MapAccessDeserializer::new(map))
        }
    }
    deserializer.deserialize_map(Object(std::marker::PhantomData))
}

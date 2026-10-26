//! Typed owner-private stock transactions. Values and receipt construction
//! establish no authority or durable effect; concrete qualified hosts do that.
use crate::{
    command::{CommitAttempt, OperationError},
    workspace::{AttachmentProof, WorkspaceBinding, WorkspaceScope, WorkspaceStore},
};
use radroots_domain::{CommandDigest, OperationId, PoolId, TransactionId, stock::PoolSnapshot};
pub use radroots_protocol::command::{CapturedInventoryCommand, InventoryCommandKind};
use radroots_transport::BoxFuture;
mod abort;
mod acceptance;
mod evidence;
pub use abort::{StockAbortOutcome, StockAbortReceipt, StockAbortRequest};
pub use acceptance::{StockAcceptanceOutcome, StockAcceptanceReceipt, StockAcceptanceRequest};
pub use evidence::{StockEvidenceOutcome, StockEvidenceReceipt, StockEvidenceRequest};

/// Exact captured owner-private inventory command and original authenticated observation.
pub struct InventoryRequest {
    binding: WorkspaceBinding,
    minimum: InventoryCheckpointRef,
    command: CapturedInventoryCommand,
    replay_only: bool,
}
impl InventoryRequest {
    pub fn new(
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        command: impl Into<CapturedInventoryCommand>,
    ) -> Result<Self, OperationError> {
        Self::build(binding, minimum, command.into(), false)
    }
    /// An explicitly retained original command under a current grant. A missing
    /// original operation requires recovery and can never create a fresh effect.
    pub fn replay_from_protected_wire(
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        wire: &[u8],
    ) -> Result<Self, OperationError> {
        Self::build(
            binding,
            minimum,
            CapturedInventoryCommand::parse(wire).map_err(|_| OperationError::RecoveryRequired)?,
            true,
        )
    }
    fn build(
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        command: CapturedInventoryCommand,
        replay_only: bool,
    ) -> Result<Self, OperationError> {
        let context = command.context();
        let selection = binding.input();
        if binding.scope() != WorkspaceScope::SellerStockOwner {
            return Err(OperationError::Unauthorized);
        }
        if (!replay_only && context.actor() != binding.actor())
            || context.workspace_id() != selection.workspace_id()
            || context.authority() != selection.authority()
            || context.route_digest() != selection.route_digest()
            || context.actor().participant() != binding.actor().participant()
            || context.actor().role() != binding.actor().role()
            || context.actor().organization() != binding.actor().organization()
        {
            return Err(OperationError::InvalidCommand);
        }
        if context.expected().is_some() {
            return Err(OperationError::MissingHistory);
        }
        if context.approval().is_some() {
            return Err(OperationError::UnverifiedApproval);
        }
        if command.protected_wire().len() > 16 * 1024 {
            return Err(OperationError::InvalidCommand);
        }
        if minimum.authority() != selection.authority() {
            return Err(crate::workspace::WorkspaceError::StaleAuthority.into());
        }
        Ok(Self {
            binding,
            minimum,
            command,
            replay_only,
        })
    }
    /// Reconstructs captured data only; the concrete host must reauthenticate it.
    pub fn from_protected_wire(
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        wire: &[u8],
    ) -> Result<Self, OperationError> {
        Self::new(
            binding,
            minimum,
            CapturedInventoryCommand::parse(wire).map_err(|_| OperationError::RecoveryRequired)?,
        )
    }
    /// Independently retained acknowledged minimum, never guessed from a cache.
    pub const fn minimum_checkpoint(&self) -> InventoryCheckpointRef {
        self.minimum
    }
    pub const fn binding(&self) -> &WorkspaceBinding {
        &self.binding
    }
    pub const fn command(&self) -> &CapturedInventoryCommand {
        &self.command
    }
    pub const fn replay_only(&self) -> bool {
        self.replay_only
    }
    pub fn retained_copy(&self) -> Result<Self, OperationError> {
        Self::build(
            self.binding.clone(),
            self.minimum,
            CapturedInventoryCommand::parse(self.command.protected_wire())
                .map_err(|_| OperationError::RecoveryRequired)?,
            self.replay_only,
        )
    }
}
impl core::fmt::Debug for InventoryRequest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("InventoryRequest(<redacted>)")
    }
}
/// A claimed acknowledged inventory checkpoint. The trusted host must retain
/// its provenance independently; this value alone establishes no authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InventoryCheckpointRef {
    authority: radroots_domain::AuthorityRef,
    revision: radroots_domain::Sequence,
    head: Option<radroots_domain::CheckpointDigest>,
    authorization: Option<crate::workspace::seller_stock::StockAuthorizationRef>,
}
impl InventoryCheckpointRef {
    pub fn from_host_metadata(
        authority: radroots_domain::AuthorityRef,
        revision: radroots_domain::Sequence,
        head: Option<radroots_domain::CheckpointDigest>,
    ) -> Result<Self, crate::workspace::WorkspaceError> {
        if (revision.get() == 0) != head.is_none() {
            return Err(crate::workspace::WorkspaceError::InvalidInput);
        }
        Ok(Self {
            authority,
            revision,
            head,
            authorization: None,
        })
    }
    /// Binds an independently retained authorization-state observation. Metadata
    /// alone supplies neither credential possession nor checkpoint provenance.
    pub const fn with_authorization(
        mut self,
        value: crate::workspace::seller_stock::StockAuthorizationRef,
    ) -> Self {
        self.authorization = Some(value);
        self
    }
    pub const fn with_optional_authorization(
        mut self,
        value: Option<crate::workspace::seller_stock::StockAuthorizationRef>,
    ) -> Self {
        self.authorization = value;
        self
    }
    pub const fn authorization(
        self,
    ) -> Option<crate::workspace::seller_stock::StockAuthorizationRef> {
        self.authorization
    }
    pub const fn authority(self) -> radroots_domain::AuthorityRef {
        self.authority
    }
    pub const fn revision(self) -> radroots_domain::Sequence {
        self.revision
    }
    pub const fn head(self) -> Option<radroots_domain::CheckpointDigest> {
        self.head
    }
}
/// Confirmed owner-private inventory effect, without any signature or publication claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventoryReceipt {
    attempt: CommitAttempt,
    pool: PoolSnapshot,
    kind: InventoryCommandKind,
    mapping: Option<radroots_domain::stock::OptionPoolMapping>,
    checkpoint: InventoryCheckpointRef,
}
impl InventoryReceipt {
    /// Reconstructs only metadata. A host must confirm its actual transaction.
    pub fn from_committed_store(
        attempt: CommitAttempt,
        kind: InventoryCommandKind,
        pool: PoolSnapshot,
        checkpoint: InventoryCheckpointRef,
    ) -> Result<Self, OperationError> {
        if attempt.transaction_id().is_none()
            || pool.version().get() == 0
            || (kind == InventoryCommandKind::CreatePool && pool.version().get() != 1)
            || (kind != InventoryCommandKind::CreatePool && pool.version().get() < 2)
            || kind == InventoryCommandKind::MapOption
            || checkpoint.revision().get() == 0
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            attempt,
            kind,
            pool,
            mapping: None,
            checkpoint,
        })
    }
    /// Reconstructs mapping-effect metadata only. A qualified host must confirm
    /// the original transaction and its unchanged referenced pool separately.
    pub fn from_committed_mapping(
        attempt: CommitAttempt,
        pool: PoolSnapshot,
        mapping: radroots_domain::stock::OptionPoolMapping,
        checkpoint: InventoryCheckpointRef,
    ) -> Result<Self, OperationError> {
        if attempt.transaction_id().is_none()
            || checkpoint.revision().get() == 0
            || pool.version().get() == 0
            || mapping.pool_id() != pool.id()
            || mapping.pool_unit() != pool.unit()
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            attempt,
            pool,
            kind: InventoryCommandKind::MapOption,
            mapping: Some(mapping),
            checkpoint,
        })
    }
    /// Original private mapping effect; absent for physical inventory commands.
    pub const fn mapping(&self) -> Option<radroots_domain::stock::OptionPoolMapping> {
        self.mapping
    }
    /// Exact effect family retained by the original command.
    pub const fn kind(&self) -> InventoryCommandKind {
        self.kind
    }
    /// Checkpoint retained by the trusted host for subsequent recovery admission.
    pub const fn checkpoint(&self) -> InventoryCheckpointRef {
        self.checkpoint
    }
    pub const fn attempt(&self) -> CommitAttempt {
        self.attempt
    }
    pub const fn pool(&self) -> PoolSnapshot {
        self.pool
    }
    pub const fn transaction_id(&self) -> Option<TransactionId> {
        self.attempt.transaction_id()
    }
}
/// Unknown completion retains the original operation/digest/transaction anchor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InventoryOutcome {
    Committed(Box<InventoryReceipt>),
    Unknown(CommitAttempt),
}
/// Native and browser hosts enforce these same atomic rules, never a memory
/// success placeholder. No signing or network work occurs in these methods.
pub trait InventoryStore: WorkspaceStore {
    /// Appends already obtained bytes against the actual protected original
    /// claim. This performs no signing or installation and renews no permission.
    /// Unsupported hosts refuse. Borrowed original evidence survives errors.
    fn capture_stock_evidence<'a>(
        &'a self,
        _request: &'a StockEvidenceRequest,
        _proof: &'a AttachmentProof,
    ) -> BoxFuture<'a, Result<StockEvidenceOutcome, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Prepares an original acceptance with complete atomic holds and the order
    /// fence. Unsupported hosts execute no fallback or simulated stock effect.
    fn prepare_stock_acceptance(
        &self,
        _request: StockAcceptanceRequest,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<StockAcceptanceOutcome, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Explicit original-preparation cleanup. The concrete store must prove
    /// every original claim absent and retain the exact release atomically.
    /// Neither this port nor an unsupported host supplies an absence proof.
    fn abort_proven_unissued(
        &self,
        _request: StockAbortRequest,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<StockAbortOutcome, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Reads original cleanup under current access; absence is not permission
    /// to retry issuance or release. Unsupported hosts refuse explicitly.
    fn stock_preparation_abort(
        &self,
        _request: StockAbortRequest,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<StockAbortReceipt>, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Reads current owner-private configuration after authenticating retained
    /// progress. Physical inventory scopes confer no configuration access.
    fn option_mapping(
        &self,
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        key: radroots_domain::stock::OptionMappingKey,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<radroots_domain::stock::OptionPoolMapping>, OperationError>>;
    fn execute_inventory(
        &self,
        request: InventoryRequest,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<InventoryOutcome, OperationError>>;
    fn inventory_operation(
        &self,
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: Option<CommandDigest>,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<InventoryReceipt>, OperationError>>;
    fn pool(
        &self,
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        id: PoolId,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<radroots_domain::stock::PoolView>, OperationError>>;
}

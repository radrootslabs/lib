//! Typed owner-private stock transactions. Values and receipt construction
//! establish no authority or durable effect; concrete qualified hosts do that.
use crate::{
    command::{CommitAttempt, OperationError},
    workspace::{AttachmentProof, WorkspaceBinding, WorkspaceScope, WorkspaceStore},
};
use radroots_domain::{CommandDigest, OperationId, PoolId, TransactionId, stock::PoolSnapshot};
use radroots_protocol::command::CapturedPoolCreation;
use radroots_transport::BoxFuture;

/// Exact captured owner-private creation and original authenticated observation.
pub struct CreatePoolRequest {
    binding: WorkspaceBinding,
    minimum: InventoryCheckpointRef,
    command: CapturedPoolCreation,
}
impl CreatePoolRequest {
    pub fn new(
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        command: CapturedPoolCreation,
    ) -> Result<Self, OperationError> {
        let context = command.context();
        let selection = binding.input();
        if binding.scope() != WorkspaceScope::SellerStockOwner {
            return Err(OperationError::Unauthorized);
        }
        if context.workspace_id() != selection.workspace_id()
            || context.authority() != selection.authority()
            || context.route_digest() != selection.route_digest()
            || context.actor() != binding.actor()
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
            CapturedPoolCreation::parse(wire).map_err(|_| OperationError::RecoveryRequired)?,
        )
    }
    /// Independently retained acknowledged minimum, never guessed from a cache.
    pub const fn minimum_checkpoint(&self) -> InventoryCheckpointRef {
        self.minimum
    }
    pub const fn binding(&self) -> &WorkspaceBinding {
        &self.binding
    }
    pub const fn command(&self) -> &CapturedPoolCreation {
        &self.command
    }
    pub fn retained_copy(&self) -> Result<Self, OperationError> {
        Self::new(
            self.binding.clone(),
            self.minimum,
            CapturedPoolCreation::parse(self.command.protected_wire())
                .map_err(|_| OperationError::RecoveryRequired)?,
        )
    }
}
impl core::fmt::Debug for CreatePoolRequest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("CreatePoolRequest(<redacted>)")
    }
}
/// A claimed acknowledged inventory checkpoint. The trusted host must retain
/// its provenance independently; this value alone establishes no authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InventoryCheckpointRef {
    authority: radroots_domain::AuthorityRef,
    revision: radroots_domain::Sequence,
    head: Option<radroots_domain::CheckpointDigest>,
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
        })
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
/// Confirmed owner-private effect, without any signature or publication claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolCreationReceipt {
    attempt: CommitAttempt,
    pool: PoolSnapshot,
    checkpoint: InventoryCheckpointRef,
}
impl PoolCreationReceipt {
    /// Reconstructs only metadata. A host must confirm its actual transaction.
    pub fn from_committed_store(
        attempt: CommitAttempt,
        pool: PoolSnapshot,
        checkpoint: InventoryCheckpointRef,
    ) -> Result<Self, OperationError> {
        if attempt.transaction_id().is_none()
            || pool.version().get() != 1
            || checkpoint.revision().get() == 0
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            attempt,
            pool,
            checkpoint,
        })
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
pub enum PoolCreationOutcome {
    Committed(Box<PoolCreationReceipt>),
    Unknown(CommitAttempt),
}
/// Native and browser hosts enforce these same atomic rules, never a memory
/// success placeholder. No signing or network work occurs in these methods.
pub trait InventoryStore: WorkspaceStore {
    fn create_pool(
        &self,
        request: CreatePoolRequest,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<PoolCreationOutcome, OperationError>>;
    fn pool_creation(
        &self,
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: Option<CommandDigest>,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<PoolCreationReceipt>, OperationError>>;
    fn pool(
        &self,
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        id: PoolId,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<radroots_domain::stock::PoolView>, OperationError>>;
}

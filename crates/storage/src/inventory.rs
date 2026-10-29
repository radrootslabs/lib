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
mod deficit;
mod delivery;
pub use deficit::StockDeficit;
mod delivery_state;
mod envelope;
mod evidence;
mod rewrap;
pub use abort::{StockAbortOutcome, StockAbortReceipt, StockAbortRequest};
pub use acceptance::{StockAcceptanceOutcome, StockAcceptanceReceipt, StockAcceptanceRequest};
pub use delivery::{
    StockDeliveryAttempt, StockDeliveryAttemptOutcome, StockDeliveryObservation,
    StockDeliveryObservationOutcome, StockDeliveryObservationReceipt, StockDeliveryRequest,
};
pub use delivery_state::{
    StockDeliveryObservationPage, StockDeliverySummary, StockDeliveryTargetProgress,
};
pub use envelope::{
    StockEnvelopeOutcome, StockEnvelopeReceipt, StockEnvelopeRequest, StockEnvelopeSource,
};
pub use evidence::{
    StockEvidenceOutcome, StockEvidenceReceipt, StockEvidenceRequest, StockEvidenceSource,
};
pub use rewrap::{
    StockRewrapAdmission, StockRewrapOutcome, StockRewrapPage, StockRewrapRequest, StockRewrapState,
};

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
    deficits: Vec<StockDeficit>,
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
            deficits: Vec::new(),
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
            deficits: Vec::new(),
            checkpoint,
        })
    }
    /// Adds retained allocation metadata only; this creates no effect or authority.
    pub fn with_deficits(mut self, deficits: Vec<StockDeficit>) -> Result<Self, OperationError> {
        if deficits.len() > 32
            || deficits.iter().any(|value| {
                value.pool() != self.pool.id()
                    || value.backing().deficit() == radroots_domain::stock::StockTicks::ZERO
            })
            || deficits.iter().enumerate().any(|(index, value)| {
                deficits[..index].iter().any(|prior| {
                    prior.acceptance() == value.acceptance() && prior.line() == value.line()
                })
            })
            || (!deficits.is_empty()
                && !matches!(
                    self.kind,
                    InventoryCommandKind::SpoilStock | InventoryCommandKind::RemoveStock
                ))
        {
            return Err(OperationError::RecoveryRequired);
        }
        self.deficits = deficits;
        Ok(self)
    }
    /// Deficits caused by this transaction at its retained checkpoint.
    /// These are not a current full-obligation balance or disclosure grant.
    pub fn deficits(&self) -> &[StockDeficit] {
        &self.deficits
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
    /// Reconciles only already obtained original bytes after a failed/unknown
    /// append. A qualified host resolves its actual connection/commit state
    /// while retaining writer ownership; it cannot sign or renew a first issue.
    /// Unsupported hosts refuse, with the borrowed original left intact.
    fn reconcile_stock_evidence<'a>(
        &'a self,
        _request: &'a StockEvidenceRequest,
        _proof: &'a AttachmentProof,
    ) -> BoxFuture<'a, Result<StockEvidenceOutcome, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Retains an obtained outer against actual original stock S/T2. The host
    /// must verify exact containment and commit before any transport right.
    /// Capture grants no dispatch permission; unsupported hosts refuse.
    fn capture_stock_envelope<'a>(
        &'a self,
        _request: StockEnvelopeRequest,
        _proof: AttachmentProof,
        _candidate: &'a crate::command::PrivateEnvelopeCandidate,
    ) -> BoxFuture<'a, Result<StockEnvelopeOutcome, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Confirms one explicit wrapping admission before invoking wrapping.
    /// Only a qualified host's newly confirmed claim qualifies one invocation;
    /// existing and uncertain admissions never qualify another invocation.
    fn prepare_stock_rewrap(
        &self,
        _request: StockRewrapRequest,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<StockRewrapOutcome, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Queries an admission and any actual captured envelope without qualifying
    /// another wrapping invocation. Missing bytes cannot authorize regeneration.
    fn stock_rewrap_attempt(
        &self,
        _request: StockRewrapRequest,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<StockRewrapState>, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Repairs the original caller's retained writer state without a fresh
    /// clock. Returns no history, wrapping permit or dispatch permission;
    /// subsequent status reads still require current recovery access.
    fn reconcile_stock_rewrap_admission(
        &self,
        _request: StockRewrapRequest,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<(), OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Appends an obtained additional outer to its actual original admission.
    fn capture_stock_rewrap<'a>(
        &'a self,
        _request: StockRewrapRequest,
        _proof: AttachmentProof,
        _candidate: &'a crate::command::PrivateEnvelopeCandidate,
    ) -> BoxFuture<'a, Result<StockEnvelopeOutcome, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Reconciles the original additional outer on its retained connection
    /// owner. This never obtains new wrapping material or renews permission.
    fn reconcile_stock_rewrap<'a>(
        &'a self,
        _request: StockRewrapRequest,
        _proof: AttachmentProof,
        _candidate: &'a crate::command::PrivateEnvelopeCandidate,
    ) -> BoxFuture<'a, Result<StockEnvelopeOutcome, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Pages additional-envelope descriptors under current recovery access.
    /// Queries retain original identities without loading every raw envelope.
    fn stock_rewraps(
        &self,
        _request: StockEnvelopeRequest,
        _after: Option<radroots_domain::Sequence>,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<StockRewrapPage, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Reads the exact original retained outer under current recovery access.
    /// Absence is not permission to generate or dispatch a replacement.
    fn stock_envelope(
        &self,
        _request: StockEnvelopeRequest,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<StockEnvelopeSource>, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Reconciles borrowed original outer material on the same retained owner.
    /// Unsupported hosts cannot replace this with another wrap or a new writer.
    fn reconcile_stock_envelope<'a>(
        &'a self,
        _request: StockEnvelopeRequest,
        _proof: AttachmentProof,
        _candidate: &'a crate::command::PrivateEnvelopeCandidate,
    ) -> BoxFuture<'a, Result<StockEnvelopeOutcome, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Commits an actual potential invocation before transport. Existing or
    /// uncertain attempts never confer another dispatch right.
    /// A fresh claim must enforce the requested expected-attempt-count
    /// precondition against authenticated history in the same transaction.
    fn prepare_stock_delivery_attempt(
        &self,
        _request: StockDeliveryRequest,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<StockDeliveryAttemptOutcome, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Reads one original request without replay or deadline renewal.
    fn stock_delivery_attempt(
        &self,
        _source: StockEnvelopeRequest,
        _request_id: radroots_transport::sink::DeliveryRequestId,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<StockDeliveryAttempt>, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Reads bounded current per-endpoint progress under current recovery access.
    fn stock_delivery_summary(
        &self,
        _source: StockEnvelopeRequest,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<StockDeliverySummary, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Captures already obtained original outcome data without requiring a
    /// fresh clock or live grant. The caller retains its original across errors.
    fn record_stock_delivery_observation<'a>(
        &'a self,
        _binding: &'a WorkspaceBinding,
        _proof: &'a AttachmentProof,
        _observation: &'a StockDeliveryObservation,
    ) -> BoxFuture<'a, Result<StockDeliveryObservationOutcome, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Reconciles only the borrowed obtained outcome on its original owner.
    fn reconcile_stock_delivery_observation<'a>(
        &'a self,
        _binding: &'a WorkspaceBinding,
        _proof: &'a AttachmentProof,
        _observation: &'a StockDeliveryObservation,
    ) -> BoxFuture<'a, Result<StockDeliveryObservationOutcome, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Pages original independent outcomes; a read never issues transport.
    fn stock_delivery_observations(
        &self,
        _source: StockEnvelopeRequest,
        _after: Option<radroots_domain::Sequence>,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<StockDeliveryObservationPage, OperationError>> {
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
    /// every original invocation unissued, exclude outstanding replay and
    /// obtained output, and retain the exact release atomically. Neither this
    /// port nor an unsupported host supplies those proofs.
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

//! Typed authoritative admission/query transactions. The SPI is not a command
//! engine or a qualification claim. Concrete hosts must execute the specified
//! transactions and classify their actual completion; descriptors mint nothing.

use crate::workspace::{AttachmentProof, WorkspaceBinding, WorkspaceError, WorkspaceStore};
use core::fmt;
use radroots_domain::{
    CommandDigest, IssuanceWindow, OperationId, Sequence, TransactionId, WorkspaceId,
};
pub use radroots_protocol::chat::ChatSigningPlan;
use radroots_protocol::command::CapturedChatCommand;
use radroots_transport::BoxFuture;

mod installation;
mod issuance;
mod snapshot;
pub use installation::{
    ChatInstallation, InstallationOutcome, InstallationRequest, PublicationWork,
};
pub use issuance::{
    CapturedSeal, ClaimAttemptOutcome, ClaimAttemptRequest, EvidenceCaptureOutcome,
    EvidenceReceipt, SealEvidence, SigningAttempt,
};
pub use snapshot::{ClaimSnapshot, OperationSnapshot, RecipientSnapshot, RetainedOperation};

/// Fixed operation-store failures known before a successful admission commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OperationError {
    /// Current workspace authentication or ownership failed.
    Workspace(WorkspaceError),
    /// Captured fields do not match the selected authenticated scope.
    InvalidCommand,
    /// Obtained bytes do not authenticate against the exact retained seal.
    InvalidEvidence,
    /// This concrete authority cannot execute the requested action.
    Unauthorized,
    /// Required authenticated thread/order history is unavailable.
    MissingHistory,
    /// No qualified protected origin verifies the supplied approval descriptor.
    UnverifiedApproval,
    /// The first-admission deadline has passed; retained operations remain queryable.
    DeadlineExceeded,
    /// The retained operation identity binds a different command commitment.
    IdempotencyConflict,
    /// A known storage failure preceded writes or had confirmed rollback.
    StorageFailure,
    /// State, completion or decoding requires explicit reconciliation.
    RecoveryRequired,
}
impl OperationError {
    /// Returns an input-free diagnostic code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Workspace(e) => e.code(),
            Self::InvalidCommand => "invalid_command",
            Self::InvalidEvidence => "invalid_evidence",
            Self::Unauthorized => "unauthorized",
            Self::MissingHistory => "missing_history",
            Self::UnverifiedApproval => "unverified_approval",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::IdempotencyConflict => "idempotency_conflict",
            Self::StorageFailure => "storage_failure",
            Self::RecoveryRequired => "recovery_required",
        }
    }
}
impl fmt::Display for OperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl core::error::Error for OperationError {}
impl From<WorkspaceError> for OperationError {
    fn from(value: WorkspaceError) -> Self {
        Self::Workspace(value)
    }
}

/// Captured command and original authenticated binding. Construction compares
/// descriptors only. The separate protected proof must authenticate current
/// persisted state inside the actual admission transaction.
pub struct AdmissionRequest {
    binding: WorkspaceBinding,
    command: CapturedChatCommand,
}
impl AdmissionRequest {
    /// Refuses substituted workspace, actor/grant, authority or route data.
    /// Deadline and current grant checks belong to the concrete transaction;
    /// an expired original command may still be queried/reconciled by identity.
    pub fn new(
        binding: WorkspaceBinding,
        command: CapturedChatCommand,
    ) -> Result<Self, OperationError> {
        let context = command.context();
        let input = binding.input();
        if context.actor() != binding.actor()
            || context.workspace_id() != input.workspace_id()
            || context.authority() != input.authority()
            || context.route_digest() != input.route_digest()
        {
            return Err(OperationError::InvalidCommand);
        }
        Ok(Self { binding, command })
    }
    /// Decodes protected retained command data and compares its captured scope.
    /// This remains structural; current access is independently authenticated.
    pub fn from_protected_wire(
        binding: WorkspaceBinding,
        wire: &[u8],
    ) -> Result<Self, OperationError> {
        let command =
            CapturedChatCommand::parse(wire).map_err(|_| OperationError::RecoveryRequired)?;
        Self::new(binding, command)
    }
    /// Borrows the captured attachment observation, without claiming it is current.
    pub const fn binding(&self) -> &WorkspaceBinding {
        &self.binding
    }
    /// Borrows exact immutable command data.
    pub const fn command(&self) -> &CapturedChatCommand {
        &self.command
    }
    /// Transfers both owned descriptors to the concrete transaction owner.
    pub fn into_parts(self) -> (WorkspaceBinding, CapturedChatCommand) {
        (self.binding, self.command)
    }
}
impl fmt::Debug for AdmissionRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AdmissionRequest(<redacted>)")
    }
}

/// Immutable anchor for one attempted admission. It is not proof of commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommitAttempt {
    workspace_id: WorkspaceId,
    operation_id: OperationId,
    command_digest: CommandDigest,
    transaction_id: Option<TransactionId>,
}
impl CommitAttempt {
    /// Captures full original identities without creating an operation.
    pub const fn new(
        workspace_id: WorkspaceId,
        operation_id: OperationId,
        command_digest: CommandDigest,
        transaction_id: Option<TransactionId>,
    ) -> Self {
        Self {
            workspace_id,
            operation_id,
            command_digest,
            transaction_id,
        }
    }
    /// Returns the selected workspace.
    pub const fn workspace_id(self) -> WorkspaceId {
        self.workspace_id
    }
    /// Returns the original caller-provided operation identity.
    pub const fn operation_id(self) -> OperationId {
        self.operation_id
    }
    /// Returns its exact semantic command commitment.
    pub const fn command_digest(self) -> CommandDigest {
        self.command_digest
    }
    /// Returns the exact attempted transaction when observed. A lost owner
    /// reply may leave only the original operation/digest anchor available.
    pub const fn transaction_id(self) -> Option<TransactionId> {
        self.transaction_id
    }
}

/// Owned observation of actual retained Prepared state. It is not an installed
/// business result, signature, delivery fact, stock hold or readiness flag.
/// A custom adapter's report does not qualify that adapter's persistence.
pub struct PreparedOperation {
    request: Box<AdmissionRequest>,
    transaction_id: TransactionId,
    revision: Sequence,
    issuance_window: IssuanceWindow,
}
impl PreparedOperation {
    /// Validates a concrete store's structural report. The adapter must first
    /// confirm its real commit or read the matching committed row after reopen.
    pub fn from_committed_store(
        request: AdmissionRequest,
        transaction_id: TransactionId,
        revision: Sequence,
        issuance_window: IssuanceWindow,
    ) -> Result<Self, OperationError> {
        if revision.get() == 0
            || issuance_window.prepared_at() < request.binding().observed_at()
            || issuance_window.first_issue_not_after() > request.command().context().not_after()
            || issuance_window.first_issue_not_after() > request.binding().expires_at()
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            request: Box::new(request),
            transaction_id,
            revision,
            issuance_window,
        })
    }
    /// Borrows the captured original request, including its diagnostic correlation.
    pub const fn request(&self) -> &AdmissionRequest {
        &self.request
    }
    /// Returns the original confirmed admission transaction identity.
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction_id
    }
    /// Returns the retained monotonic operation revision.
    pub const fn revision(&self) -> Sequence {
        self.revision
    }
    /// Returns the original preparation observation and first-issue deadline.
    /// It remains historical after expiry and never establishes nonissuance.
    pub const fn issuance_window(&self) -> IssuanceWindow {
        self.issuance_window
    }
    /// Returns the exact retained identity/commitment anchor.
    pub fn identity(&self) -> CommitAttempt {
        let context = self.request.command().context();
        CommitAttempt::new(
            context.workspace_id(),
            context.operation_id(),
            self.request.command().digest(),
            Some(self.transaction_id),
        )
    }
}
impl fmt::Debug for PreparedOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PreparedOperation(<redacted>)")
    }
}

/// Actual initial transaction completion, separate from signing/installation.
pub enum AdmissionOutcome {
    /// The exact row commit or an existing committed row was confirmed.
    Committed(PreparedOperation),
    /// Completion is unknown. Reconcile this original anchor after safe reopen;
    /// never infer rollback, discard the ID or issue from another preparation.
    Unknown(CommitAttempt),
}
impl fmt::Debug for AdmissionOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Committed(_) => "AdmissionOutcome::Committed(<redacted>)",
            Self::Unknown(_) => "AdmissionOutcome::Unknown(<retained>)",
        })
    }
}

/// Typed transaction port, implemented by actual shared storage adapters.
/// Applications do not implement a second orchestration or stock engine here.
pub trait OperationJournal: WorkspaceStore {
    /// Atomically installs the original logical chat result and retains distinct
    /// publication work for actual obtained recipients. No signing or transport
    /// occurs here; unknown completion retains all original claims and evidence.
    fn install_transition_and_outbox(
        &self,
        request: InstallationRequest,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<InstallationOutcome, OperationError>>;
    /// Queries one coherent original preparation/plan/per-recipient evidence
    /// graph under current access. Reported absence grants no safe reissue.
    fn operation_snapshot(
        &self,
        binding: WorkspaceBinding,
        operation_id: OperationId,
        expected_digest: Option<CommandDigest>,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<OperationSnapshot>, OperationError>>;
    /// Commits a first claim before dispatch. Retained/unknown outcomes never
    /// authorize another invocation; the concrete executor owns that decision.
    fn claim_attempt(
        &self,
        request: ClaimAttemptRequest,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<ClaimAttemptOutcome, OperationError>>;
    /// Reads a retained potential invocation under current workspace access.
    fn signing_attempt(
        &self,
        request: ClaimAttemptRequest,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<SigningAttempt>, OperationError>>;
    /// Appends matching authenticated obtained bytes to the actual original
    /// claim, without a fresh-issuance deadline or observation clock. Borrowed
    /// input remains caller-owned through error, lost wait or unknown completion.
    fn capture_evidence<'a>(
        &'a self,
        input: crate::workspace::AttachmentInput,
        proof: &'a AttachmentProof,
        evidence: &'a SealEvidence,
    ) -> BoxFuture<'a, Result<EvidenceCaptureOutcome, OperationError>>;
    /// Reads original exact captured bytes under current workspace access.
    fn obtained_evidence(
        &self,
        request: ClaimAttemptRequest,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<CapturedSeal>, OperationError>>;
    /// Atomically retains the complete immutable plan under current access and
    /// the original first-issue window. No signer invocation happens here.
    fn bind_signing_intents(
        &self,
        binding: WorkspaceBinding,
        plan: ChatSigningPlan,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<PlanBindingOutcome, OperationError>>;
    /// Queries original complete plan bytes under current access, including
    /// after expiry. The returned value grants no permission to dispatch.
    fn signing_plan(
        &self,
        binding: WorkspaceBinding,
        operation_id: OperationId,
        expected_digest: CommandDigest,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<BoundSigningPlan>, OperationError>>;
    /// Authenticates current grant/fences, checks the original operation first,
    /// and confirms atomic retained preparation before any signing. Chat has no
    /// physical hold. Only confirmed rollback may return a known-failure error.
    fn admit_command_and_hold(
        &self,
        request: AdmissionRequest,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<AdmissionOutcome, OperationError>>;
    /// Queries original committed state under current access. Absence is not a
    /// generic proof that an unknown external effect never happened.
    fn operation(
        &self,
        binding: WorkspaceBinding,
        operation_id: OperationId,
        expected_digest: Option<CommandDigest>,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<PreparedOperation>, OperationError>>;
}

/// Complete retained T1 observation, separate from issuance and installation.
/// A custom adapter's structural report does not qualify durable execution.
pub struct BoundSigningPlan {
    prepared: PreparedOperation,
    plan: ChatSigningPlan,
    transaction_id: TransactionId,
    bound_at: radroots_domain::UnixSeconds,
}
impl BoundSigningPlan {
    /// Compares exact original command and fixed first-issue window.
    pub fn from_committed_store(
        prepared: PreparedOperation,
        plan: ChatSigningPlan,
        transaction_id: TransactionId,
        bound_at: radroots_domain::UnixSeconds,
    ) -> Result<Self, OperationError> {
        if plan.operation_id() != prepared.identity().operation_id()
            || plan.command_digest() != prepared.identity().command_digest()
            || bound_at < prepared.issuance_window().prepared_at()
            || bound_at >= prepared.issuance_window().first_issue_not_after()
            || plan.created_at() < prepared.issuance_window().prepared_at()
            || plan.created_at() > bound_at
        {
            return Err(OperationError::RecoveryRequired);
        }
        // Reconstruct against the actually retained command, rather than trust
        // a matching digest descriptor attached to unrelated plan fields.
        let checked = ChatSigningPlan::parse(prepared.request().command(), plan.protected_wire())
            .map_err(|_| OperationError::RecoveryRequired)?;
        Ok(Self {
            prepared,
            plan: checked,
            transaction_id,
            bound_at,
        })
    }
    /// Transfers retained command and exact plan without changing their identities.
    pub fn into_parts(self) -> (PreparedOperation, ChatSigningPlan) {
        (self.prepared, self.plan)
    }
    pub const fn prepared(&self) -> &PreparedOperation {
        &self.prepared
    }
    pub const fn plan(&self) -> &ChatSigningPlan {
        &self.plan
    }
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction_id
    }
    /// Returns the checked observation captured before protection; this is not
    /// an independently measured wall-clock commit or signing time.
    pub const fn bound_at(&self) -> radroots_domain::UnixSeconds {
        self.bound_at
    }
}
impl fmt::Debug for BoundSigningPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BoundSigningPlan(<redacted>)")
    }
}
/// Confirmed T1 completion or an original anchor requiring reconciliation.
pub enum PlanBindingOutcome {
    Committed(BoundSigningPlan),
    Unknown(CommitAttempt),
}
impl fmt::Debug for PlanBindingOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Committed(_) => "PlanBindingOutcome::Committed(<redacted>)",
            Self::Unknown(_) => "PlanBindingOutcome::Unknown(<retained>)",
        })
    }
}

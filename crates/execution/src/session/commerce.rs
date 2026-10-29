//! Shared acceptance orchestration and typed host transaction boundaries.
//! Host reports are a qualification boundary, never caller-created authority.
use super::{HostBound, HostFuture};
use alloc::boxed::Box;
use core::fmt;
use radroots_domain::{CommandDigest, OperationId, OrderId, PublicKey, RecordId, SigningAttemptId};
use radroots_protocol::commerce::CommerceSigningPlan;
use radroots_signing::{
    commerce::VerifiedCommerceSigningPlan,
    issuance::{ExactRecoveryRequest, IntentBinding},
    provider::ProviderCall,
    request::CancellationSignal,
};
use radroots_storage::{
    command::{CommitAttempt, OperationError},
    inventory::{
        InventoryCheckpointRef, StockAcceptanceReceipt, StockDeliverySummary, StockEvidenceReceipt,
        StockEvidenceSource,
    },
    workspace::{AttachmentProof, WorkspaceBinding},
};

#[path = "commerce_run.rs"]
mod run;
pub(crate) use run::observe;
pub(in crate::session) use run::rewrap_status;

/// Original operation selectors and independently retained minimum; no authority.
#[derive(Clone)]
pub struct AcceptanceSelection {
    /// Exact selected authenticated attachment descriptor.
    pub binding: WorkspaceBinding,
    /// Independently retained protected inventory minimum.
    pub minimum: InventoryCheckpointRef,
    /// Original full-width operation identity.
    pub operation: OperationId,
    /// Original captured command commitment.
    pub digest: CommandDigest,
}

/// Original durable claim metadata, without a first-dispatch or replay permit.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct AcceptanceClaim {
    /// Protected store claim identity.
    pub id: SigningAttemptId,
    /// Exact original event, recipient, command and deadline commitment.
    pub binding: IntentBinding,
}
impl fmt::Debug for AcceptanceClaim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AcceptanceClaim(<redacted>)")
    }
}

/// One recipient's independent actual retained signing observations.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct AcceptanceRecipient {
    /// Exact original participant.
    pub recipient: PublicKey,
    /// Original claim, if durably observed; never permission to sign again.
    pub claim: Option<AcceptanceClaim>,
    /// First authenticated original encoding, if durably observed.
    pub evidence: Option<StockEvidenceReceipt>,
    /// Authenticated fact about the original invocation, independent of later S.
    pub original_unissued: Option<AcceptanceUnissuedReceipt>,
}

/// Actual initial agreement installation, independent of self-copy and delivery.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct InstalledAcceptance {
    /// Original confirmed installation transaction.
    pub attempt: CommitAttempt,
    /// Installed order identity.
    pub order: OrderId,
    /// Installed original commitment record.
    pub record: RecordId,
    /// Confirmed protected checkpoint.
    pub checkpoint: InventoryCheckpointRef,
}
impl fmt::Debug for InstalledAcceptance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InstalledAcceptance(<redacted>)")
    }
}

/// Read-only authenticated native/browser snapshot retained by its target host.
/// Implementations expose original facts, not fresh allocation or signing policy.
pub trait AcceptanceSnapshot: HostBound {
    /// Actual original T0 and its unchanged rumor/window.
    fn preparation(&self) -> &StockAcceptanceReceipt;
    /// Complete original T1, if present.
    fn plan(&self) -> Option<&CommerceSigningPlan>;
    /// Current authenticated protected checkpoint.
    fn checkpoint(&self) -> InventoryCheckpointRef;
    /// Complete original bilateral set, in canonical participant order.
    fn recipients(&self) -> [AcceptanceRecipient; 2];
    /// Actual matching installation, if present.
    fn installation(&self) -> Option<InstalledAcceptance>;
    /// Whether an actual original proven-unissued abort was observed.
    fn aborted(&self) -> bool;
    /// Coherent bounded original per-recipient delivery progress, in the same
    /// canonical order as recipients. None means unsupported by this catalog.
    fn delivery(&self) -> Option<&[StockDeliverySummary; 2]>;
}

#[path = "commerce_dispatch.rs"]
mod dispatch;
pub use self::dispatch::{
    AcceptanceDispatch, AcceptanceDispatchOutcome, AcceptanceUnissued, AcceptanceUnissuedOutcome,
    AcceptanceUnissuedReceipt,
};

/// Actual host completion of T1; an unknown commit must be queried, not repeated
/// as a new preparation or signature.
pub enum AcceptancePlanOutcome {
    /// The exact original complete plan is durably bound.
    Bound(CommitAttempt),
    /// Original transaction completion remains uncertain.
    Unknown(CommitAttempt),
}
/// Per-recipient host claim results, without optimistic existing-claim replay.
pub enum AcceptanceClaimOutcome {
    /// Fresh confirmed single-use first-dispatch permit.
    Fresh(Box<dyn AcceptanceDispatch>),
    /// Original claim exists and provides no fresh permit.
    Existing(AcceptanceClaim),
    /// Original claim transaction completion remains uncertain.
    Unknown(CommitAttempt),
}
/// Actual initial T2 result; a report never claims wrapping or relay delivery.
pub enum AcceptanceInstallationOutcome {
    /// Original matching agreement and recipient work are installed.
    Installed(Box<InstalledAcceptance>),
    /// Original installation transaction completion remains uncertain.
    Unknown(CommitAttempt),
}

/// Mechanical target transactions for the shared concrete acceptance algorithm.
/// Implementations enforce protected identity, CAS, consent and exact transaction
/// completion. They do not choose business inputs, invoke signers or run a
/// replacement application workflow. The ordinary facade selects the same owner
/// as its InventoryStore; custom hosts require their own executed qualification.
pub trait AcceptanceStore: HostBound {
    /// Reads the exact retained signed seal under current recovery access.
    /// This cannot create signing material or confer dispatch permission.
    fn signing_evidence<'a>(
        &'a self,
        _selection: AcceptanceSelection,
        _receipt: StockEvidenceReceipt,
        _proof: &'a AttachmentProof,
    ) -> HostFuture<'a, Result<Option<StockEvidenceSource>, OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Reads original T0/T1, independent recipient S and initial T2 coherently.
    fn snapshot<'a>(
        &'a self,
        selection: AcceptanceSelection,
        proof: &'a AttachmentProof,
    ) -> HostFuture<'a, Result<Box<dyn AcceptanceSnapshot>, OperationError>>;
    /// Binds a shared cryptographically qualified exact plan to original T0.
    fn bind<'a>(
        &'a self,
        selection: AcceptanceSelection,
        plan: VerifiedCommerceSigningPlan,
        proof: &'a AttachmentProof,
    ) -> HostFuture<'a, Result<AcceptancePlanOutcome, OperationError>>;
    /// Commits and observes the exact original recipient's first claim.
    fn claim<'a>(
        &'a self,
        selection: AcceptanceSelection,
        recipient: PublicKey,
        proof: &'a AttachmentProof,
    ) -> HostFuture<'a, Result<AcceptanceClaimOutcome, OperationError>>;
    /// Explicit original exact replay authorization; unsupported hosts refuse.
    /// Refusal proves nothing about the original invocation's issuance.
    fn exact_replay<'a>(
        &'a self,
        _selection: AcceptanceSelection,
        _recipient: PublicKey,
        _call: ProviderCall,
        _proof: &'a AttachmentProof,
    ) -> HostFuture<'a, Result<(ExactRecoveryRequest, AcceptanceClaim), OperationError>> {
        Box::pin(async { Err(OperationError::UnsupportedOperation) })
    }
    /// Installs only the original agreement backed by actually retained peer S.
    fn install<'a>(
        &'a self,
        selection: AcceptanceSelection,
        proof: &'a AttachmentProof,
    ) -> HostFuture<'a, Result<AcceptanceInstallationOutcome, OperationError>>;
}

/// Owned original operation facts. No wait, signature or relay outcome is
/// collapsed into business success; this report cannot dispatch or release stock.
pub struct AcceptanceStatus {
    pub(crate) attempt: CommitAttempt,
    pub(crate) record: RecordId,
    pub(crate) checkpoint: InventoryCheckpointRef,
    pub(crate) recipients: [AcceptanceRecipient; 2],
    pub(crate) plan_bound: bool,
    pub(crate) installation: Option<InstalledAcceptance>,
    pub(crate) aborted: bool,
    pub(crate) delivery: Option<Box<[StockDeliverySummary; 2]>>,
}
impl AcceptanceStatus {
    /// Original per-recipient delivery progress, independently of installation
    /// or buyer receipt. Absence means this catalog does not support stock D.
    pub fn delivery(&self) -> Option<&[StockDeliverySummary; 2]> {
        self.delivery.as_deref()
    }
    /// Original confirmed T0 anchor.
    pub const fn attempt(&self) -> CommitAttempt {
        self.attempt
    }
    /// Original immutable commitment record.
    pub const fn record_id(&self) -> RecordId {
        self.record
    }
    /// Current authenticated observation checkpoint.
    pub const fn checkpoint(&self) -> InventoryCheckpointRef {
        self.checkpoint
    }
    /// Independent peer/self original signing observations.
    pub const fn recipients(&self) -> &[AcceptanceRecipient; 2] {
        &self.recipients
    }
    /// Whether complete original T1 was actually observed.
    pub const fn plan_bound(&self) -> bool {
        self.plan_bound
    }
    /// Actual original T2, independently of self-copy or relay delivery.
    pub const fn installation(&self) -> Option<InstalledAcceptance> {
        self.installation
    }
    /// Actual original abort observation, never inferred from absent evidence.
    pub const fn aborted(&self) -> bool {
        self.aborted
    }
}
impl fmt::Debug for AcceptanceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AcceptanceStatus(<redacted>)")
    }
}

/// Separate current-access observations of one explicitly requested rewrap.
/// A missing outer is an unknown wrapping outcome, never proof of nonissuance
/// or permission to regenerate. Delivery carries its own authenticated read
/// checkpoint; this report does not claim an atomic combined absence snapshot.
#[derive(Clone)]
pub struct AcceptanceRewrapStatus {
    id: OperationId,
    recipient: PublicKey,
    state: Option<radroots_storage::inventory::StockRewrapState>,
    delivery: Option<StockDeliverySummary>,
}
impl AcceptanceRewrapStatus {
    /// Explicit request identity, distinct from the original business command.
    pub const fn rewrap_id(&self) -> OperationId {
        self.id
    }
    /// Recipient whose original evidence and additional outer were selected.
    pub const fn recipient(&self) -> PublicKey {
        self.recipient
    }
    /// Observed admission and optional captured outer; absence grants no work.
    pub const fn state(&self) -> Option<radroots_storage::inventory::StockRewrapState> {
        self.state
    }
    /// Independent current delivery observation for the exact captured outer.
    pub const fn delivery(&self) -> Option<&StockDeliverySummary> {
        self.delivery.as_ref()
    }
}
impl fmt::Debug for AcceptanceRewrapStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AcceptanceRewrapStatus(<redacted>)")
    }
}

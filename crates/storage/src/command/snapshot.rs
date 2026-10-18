//! Coherent retained graph reports. Values are not persistence qualification or
//! dispatch permission; concrete owners must authenticate and compare all facts.
use super::{
    BoundSigningPlan, ChatInstallation, DeliveryAttemptSummary, DeliveryObservation,
    EvidenceReceipt, OperationError, PreparedOperation, PublicationWork, SigningAttempt,
};
use core::fmt;
use radroots_domain::{
    ChatMessage, ChatRelation, CommandDigest, EnvelopeEventId, MessageId, OperationId, PublicKey,
    SigningAttemptId, TransactionId, UnixSeconds, WorkspaceId,
};
use radroots_protocol::intent::IntentBinding;
use sha2::{Digest, Sha256};
use std::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
    vec,
    vec::Vec,
};

/// Fixed original claim facts, without another copy of its complete private plan.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ClaimSnapshot {
    workspace: WorkspaceId,
    operation: OperationId,
    command: CommandDigest,
    plan: [u8; 32],
    recipient: PublicKey,
    attempt: SigningAttemptId,
    transaction: TransactionId,
    claimed_at: UnixSeconds,
    event: EnvelopeEventId,
    intent: IntentBinding,
}
impl ClaimSnapshot {
    /// Copies a structural store report; no backend claim or nonissuance proof
    /// is created by this conversion.
    pub fn from_store_report(attempt: &SigningAttempt) -> Self {
        let identity = attempt.bound_plan().prepared().identity();
        Self {
            workspace: identity.workspace_id(),
            operation: identity.operation_id(),
            command: identity.command_digest(),
            plan: *attempt.bound_plan().plan().digest(),
            recipient: attempt.seal().recipient(),
            attempt: attempt.attempt_id(),
            transaction: attempt.transaction_id(),
            claimed_at: attempt.claimed_at(),
            event: EnvelopeEventId::from_bytes(attempt.seal().event_id()),
            intent: attempt.binding(),
        }
    }
    /// Original workspace identity.
    pub const fn workspace_id(self) -> WorkspaceId {
        self.workspace
    }
    /// Original operation identity.
    pub const fn operation_id(self) -> OperationId {
        self.operation
    }
    /// Original semantic command commitment.
    pub const fn command_digest(self) -> CommandDigest {
        self.command
    }
    /// Original complete plan commitment.
    pub const fn plan_digest(&self) -> &[u8; 32] {
        &self.plan
    }
    /// Exact intended recipient.
    pub const fn recipient(self) -> PublicKey {
        self.recipient
    }
    /// Full-width retained attempt identity.
    pub const fn attempt_id(self) -> SigningAttemptId {
        self.attempt
    }
    /// Original claim transaction.
    pub const fn transaction_id(self) -> TransactionId {
        self.transaction
    }
    /// Original claim observation, never a fresh deadline.
    pub const fn claimed_at(self) -> UnixSeconds {
        self.claimed_at
    }
    /// Expected exact signed seal ID, not a logical record identity.
    pub const fn event_id(self) -> EnvelopeEventId {
        self.event
    }
    /// Exact original intent commitment.
    pub const fn intent(self) -> IntentBinding {
        self.intent
    }
}
impl fmt::Debug for ClaimSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ClaimSnapshot(<redacted>)")
    }
}

/// One expected recipient's independent retained facts. Absence of obtained
/// evidence for a retained claim never establishes safe reissue.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct RecipientSnapshot {
    recipient: PublicKey,
    expected_event: Option<EnvelopeEventId>,
    claim: Option<ClaimSnapshot>,
    obtained: Option<EvidenceReceipt>,
}
impl RecipientSnapshot {
    /// Structural report; the complete snapshot constructor checks its scope.
    pub const fn from_store_report(
        recipient: PublicKey,
        expected_event: Option<EnvelopeEventId>,
        claim: Option<ClaimSnapshot>,
        obtained: Option<EvidenceReceipt>,
    ) -> Self {
        Self {
            recipient,
            expected_event,
            claim,
            obtained,
        }
    }
    /// Exact expected recipient.
    pub const fn recipient(self) -> PublicKey {
        self.recipient
    }
    /// Expected retained seal ID, when a complete plan exists.
    pub const fn expected_event(self) -> Option<EnvelopeEventId> {
        self.expected_event
    }
    /// Original potential invocation; not permission to invoke again.
    pub const fn claim(self) -> Option<ClaimSnapshot> {
        self.claim
    }
    /// Original protected obtained-artifact reference and optional observation.
    pub const fn obtained(self) -> Option<EvidenceReceipt> {
        self.obtained
    }
}
impl fmt::Debug for RecipientSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecipientSnapshot(<redacted>)")
    }
}

/// One owned preparation and at most one complete plan, shared by all recipient
/// reports without duplicating protected private plan data per recipient.
pub enum RetainedOperation {
    /// Confirmed preparation without a bound complete plan.
    Prepared(PreparedOperation),
    /// Confirmed preparation and complete immutable plan.
    Planned(Box<BoundSigningPlan>),
}
impl RetainedOperation {
    /// Borrows original committed preparation.
    pub fn prepared(&self) -> &PreparedOperation {
        match self {
            Self::Prepared(value) => value,
            Self::Planned(value) => value.prepared(),
        }
    }
    /// Borrows the complete plan, if retained.
    pub fn bound_plan(&self) -> Option<&BoundSigningPlan> {
        match self {
            Self::Prepared(_) => None,
            Self::Planned(value) => Some(value),
        }
    }
}
impl fmt::Debug for RetainedOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RetainedOperation(<redacted>)")
    }
}

/// Bounded coherent T0/T1/S graph report. Construction checks data relationships
/// only. It grants no authentication, backend qualification or installation fact.
pub struct OperationSnapshot {
    retained: RetainedOperation,
    recipients: Box<[RecipientSnapshot]>,
    observed_at: UnixSeconds,
    installation: Option<ChatInstallation>,
    message: Option<ChatMessage>,
    publications: Box<[PublicationWork]>,
    deliveries: Box<[DeliveryAttemptSummary]>,
    observations: Box<[DeliveryObservation]>,
}
impl OperationSnapshot {
    /// Stable scoped progress identity over original transaction, command,
    /// plan, claim and obtained references. Query time and diagnostic correlation
    /// are excluded. Hashing grants no backend authority.
    pub fn checkpoint_digest(&self) -> radroots_domain::CheckpointDigest {
        let prepared = self.prepared();
        let identity = prepared.identity();
        let mut digest = Sha256::new();
        digest.update(b"radroots.operation.snapshot.v3\0");
        digest.update(identity.workspace_id().as_bytes());
        digest.update(identity.operation_id().as_bytes());
        digest.update(identity.command_digest().as_bytes());
        digest.update(prepared.transaction_id().as_bytes());
        digest.update(prepared.revision().get().to_be_bytes());
        digest.update(prepared.issuance_window().prepared_at().get().to_be_bytes());
        digest.update(
            prepared
                .issuance_window()
                .first_issue_not_after()
                .get()
                .to_be_bytes(),
        );
        digest.update(
            prepared
                .request()
                .binding()
                .input()
                .checkpoint()
                .digest()
                .as_bytes(),
        );
        match self.bound_plan() {
            Some(bound) => {
                digest.update([1]);
                digest.update(bound.plan().digest());
                digest.update(bound.transaction_id().as_bytes());
                digest.update(bound.bound_at().get().to_be_bytes());
            }
            None => digest.update([0]),
        }
        for recipient in self.recipients() {
            digest.update(recipient.recipient().as_bytes());
            match recipient.claim() {
                Some(claim) => {
                    digest.update([1]);
                    digest.update(claim.attempt_id().as_bytes());
                    digest.update(claim.transaction_id().as_bytes());
                    digest.update(claim.intent().digest());
                    digest.update(claim.claimed_at().get().to_be_bytes());
                }
                None => digest.update([0]),
            }
            match recipient.obtained() {
                Some(obtained) => {
                    digest.update([1]);
                    digest.update(obtained.transaction_id().as_bytes());
                    digest.update(obtained.event_id().as_bytes());
                    digest.update(obtained.digest());
                    match obtained.observed_at_unix_ms() {
                        Some(time) => {
                            digest.update([1]);
                            digest.update(time.get().to_be_bytes());
                        }
                        None => digest.update([0]),
                    }
                }
                None => digest.update([0]),
            }
        }
        match self.installation() {
            Some(installed) => {
                digest.update([1]);
                digest.update(installed.transaction_id().as_bytes());
                digest.update(installed.installed_at().get().to_be_bytes());
                digest.update(installed.basis().transaction_id().as_bytes());
            }
            None => digest.update([0]),
        }
        for work in self.publications() {
            digest.update(work.evidence().recipient().as_bytes());
            digest.update(work.transaction_id().as_bytes());
            digest.update(work.queued_at().get().to_be_bytes());
        }
        for attempt in self.delivery_attempts() {
            digest.update(attempt.work().evidence().recipient().as_bytes());
            digest.update(attempt.intent().digest());
            digest.update(attempt.envelope_digest());
            digest.update(attempt.transaction_id().as_bytes());
            digest.update(attempt.retained_at().get().to_be_bytes());
            digest.update(attempt.target_ordinals());
        }
        for observation in self.delivery_observations() {
            digest.update(observation.work().evidence().recipient().as_bytes());
            digest.update(observation.request_id().as_str().as_bytes());
            digest.update(observation.event_id().as_bytes());
            digest.update(observation.envelope_digest());
            digest.update(
                observation
                    .target_receipt()
                    .target()
                    .fingerprint()
                    .as_str()
                    .as_bytes(),
            );
            digest.update([observation.kind().code()]);
            digest.update([
                observation.target_ordinal(),
                u8::from(observation.target_receipt().was_attempted()),
            ]);
            let outcome = observation.target_receipt().outcome();
            digest.update([match outcome.kind() {
                radroots_transport::outcome::DeliveryOutcomeKind::Accepted => 0,
                radroots_transport::outcome::DeliveryOutcomeKind::Delivered => 1,
                radroots_transport::outcome::DeliveryOutcomeKind::Rejected => 2,
                radroots_transport::outcome::DeliveryOutcomeKind::Unavailable => 3,
                radroots_transport::outcome::DeliveryOutcomeKind::Failed => 4,
            }]);
            digest.update([match outcome.retryability() {
                radroots_transport::outcome::Retryability::NotApplicable => 0,
                radroots_transport::outcome::Retryability::Retryable => 1,
                radroots_transport::outcome::Retryability::Terminal => 2,
            }]);
            for value in [outcome.code(), outcome.message()] {
                match value {
                    Some(value) => {
                        digest.update([1]);
                        digest.update((value.len() as u32).to_be_bytes());
                        digest.update(value.as_bytes());
                    }
                    None => digest.update([0]),
                }
            }
            match observation.observed_at_unix_ms() {
                Some(time) => {
                    digest.update([1]);
                    digest.update(time.get().to_be_bytes());
                }
                None => digest.update([0]),
            }
        }
        radroots_domain::CheckpointDigest::from_bytes(digest.finalize().into())
    }
    /// Checks the exact complete recipient set, original claim scope and obtained
    /// references. Concrete adapters must independently authenticate all rows and
    /// compare both retained and absent facts in one coherent transaction.
    pub fn from_store_report(
        retained: RetainedOperation,
        recipients: Vec<RecipientSnapshot>,
        observed_at: UnixSeconds,
    ) -> Result<Self, OperationError> {
        if recipients.is_empty() || recipients.len() > 2 || observed_at.get() == 0 {
            return Err(OperationError::RecoveryRequired);
        }
        let prepared = retained.prepared();
        let command = prepared.request().command();
        let mut expected = vec![command.context().actor().participant(), command.peer()];
        expected.sort_unstable();
        expected.dedup();
        if recipients.len() != expected.len() {
            return Err(OperationError::RecoveryRequired);
        }
        for (recipient, expected) in recipients.iter().zip(expected) {
            if recipient.recipient != expected {
                return Err(OperationError::RecoveryRequired);
            }
            let Some(bound) = retained.bound_plan() else {
                if recipient.expected_event.is_some()
                    || recipient.claim.is_some()
                    || recipient.obtained.is_some()
                {
                    return Err(OperationError::RecoveryRequired);
                }
                continue;
            };
            let seal = bound
                .plan()
                .seals()
                .iter()
                .find(|seal| seal.recipient() == expected)
                .ok_or(OperationError::RecoveryRequired)?;
            let event = EnvelopeEventId::from_bytes(seal.event_id());
            if recipient.expected_event != Some(event) {
                return Err(OperationError::RecoveryRequired);
            }
            if let Some(claim) = recipient.claim {
                let identity = prepared.identity();
                if claim.workspace != identity.workspace_id()
                    || claim.operation != identity.operation_id()
                    || claim.command != identity.command_digest()
                    || &claim.plan != bound.plan().digest()
                    || claim.recipient != expected
                    || claim.event != event
                    || claim.intent.operation_id() != identity.operation_id()
                    || claim.intent.command_digest() != identity.command_digest().as_bytes()
                    || claim.claimed_at < bound.bound_at()
                    || claim.claimed_at >= prepared.issuance_window().first_issue_not_after()
                {
                    return Err(OperationError::RecoveryRequired);
                }
                if let Some(obtained) = recipient.obtained
                    && (obtained.workspace_id() != claim.workspace
                        || obtained.operation_id() != claim.operation
                        || obtained.command_digest() != claim.command
                        || obtained.attempt_id() != claim.attempt
                        || obtained.recipient() != expected
                        || obtained.event_id() != event)
                {
                    return Err(OperationError::RecoveryRequired);
                }
            } else if recipient.obtained.is_some() {
                return Err(OperationError::RecoveryRequired);
            }
        }
        Ok(Self {
            retained,
            recipients: recipients.into_boxed_slice(),
            observed_at,
            installation: None,
            message: None,
            publications: Vec::new().into_boxed_slice(),
            deliveries: Vec::new().into_boxed_slice(),
            observations: Vec::new().into_boxed_slice(),
        })
    }
    /// Adds structurally checked T2 reports to this data snapshot. The concrete
    /// owner must authenticate and atomically compare these records with the
    /// complete original graph. This transformation performs no installation.
    pub fn with_installation(
        mut self,
        installation: Option<ChatInstallation>,
        publications: Vec<PublicationWork>,
    ) -> Result<Self, OperationError> {
        if !self.deliveries.is_empty() || !self.observations.is_empty() {
            return Err(OperationError::RecoveryRequired);
        }
        let Some(installed) = installation else {
            if !publications.is_empty() {
                return Err(OperationError::RecoveryRequired);
            }
            self.installation = None;
            self.message = None;
            self.publications = Vec::new().into_boxed_slice();
            return Ok(self);
        };
        let bound = self.bound_plan().ok_or(OperationError::RecoveryRequired)?;
        let checked = ChatInstallation::from_committed_store(
            bound,
            installed.basis(),
            installed.transaction_id(),
            installed.installed_at(),
        )?;
        if checked != installed
            || !self
                .recipients()
                .iter()
                .any(|recipient| recipient.obtained() == Some(installed.basis()))
            || publications.is_empty()
            || publications.len() > self.recipients().len()
        {
            return Err(OperationError::RecoveryRequired);
        }
        let mut previous = None;
        let mut basis_linked = false;
        for work in &publications {
            let recipient = work.evidence().recipient();
            if previous.is_some_and(|key| key >= recipient)
                || PublicationWork::from_committed_store(
                    bound,
                    installed,
                    work.evidence(),
                    work.transaction_id(),
                    work.queued_at(),
                )? != *work
                || !self
                    .recipients()
                    .iter()
                    .any(|value| value.obtained() == Some(work.evidence()))
            {
                return Err(OperationError::RecoveryRequired);
            }
            previous = Some(recipient);
            basis_linked |= work.evidence() == installed.basis();
        }
        if !basis_linked {
            return Err(OperationError::RecoveryRequired);
        }
        let command = bound.prepared().request().command();
        let relation = match command.parent() {
            Some(parent) => ChatRelation::Reply { parent },
            None => ChatRelation::Inquire {
                source: command.source().map(|source| source.event_id()),
            },
        };
        let message = ChatMessage::new(
            installed.message_id(),
            command.thread_id(),
            command.context().actor().participant(),
            command.peer(),
            bound.plan().created_at(),
            relation,
            command.text(),
        )
        .map_err(|_| OperationError::RecoveryRequired)?;
        self.installation = Some(installed);
        self.message = Some(message);
        self.publications = publications.into_boxed_slice();
        Ok(self)
    }
    /// Actual retained installation report, when supplied by the concrete owner.
    pub const fn installation(&self) -> Option<ChatInstallation> {
        self.installation
    }
    /// Owned bounded logical business result, reconstructed from the original
    /// protected command and complete plan, never a caller's replacement body.
    pub fn installed_message(&self) -> Option<&ChatMessage> {
        self.message.as_ref()
    }
    /// Independent retained recipient publication work. No outer envelope or
    /// relay observation is inferred from these references.
    pub fn publications(&self) -> &[PublicationWork] {
        &self.publications
    }
    /// Adds bounded original D references and append-only target observations.
    /// The native owner must authenticate and fence the complete actual graph;
    /// constructing these values proves neither execution nor permission.
    pub fn with_delivery(
        mut self,
        mut attempts: Vec<DeliveryAttemptSummary>,
        mut observations: Vec<DeliveryObservation>,
    ) -> Result<Self, OperationError> {
        if attempts.len() > 384 || observations.len() > 2304 {
            return Err(OperationError::RecoveryRequired);
        }
        attempts.sort_by(|a, b| {
            (
                a.work().evidence().recipient(),
                a.intent().request_id().as_str(),
            )
                .cmp(&(
                    b.work().evidence().recipient(),
                    b.intent().request_id().as_str(),
                ))
        });
        let mut keys = BTreeSet::new();
        let mut transactions = BTreeSet::new();
        let mut counters = BTreeMap::<(PublicKey, &str), BTreeSet<u8>>::new();
        let mut slots = 0usize;
        let mut policies = BTreeMap::new();
        for attempt in &attempts {
            let recipient = attempt.work().evidence().recipient();
            slots = slots
                .checked_add(attempt.intent().selection().len())
                .filter(|n| *n <= 384)
                .ok_or(OperationError::RecoveryRequired)?;
            if !self.publications.contains(&attempt.work())
                || !keys.insert((recipient, attempt.intent().request_id().as_str()))
                || !transactions.insert(attempt.transaction_id())
                || attempt.target_ordinals().len() != attempt.intent().selection().len()
                || policies
                    .insert(recipient, *attempt.intent().publication_policy_digest())
                    .is_some_and(|previous| {
                        previous != *attempt.intent().publication_policy_digest()
                    })
            {
                return Err(OperationError::RecoveryRequired);
            }
            for (target, ordinal) in attempt
                .intent()
                .selected_targets()
                .zip(attempt.target_ordinals())
            {
                if !(1..=12).contains(ordinal)
                    || !counters
                        .entry((recipient, target.uri().as_str()))
                        .or_default()
                        .insert(*ordinal)
                {
                    return Err(OperationError::RecoveryRequired);
                }
            }
        }
        if counters
            .values()
            .any(|values| values.iter().copied().ne(1..=values.len() as u8))
        {
            return Err(OperationError::RecoveryRequired);
        }
        observations.sort_by(|a, b| {
            (
                a.work().evidence().recipient(),
                a.request_id().as_str(),
                a.target_receipt().target().fingerprint().as_str(),
                a.kind().code(),
            )
                .cmp(&(
                    b.work().evidence().recipient(),
                    b.request_id().as_str(),
                    b.target_receipt().target().fingerprint().as_str(),
                    b.kind().code(),
                ))
        });
        let mut seen = BTreeSet::new();
        for observation in &observations {
            let attempt = attempts
                .iter()
                .find(|attempt| {
                    attempt.work() == observation.work()
                        && attempt.intent().request_id() == observation.request_id()
                })
                .ok_or(OperationError::RecoveryRequired)?;
            let ordinal = attempt
                .intent()
                .selection()
                .iter()
                .position(|target| target == observation.target_receipt().target().fingerprint())
                .and_then(|index| attempt.target_ordinals().get(index))
                .copied();
            if ordinal != Some(observation.target_ordinal()) {
                return Err(OperationError::RecoveryRequired);
            }
            if observation.event_id() != attempt.intent().event_id()
                || observation.envelope_digest() != attempt.envelope_digest()
                || !attempt
                    .intent()
                    .targets()
                    .targets()
                    .contains(observation.target_receipt().target())
                || !seen.insert((
                    observation.work().evidence().recipient(),
                    observation.request_id().as_str(),
                    observation.target_receipt().target().fingerprint().as_str(),
                    observation.kind().code(),
                ))
            {
                return Err(OperationError::RecoveryRequired);
            }
        }
        self.deliveries = attempts.into_boxed_slice();
        self.observations = observations.into_boxed_slice();
        Ok(self)
    }
    /// Original exact-envelope/target intent references, with no duplicated raw payload.
    pub fn delivery_attempts(&self) -> &[DeliveryAttemptSummary] {
        &self.deliveries
    }
    /// Individual original per-attempt target facts; absence is never an ACK.
    pub fn delivery_observations(&self) -> &[DeliveryObservation] {
        &self.observations
    }
    /// Evaluates the original full publication criterion across all exact
    /// attempts for this work. Retry selection never narrows the criterion;
    /// an earlier acceptance remains authoritative across later failures.
    pub fn publication_accepted(&self, work: PublicationWork) -> Result<bool, OperationError> {
        if !self.publications.contains(&work) {
            return Err(OperationError::InvalidEvidence);
        }
        let Some(original) = self
            .deliveries
            .iter()
            .find(|attempt| attempt.work() == work)
        else {
            return Ok(false);
        };
        radroots_transport::policy::evaluate_satisfaction(
            original.intent().policy(),
            original.intent().targets(),
            self.observations
                .iter()
                .filter(|value| value.work() == work)
                .map(|value| {
                    (
                        value.target_receipt().target().fingerprint(),
                        value.target_receipt().outcome(),
                    )
                }),
        )
        .map(|state| state == radroots_transport::policy::SatisfactionState::Satisfied)
        .map_err(|_| OperationError::RecoveryRequired)
    }
    /// Original preparation, without a fabricated installed business result.
    pub fn prepared(&self) -> &PreparedOperation {
        self.retained.prepared()
    }
    /// Complete retained plan, when present.
    pub fn bound_plan(&self) -> Option<&BoundSigningPlan> {
        self.retained.bound_plan()
    }
    /// Exact original logical message identity, separate from seal event IDs.
    pub fn message_id(&self) -> Option<MessageId> {
        self.bound_plan().map(|bound| bound.plan().message_id())
    }
    /// Complete sorted unique recipient set and independent outcomes.
    pub fn recipients(&self) -> &[RecipientSnapshot] {
        &self.recipients
    }
    /// Actual query clock observation; never an issuance restart or global
    /// freshness guarantee.
    pub const fn observed_at(&self) -> UnixSeconds {
        self.observed_at
    }
}
impl fmt::Debug for OperationSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OperationSnapshot(<redacted>)")
    }
}

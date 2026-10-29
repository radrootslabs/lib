//! Bounded stock delivery projections; original outcomes remain separately paged.
use super::*;
use crate::command::{DeliveryIntent, DeliveryObservationKind};
use core::{fmt, num::NonZeroU64};
use radroots_domain::Sequence;
use radroots_protocol::intent::ArtifactRole;
use radroots_transport::{
    outcome::DeliveryOutcome,
    policy::{SatisfactionState, evaluate_satisfaction},
    target::TargetFingerprint,
};

/// One original endpoint's bounded progress. Counts are potential invocations;
/// acceptance is a relay observation, never physical receipt or business state.
#[derive(Clone, Eq, PartialEq)]
pub struct StockDeliveryTargetProgress {
    target: TargetFingerprint,
    attempts: u8,
    accepted: bool,
    last: Option<DeliveryObservationKind>,
    observed: Option<NonZeroU64>,
}
impl StockDeliveryTargetProgress {
    pub fn from_store_report(
        target: TargetFingerprint,
        attempts: u8,
        accepted: bool,
        last: Option<DeliveryObservationKind>,
        observed: Option<NonZeroU64>,
    ) -> Result<Self, OperationError> {
        if attempts > 12
            || (attempts == 0 && (accepted || last.is_some()))
            || (accepted && last.is_none())
            || (last == Some(DeliveryObservationKind::Accepted) && !accepted)
            || (last.is_none() && observed.is_some())
            || observed.is_some_and(|time| time.get() > 253_402_300_799_999)
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            target,
            attempts,
            accepted,
            last,
            observed,
        })
    }
    pub const fn target(&self) -> &TargetFingerprint {
        &self.target
    }
    pub const fn potential_attempts(&self) -> u8 {
        self.attempts
    }
    pub const fn accepted(&self) -> bool {
        self.accepted
    }
    pub const fn last_observation_kind(&self) -> Option<DeliveryObservationKind> {
        self.last
    }
    pub const fn last_observed_at_unix_ms(&self) -> Option<NonZeroU64> {
        self.observed
    }
}
impl fmt::Debug for StockDeliveryTargetProgress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StockDeliveryTargetProgress(<redacted>)")
    }
}

/// At most sixteen endpoint summaries and one original publication policy.
/// No collection duplicates the signed outer for every retained attempt.
#[derive(Clone, Eq, PartialEq)]
pub struct StockDeliverySummary {
    source: Option<StockEnvelopeReceipt>,
    first: Option<DeliveryIntent>,
    policy_source: Option<StockEnvelopeReceipt>,
    attempts: u16,
    targets: Box<[StockDeliveryTargetProgress]>,
    checkpoint: InventoryCheckpointRef,
}
impl StockDeliverySummary {
    /// Validates report shape only; current access and original protected history
    /// must be authenticated by the concrete owner in the actual read transaction.
    pub fn from_store_report(
        request: &StockEnvelopeRequest,
        source: Option<StockEnvelopeReceipt>,
        first: Option<DeliveryIntent>,
        attempts: u16,
        targets: Vec<StockDeliveryTargetProgress>,
        checkpoint: InventoryCheckpointRef,
    ) -> Result<Self, OperationError> {
        let policy_source = first.as_ref().and(source);
        Self::from_store_report_with_policy_source(
            request,
            source,
            policy_source,
            first,
            attempts,
            targets,
            checkpoint,
        )
    }
    /// Validates an envelope-scoped projection while retaining the actual first
    /// publication policy's source. A new carrier cannot renew its target set,
    /// criterion or cumulative invocation budget. Reports grant no dispatch.
    pub fn from_store_report_with_policy_source(
        request: &StockEnvelopeRequest,
        source: Option<StockEnvelopeReceipt>,
        policy_source: Option<StockEnvelopeReceipt>,
        first: Option<DeliveryIntent>,
        attempts: u16,
        targets: Vec<StockDeliveryTargetProgress>,
        checkpoint: InventoryCheckpointRef,
    ) -> Result<Self, OperationError> {
        if checkpoint.authority() != request.binding().input().authority()
            || checkpoint.revision().get() == 0
            || attempts > 16 * 12
            || targets.len() > 16
        {
            return Err(OperationError::RecoveryRequired);
        }
        if let Some(source) = source {
            let original = source.attempt();
            if original.workspace_id() != request.binding().input().workspace_id()
                || original.operation_id() != request.operation_id()
                || original.command_digest() != request.command_digest()
                || source.recipient() != request.recipient()
                || source.checkpoint().authority() != checkpoint.authority()
                || source.checkpoint().revision() > checkpoint.revision()
                || request
                    .envelope_event_id()
                    .is_some_and(|event| event != source.event_id())
            {
                return Err(OperationError::RecoveryRequired);
            }
        }
        if let Some(policy_source) = policy_source {
            let selected = source.ok_or(OperationError::RecoveryRequired)?;
            let policy_attempt = policy_source.attempt();
            let selected_attempt = selected.attempt();
            if policy_source.evidence() != selected.evidence()
                || policy_source.installation_transaction_id()
                    != selected.installation_transaction_id()
                || policy_attempt.workspace_id() != selected_attempt.workspace_id()
                || policy_attempt.operation_id() != selected_attempt.operation_id()
                || policy_attempt.command_digest() != selected_attempt.command_digest()
                || policy_source.checkpoint().authority() != checkpoint.authority()
                || policy_source.checkpoint().revision() > checkpoint.revision()
                || first.is_none()
            {
                return Err(OperationError::RecoveryRequired);
            }
        }
        match &first {
            None if attempts != 0 || !targets.is_empty() => {
                return Err(OperationError::RecoveryRequired);
            }
            Some(intent)
                if attempts == 0
                    || intent.role() != ArtifactRole::GiftWrap
                    || policy_source
                        .is_none_or(|source| source.event_id() != intent.event_id())
                    || intent
                        .targets()
                        .targets()
                        .iter()
                        .map(|target| target.fingerprint())
                        .ne(targets.iter().map(|target| target.target()))
                    || targets
                        .iter()
                        .any(|target| u16::from(target.attempts) > attempts)
                    || targets
                        .iter()
                        .map(|target| u16::from(target.attempts))
                        .sum::<u16>()
                        < attempts =>
            {
                return Err(OperationError::RecoveryRequired);
            }
            _ => {}
        }
        Ok(Self {
            source,
            first,
            policy_source,
            attempts,
            targets: targets.into_boxed_slice(),
            checkpoint,
        })
    }
    pub const fn source(&self) -> Option<StockEnvelopeReceipt> {
        self.source
    }
    pub const fn original_intent(&self) -> Option<&DeliveryIntent> {
        self.first.as_ref()
    }
    /// Source of the first immutable target policy, which may differ from the
    /// explicitly selected rewrapped envelope.
    pub const fn publication_policy_source(&self) -> Option<StockEnvelopeReceipt> {
        self.policy_source
    }
    pub const fn attempt_count(&self) -> u16 {
        self.attempts
    }
    pub fn targets(&self) -> &[StockDeliveryTargetProgress] {
        &self.targets
    }
    pub const fn checkpoint(&self) -> InventoryCheckpointRef {
        self.checkpoint
    }
    /// Applies the shared transport-owned law to sticky original acceptances.
    pub fn acceptance_satisfied(&self) -> Result<bool, OperationError> {
        let Some(intent) = &self.first else {
            return Ok(false);
        };
        let accepted = DeliveryOutcome::accepted();
        evaluate_satisfaction(
            intent.policy(),
            intent.targets(),
            self.targets
                .iter()
                .filter(|target| target.accepted)
                .map(|target| (target.target(), &accepted)),
        )
        .map(|state| state == SatisfactionState::Satisfied)
        .map_err(|_| OperationError::RecoveryRequired)
    }
}
impl fmt::Debug for StockDeliverySummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StockDeliverySummary(<redacted>)")
    }
}

/// A bounded original-outcome page, ordered by protected stock revision.
pub struct StockDeliveryObservationPage {
    entries: Vec<StockDeliveryObservationReceipt>,
    next: Option<Sequence>,
    checkpoint: InventoryCheckpointRef,
}
impl StockDeliveryObservationPage {
    pub fn from_store_report(
        request: &StockEnvelopeRequest,
        entries: Vec<StockDeliveryObservationReceipt>,
        more: bool,
        checkpoint: InventoryCheckpointRef,
    ) -> Result<Self, OperationError> {
        if entries.len() > 16
            || (entries.is_empty() && more)
            || checkpoint.authority() != request.binding().input().authority()
            || entries
                .windows(2)
                .any(|pair| pair[0].checkpoint().revision() >= pair[1].checkpoint().revision())
        {
            return Err(OperationError::RecoveryRequired);
        }
        for entry in &entries {
            let original = entry.observation().source();
            if original.attempt().workspace_id() != request.binding().input().workspace_id()
                || original.attempt().operation_id() != request.operation_id()
                || original.attempt().command_digest() != request.command_digest()
                || original.recipient() != request.recipient()
                || entry.checkpoint().authority() != checkpoint.authority()
                || entry.checkpoint().revision() > checkpoint.revision()
                || request
                    .envelope_event_id()
                    .is_some_and(|event| event != original.event_id())
            {
                return Err(OperationError::RecoveryRequired);
            }
        }
        let next = if more {
            entries.last().map(|entry| entry.checkpoint().revision())
        } else {
            None
        };
        Ok(Self {
            entries,
            next,
            checkpoint,
        })
    }
    pub fn entries(&self) -> &[StockDeliveryObservationReceipt] {
        &self.entries
    }
    pub const fn next_after(&self) -> Option<Sequence> {
        self.next
    }
    pub const fn checkpoint(&self) -> InventoryCheckpointRef {
        self.checkpoint
    }
}
impl fmt::Debug for StockDeliveryObservationPage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StockDeliveryObservationPage(<redacted>)")
    }
}

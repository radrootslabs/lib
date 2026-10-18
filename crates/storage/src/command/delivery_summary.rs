use super::{
    DeliveryAttempt, DeliveryIntent, DeliveryObservation, OperationError, PublicationWork,
};
use core::fmt;
use radroots_domain::{TransactionId, UnixSeconds};

/// Bounded original delivery references without another copy of its signed
/// bytes. This data projection grants no dispatch permission or storage proof.
#[derive(Clone, Eq, PartialEq)]
pub struct DeliveryAttemptSummary {
    work: PublicationWork,
    intent: DeliveryIntent,
    digest: [u8; 32],
    transaction: TransactionId,
    retained_at: UnixSeconds,
    ordinals: Box<[u8]>,
}
impl DeliveryAttemptSummary {
    /// Copies original report facts, never a scheduling capability.
    pub fn from_attempt(attempt: &DeliveryAttempt) -> Self {
        Self {
            work: attempt.work(),
            intent: attempt.intent().clone(),
            digest: *attempt.envelope_digest(),
            transaction: attempt.transaction_id(),
            retained_at: attempt.retained_at(),
            ordinals: attempt.target_ordinals().into(),
        }
    }
    /// Original protected publication linkage.
    pub const fn work(&self) -> PublicationWork {
        self.work
    }
    /// Complete immutable original transport intent.
    pub const fn intent(&self) -> &DeliveryIntent {
        &self.intent
    }
    /// SHA-256 of the original exact signed outer representation.
    pub const fn envelope_digest(&self) -> &[u8; 32] {
        &self.digest
    }
    /// Actual original retention transaction report.
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
    /// Original retention time, never a renewed lease or deadline.
    pub const fn retained_at(&self) -> UnixSeconds {
        self.retained_at
    }
    /// Original endpoint attempt numbers in target order.
    pub fn target_ordinals(&self) -> &[u8] {
        &self.ordinals
    }
    /// Evaluates only this attempt's retained exact target evidence with the
    /// shared acceptance law. A success does not prove recipient delivery.
    pub fn acceptance_satisfied(
        &self,
        observations: &[DeliveryObservation],
    ) -> Result<bool, OperationError> {
        let relevant = observations.iter().filter(|value| {
            value.work() == self.work && value.request_id() == self.intent.request_id()
        });
        for value in relevant.clone() {
            if value.event_id() != self.intent.event_id()
                || value.envelope_digest() != &self.digest
                || !self
                    .intent
                    .targets()
                    .targets()
                    .contains(value.target_receipt().target())
            {
                return Err(OperationError::InvalidEvidence);
            }
        }
        radroots_transport::policy::evaluate_satisfaction(
            self.intent.policy(),
            self.intent.targets(),
            relevant.map(|value| {
                (
                    value.target_receipt().target().fingerprint(),
                    value.target_receipt().outcome(),
                )
            }),
        )
        .map(|value| value == radroots_transport::policy::SatisfactionState::Satisfied)
        .map_err(|_| OperationError::InvalidEvidence)
    }
}
impl fmt::Debug for DeliveryAttemptSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeliveryAttemptSummary(<redacted>)")
    }
}

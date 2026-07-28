use super::{
    RadrootsEventAdmissionStatus, RadrootsEventStoreSourceGeneration, RadrootsStoredRawEvent,
};
use radroots_event::ids::RadrootsEventId;
pub use radroots_event_codec::deletion::reconciliation_v1::evaluator::{
    RadrootsNip09SuppressionOutcome, RadrootsNip09SuppressionReason,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsNip09SuppressionEvidenceV1 {
    pub(crate) outcome: RadrootsNip09SuppressionOutcome,
    pub(crate) reason: RadrootsNip09SuppressionReason,
    pub(crate) event_reference_request_id: Option<RadrootsEventId>,
    pub(crate) address_reference_request_id: Option<RadrootsEventId>,
    pub(crate) address_reference_cutoff: Option<u64>,
}

impl RadrootsNip09SuppressionEvidenceV1 {
    pub const fn outcome(&self) -> RadrootsNip09SuppressionOutcome {
        self.outcome
    }

    pub const fn reason(&self) -> RadrootsNip09SuppressionReason {
        self.reason
    }

    pub const fn event_reference_request_id(&self) -> Option<&RadrootsEventId> {
        self.event_reference_request_id.as_ref()
    }

    pub const fn address_reference_request_id(&self) -> Option<&RadrootsEventId> {
        self.address_reference_request_id.as_ref()
    }

    pub const fn address_reference_cutoff(&self) -> Option<u64> {
        self.address_reference_cutoff
    }

    pub(crate) fn is_coherent_for_event(&self, kind: u32, created_at: u64) -> bool {
        let event_reference = self.event_reference_request_id.is_some();
        let address_reference = self.address_reference_request_id.is_some();
        let cutoff = self.address_reference_cutoff;
        if address_reference != cutoff.is_some() {
            return false;
        }
        if kind == 5 {
            return self.outcome == RadrootsNip09SuppressionOutcome::Visible
                && self.reason == RadrootsNip09SuppressionReason::DeletionRequestImmune
                && !event_reference
                && !address_reference;
        }
        match self.reason {
            RadrootsNip09SuppressionReason::DeletionRequestImmune => false,
            RadrootsNip09SuppressionReason::NoAuthorizedReference
            | RadrootsNip09SuppressionReason::RequestAuthorMismatch => {
                self.outcome == RadrootsNip09SuppressionOutcome::Visible
                    && !event_reference
                    && !address_reference
            }
            RadrootsNip09SuppressionReason::AddressCutoffPrecedesTarget => {
                self.outcome == RadrootsNip09SuppressionOutcome::Visible
                    && !event_reference
                    && cutoff.is_some_and(|value| value < created_at)
            }
            RadrootsNip09SuppressionReason::EventIdReference => {
                self.outcome == RadrootsNip09SuppressionOutcome::Suppressed
                    && event_reference
                    && cutoff.is_none_or(|value| value < created_at)
            }
            RadrootsNip09SuppressionReason::AddressReferenceAtOrBeforeCutoff => {
                self.outcome == RadrootsNip09SuppressionOutcome::Suppressed
                    && !event_reference
                    && cutoff.is_some_and(|value| value >= created_at)
            }
            RadrootsNip09SuppressionReason::EventIdAndAddressReference => {
                self.outcome == RadrootsNip09SuppressionOutcome::Suppressed
                    && event_reference
                    && cutoff.is_some_and(|value| value >= created_at)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsCurrentVisibilityDecisionV1 {
    Visible,
    NotAdmitted,
    NotCurrent,
    Suppressed,
}

impl RadrootsCurrentVisibilityDecisionV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::NotAdmitted => "not_admitted",
            Self::NotCurrent => "not_current",
            Self::Suppressed => "suppressed",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, crate::RadrootsEventStoreError> {
        match value {
            "visible" => Ok(Self::Visible),
            "not_admitted" => Ok(Self::NotAdmitted),
            "not_current" => Ok(Self::NotCurrent),
            "suppressed" => Ok(Self::Suppressed),
            _ => Err(crate::RadrootsEventStoreError::InvalidStoredEnum {
                field: "current_visibility.current_visibility",
                value: value.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsCurrentEventVisibilityV1 {
    pub(crate) source_generation: RadrootsEventStoreSourceGeneration,
    pub(crate) event: RadrootsStoredRawEvent,
    pub(crate) is_raw_head: bool,
    pub(crate) raw_head_event_id: Option<RadrootsEventId>,
    pub(crate) suppression: Option<RadrootsNip09SuppressionEvidenceV1>,
    pub(crate) decision: RadrootsCurrentVisibilityDecisionV1,
}

impl RadrootsCurrentEventVisibilityV1 {
    pub const fn source_generation(&self) -> RadrootsEventStoreSourceGeneration {
        self.source_generation
    }

    pub fn event(&self) -> &RadrootsStoredRawEvent {
        &self.event
    }

    pub const fn admission_status(&self) -> RadrootsEventAdmissionStatus {
        self.event.admission_status
    }

    pub const fn is_raw_head(&self) -> bool {
        self.is_raw_head
    }

    pub const fn raw_head_event_id(&self) -> Option<&RadrootsEventId> {
        self.raw_head_event_id.as_ref()
    }

    pub const fn suppression(&self) -> Option<&RadrootsNip09SuppressionEvidenceV1> {
        self.suppression.as_ref()
    }

    pub const fn decision(&self) -> RadrootsCurrentVisibilityDecisionV1 {
        self.decision
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event_id(byte: char) -> RadrootsEventId {
        RadrootsEventId::parse(core::iter::repeat_n(byte, 64).collect::<String>())
            .expect("event id")
    }

    fn evidence(
        outcome: RadrootsNip09SuppressionOutcome,
        reason: RadrootsNip09SuppressionReason,
        event_reference: bool,
        address_cutoff: Option<u64>,
    ) -> RadrootsNip09SuppressionEvidenceV1 {
        RadrootsNip09SuppressionEvidenceV1 {
            outcome,
            reason,
            event_reference_request_id: event_reference.then(|| event_id('a')),
            address_reference_request_id: address_cutoff.map(|_| event_id('b')),
            address_reference_cutoff: address_cutoff,
        }
    }

    #[test]
    fn visibility_decisions_round_trip_and_reject_unknown_values() {
        for decision in [
            RadrootsCurrentVisibilityDecisionV1::Visible,
            RadrootsCurrentVisibilityDecisionV1::NotAdmitted,
            RadrootsCurrentVisibilityDecisionV1::NotCurrent,
            RadrootsCurrentVisibilityDecisionV1::Suppressed,
        ] {
            assert_eq!(
                RadrootsCurrentVisibilityDecisionV1::parse(decision.as_str()).expect("decision"),
                decision
            );
        }
        assert!(RadrootsCurrentVisibilityDecisionV1::parse("unknown").is_err());
    }

    #[test]
    fn suppression_evidence_coherence_covers_every_protocol_reason() {
        use RadrootsNip09SuppressionOutcome::{Suppressed, Visible};
        use RadrootsNip09SuppressionReason::{
            AddressCutoffPrecedesTarget, AddressReferenceAtOrBeforeCutoff, DeletionRequestImmune,
            EventIdAndAddressReference, EventIdReference, NoAuthorizedReference,
            RequestAuthorMismatch,
        };

        assert!(evidence(Visible, DeletionRequestImmune, false, None).is_coherent_for_event(5, 10));
        assert!(
            !evidence(Suppressed, DeletionRequestImmune, false, None).is_coherent_for_event(5, 10)
        );
        assert!(
            !evidence(Visible, NoAuthorizedReference, false, None).is_coherent_for_event(5, 10)
        );
        assert!(!evidence(Visible, DeletionRequestImmune, true, None).is_coherent_for_event(5, 10));
        assert!(
            !evidence(Visible, DeletionRequestImmune, false, None).is_coherent_for_event(1, 10)
        );

        for reason in [NoAuthorizedReference, RequestAuthorMismatch] {
            assert!(evidence(Visible, reason, false, None).is_coherent_for_event(1, 10));
            assert!(!evidence(Suppressed, reason, false, None).is_coherent_for_event(1, 10));
            assert!(!evidence(Visible, reason, true, None).is_coherent_for_event(1, 10));
            assert!(!evidence(Visible, reason, false, Some(9)).is_coherent_for_event(1, 10));
        }

        assert!(
            evidence(Visible, AddressCutoffPrecedesTarget, false, Some(9))
                .is_coherent_for_event(1, 10)
        );
        assert!(
            !evidence(Visible, AddressCutoffPrecedesTarget, false, Some(10))
                .is_coherent_for_event(1, 10)
        );
        assert!(
            !evidence(Suppressed, AddressCutoffPrecedesTarget, false, Some(9))
                .is_coherent_for_event(1, 10)
        );
        assert!(
            !evidence(Visible, AddressCutoffPrecedesTarget, true, Some(9))
                .is_coherent_for_event(1, 10)
        );

        assert!(evidence(Suppressed, EventIdReference, true, None).is_coherent_for_event(1, 10));
        assert!(evidence(Suppressed, EventIdReference, true, Some(9)).is_coherent_for_event(1, 10));
        assert!(
            !evidence(Suppressed, EventIdReference, true, Some(10)).is_coherent_for_event(1, 10)
        );
        assert!(!evidence(Visible, EventIdReference, true, None).is_coherent_for_event(1, 10));
        assert!(!evidence(Suppressed, EventIdReference, false, None).is_coherent_for_event(1, 10));

        assert!(
            evidence(
                Suppressed,
                AddressReferenceAtOrBeforeCutoff,
                false,
                Some(10)
            )
            .is_coherent_for_event(1, 10)
        );
        assert!(
            !evidence(Suppressed, AddressReferenceAtOrBeforeCutoff, false, Some(9))
                .is_coherent_for_event(1, 10)
        );
        assert!(
            !evidence(Visible, AddressReferenceAtOrBeforeCutoff, false, Some(10))
                .is_coherent_for_event(1, 10)
        );
        assert!(
            !evidence(Suppressed, AddressReferenceAtOrBeforeCutoff, true, Some(10))
                .is_coherent_for_event(1, 10)
        );

        assert!(
            evidence(Suppressed, EventIdAndAddressReference, true, Some(10))
                .is_coherent_for_event(1, 10)
        );
        assert!(
            !evidence(Visible, EventIdAndAddressReference, true, Some(10))
                .is_coherent_for_event(1, 10)
        );
        assert!(
            !evidence(Suppressed, EventIdAndAddressReference, false, Some(10))
                .is_coherent_for_event(1, 10)
        );
        assert!(
            !evidence(Suppressed, EventIdAndAddressReference, true, Some(9))
                .is_coherent_for_event(1, 10)
        );

        let incoherent = RadrootsNip09SuppressionEvidenceV1 {
            outcome: Visible,
            reason: NoAuthorizedReference,
            event_reference_request_id: None,
            address_reference_request_id: Some(event_id('c')),
            address_reference_cutoff: None,
        };
        assert!(!incoherent.is_coherent_for_event(1, 10));
    }
}

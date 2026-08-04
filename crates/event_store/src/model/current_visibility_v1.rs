use super::{
    RadrootsEventAdmissionStatus, RadrootsEventStoreSourceGeneration, RadrootsStoredRawEvent,
};
use radroots_event::id::EventId;
pub use radroots_event_codec::admission::deletion::reconciliation_v1::{
    RadrootsNip09SuppressionOutcome, RadrootsNip09SuppressionReason,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsNip09SuppressionEvidenceV1 {
    pub(crate) outcome: RadrootsNip09SuppressionOutcome,
    pub(crate) reason: RadrootsNip09SuppressionReason,
    pub(crate) event_reference_request_id: Option<EventId>,
    pub(crate) address_reference_request_id: Option<EventId>,
    pub(crate) address_reference_cutoff: Option<u64>,
}

impl RadrootsNip09SuppressionEvidenceV1 {
    pub const fn outcome(&self) -> RadrootsNip09SuppressionOutcome {
        self.outcome
    }

    pub const fn reason(&self) -> RadrootsNip09SuppressionReason {
        self.reason
    }

    pub const fn event_reference_request_id(&self) -> Option<&EventId> {
        self.event_reference_request_id.as_ref()
    }

    pub const fn address_reference_request_id(&self) -> Option<&EventId> {
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
            return (
                self.outcome,
                self.reason,
                event_reference,
                address_reference,
            ) == (
                RadrootsNip09SuppressionOutcome::Visible,
                RadrootsNip09SuppressionReason::DeletionRequestImmune,
                false,
                false,
            );
        }
        match self.reason {
            RadrootsNip09SuppressionReason::DeletionRequestImmune => false,
            RadrootsNip09SuppressionReason::NoAuthorizedReference
            | RadrootsNip09SuppressionReason::RequestAuthorMismatch => {
                (self.outcome, event_reference, address_reference)
                    == (RadrootsNip09SuppressionOutcome::Visible, false, false)
            }
            RadrootsNip09SuppressionReason::AddressCutoffPrecedesTarget => {
                (
                    self.outcome,
                    event_reference,
                    cutoff.is_some_and(|value| value < created_at),
                ) == (RadrootsNip09SuppressionOutcome::Visible, false, true)
            }
            RadrootsNip09SuppressionReason::EventIdReference => {
                (
                    self.outcome,
                    event_reference,
                    cutoff.is_none_or(|value| value < created_at),
                ) == (RadrootsNip09SuppressionOutcome::Suppressed, true, true)
            }
            RadrootsNip09SuppressionReason::AddressReferenceAtOrBeforeCutoff => {
                (
                    self.outcome,
                    event_reference,
                    cutoff.is_some_and(|value| value >= created_at),
                ) == (RadrootsNip09SuppressionOutcome::Suppressed, false, true)
            }
            RadrootsNip09SuppressionReason::EventIdAndAddressReference => {
                (
                    self.outcome,
                    event_reference,
                    cutoff.is_some_and(|value| value >= created_at),
                ) == (RadrootsNip09SuppressionOutcome::Suppressed, true, true)
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
    pub(crate) raw_head_event_id: Option<EventId>,
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

    pub const fn raw_head_event_id(&self) -> Option<&EventId> {
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

    fn event_id(byte: char) -> EventId {
        EventId::parse(byte.to_string().repeat(64)).expect("event id")
    }

    fn evidence(
        outcome: RadrootsNip09SuppressionOutcome,
        reason: RadrootsNip09SuppressionReason,
        event_reference: bool,
        address_reference: bool,
        cutoff: Option<u64>,
    ) -> RadrootsNip09SuppressionEvidenceV1 {
        RadrootsNip09SuppressionEvidenceV1 {
            outcome,
            reason,
            event_reference_request_id: event_reference.then(|| event_id('a')),
            address_reference_request_id: address_reference.then(|| event_id('b')),
            address_reference_cutoff: cutoff,
        }
    }

    #[test]
    fn suppression_evidence_coherence_covers_every_governed_reason() {
        use RadrootsNip09SuppressionOutcome::{Suppressed, Visible};
        use RadrootsNip09SuppressionReason::{
            AddressCutoffPrecedesTarget, AddressReferenceAtOrBeforeCutoff, DeletionRequestImmune,
            EventIdAndAddressReference, EventIdReference, NoAuthorizedReference,
            RequestAuthorMismatch,
        };

        let immune = evidence(Visible, DeletionRequestImmune, false, false, None);
        assert!(immune.is_coherent_for_event(5, 10));
        assert!(!immune.is_coherent_for_event(1, 10));
        assert!(
            !evidence(Suppressed, DeletionRequestImmune, false, false, None)
                .is_coherent_for_event(5, 10)
        );
        assert!(
            !evidence(Visible, DeletionRequestImmune, true, false, None)
                .is_coherent_for_event(5, 10)
        );

        for reason in [NoAuthorizedReference, RequestAuthorMismatch] {
            assert!(evidence(Visible, reason, false, false, None).is_coherent_for_event(1, 10));
            assert!(!evidence(Suppressed, reason, false, false, None).is_coherent_for_event(1, 10));
            assert!(!evidence(Visible, reason, true, false, None).is_coherent_for_event(1, 10));
        }

        assert!(
            evidence(Visible, AddressCutoffPrecedesTarget, false, true, Some(9))
                .is_coherent_for_event(30_402, 10)
        );
        assert!(
            !evidence(Visible, AddressCutoffPrecedesTarget, false, true, Some(10))
                .is_coherent_for_event(30_402, 10)
        );
        assert!(
            !evidence(
                Suppressed,
                AddressCutoffPrecedesTarget,
                false,
                true,
                Some(9)
            )
            .is_coherent_for_event(30_402, 10)
        );

        assert!(
            evidence(Suppressed, EventIdReference, true, false, None).is_coherent_for_event(1, 10)
        );
        assert!(
            evidence(Suppressed, EventIdReference, true, true, Some(9))
                .is_coherent_for_event(1, 10)
        );
        assert!(
            !evidence(Suppressed, EventIdReference, true, true, Some(10))
                .is_coherent_for_event(1, 10)
        );
        assert!(
            !evidence(Visible, EventIdReference, true, false, None).is_coherent_for_event(1, 10)
        );

        assert!(
            evidence(
                Suppressed,
                AddressReferenceAtOrBeforeCutoff,
                false,
                true,
                Some(10),
            )
            .is_coherent_for_event(30_402, 10)
        );
        assert!(
            !evidence(
                Suppressed,
                AddressReferenceAtOrBeforeCutoff,
                false,
                true,
                Some(9),
            )
            .is_coherent_for_event(30_402, 10)
        );
        assert!(
            !evidence(
                Suppressed,
                AddressReferenceAtOrBeforeCutoff,
                true,
                true,
                Some(10),
            )
            .is_coherent_for_event(30_402, 10)
        );

        assert!(
            evidence(Suppressed, EventIdAndAddressReference, true, true, Some(10),)
                .is_coherent_for_event(30_402, 10)
        );
        assert!(
            !evidence(Suppressed, EventIdAndAddressReference, true, true, Some(9),)
                .is_coherent_for_event(30_402, 10)
        );
        assert!(
            !evidence(Visible, EventIdAndAddressReference, true, true, Some(10),)
                .is_coherent_for_event(30_402, 10)
        );

        assert!(
            !evidence(Visible, NoAuthorizedReference, false, true, None)
                .is_coherent_for_event(1, 10)
        );
    }

    #[test]
    fn suppression_and_visibility_accessors_preserve_stable_values() {
        let evidence = evidence(
            RadrootsNip09SuppressionOutcome::Suppressed,
            RadrootsNip09SuppressionReason::EventIdReference,
            true,
            false,
            None,
        );
        assert_eq!(
            evidence.outcome(),
            RadrootsNip09SuppressionOutcome::Suppressed
        );
        assert_eq!(
            evidence.reason(),
            RadrootsNip09SuppressionReason::EventIdReference
        );
        assert!(evidence.event_reference_request_id().is_some());
        assert!(evidence.address_reference_request_id().is_none());
        assert_eq!(evidence.address_reference_cutoff(), None);

        for (raw, decision) in [
            ("visible", RadrootsCurrentVisibilityDecisionV1::Visible),
            (
                "not_admitted",
                RadrootsCurrentVisibilityDecisionV1::NotAdmitted,
            ),
            (
                "not_current",
                RadrootsCurrentVisibilityDecisionV1::NotCurrent,
            ),
            (
                "suppressed",
                RadrootsCurrentVisibilityDecisionV1::Suppressed,
            ),
        ] {
            assert_eq!(decision.as_str(), raw);
            assert_eq!(
                RadrootsCurrentVisibilityDecisionV1::parse(raw).expect("decision"),
                decision
            );
        }
        assert!(matches!(
            RadrootsCurrentVisibilityDecisionV1::parse("retired"),
            Err(crate::RadrootsEventStoreError::InvalidStoredEnum { .. })
        ));
    }
}

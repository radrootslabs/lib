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

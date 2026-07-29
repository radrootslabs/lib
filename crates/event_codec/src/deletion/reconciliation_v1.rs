#![forbid(unsafe_code)]

//! Frozen NIP-09 projection, admission, and suppression semantics.

pub mod admission {
    //! Frozen NIP-09 request-admission semantics for reconciliation v1.

    use core::fmt;

    use radroots_event::contract::registry_v7::{EventContract, event_contract_registry_v7};
    use radroots_event::envelope::EventEnvelope;

    use crate::{
        deletion::reconciliation_v1::inbound::{
            RadrootsInboundNip09DeletionProjection, RadrootsNip09DeletionProjectionError,
        },
        verification::v1::{
            RadrootsNip01VerificationError, RadrootsSignatureVerifiedEvent, verify_nip01_event_v1,
        },
    };

    /// A signature-and-id verified kind-5 event admitted as a NIP-09 request.
    ///
    /// Admission establishes only the request contract. It does not establish that
    /// any requested deletion effect is authorized or applicable.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct RadrootsAdmittedNip09DeletionRequestEventV1 {
        verified_event: RadrootsSignatureVerifiedEvent,
        projection: RadrootsInboundNip09DeletionProjection,
    }

    /// Current compatibility name for the reconciliation-v1 deletion admission.
    pub type RadrootsAdmittedNip09DeletionRequestEvent =
        RadrootsAdmittedNip09DeletionRequestEventV1;

    impl RadrootsAdmittedNip09DeletionRequestEventV1 {
        pub fn verified_event(&self) -> &RadrootsSignatureVerifiedEvent {
            &self.verified_event
        }

        pub fn event(&self) -> &EventEnvelope {
            self.verified_event.event()
        }

        pub const fn projection(&self) -> &RadrootsInboundNip09DeletionProjection {
            &self.projection
        }

        pub fn contract(&self) -> &'static EventContract {
            event_contract_registry_v7(self.projection.contract_id())
                .expect("NIP-09 deletion request contract is registry-owned")
        }

        pub fn into_parts(
            self,
        ) -> (
            RadrootsSignatureVerifiedEvent,
            RadrootsInboundNip09DeletionProjection,
        ) {
            (self.verified_event, self.projection)
        }
    }

    #[non_exhaustive]
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum RadrootsNip09DeletionAdmissionError {
        Nip01Verification(RadrootsNip01VerificationError),
        Projection(RadrootsNip09DeletionProjectionError),
    }

    impl RadrootsNip09DeletionAdmissionError {
        pub const fn code(&self) -> &'static str {
            match self {
                Self::Nip01Verification(error) => error.code(),
                Self::Projection(error) => error.code(),
            }
        }
    }

    impl fmt::Display for RadrootsNip09DeletionAdmissionError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Nip01Verification(error) => write!(formatter, "{error}"),
                Self::Projection(error) => write!(formatter, "{error}"),
            }
        }
    }

    #[cfg(feature = "std")]
    impl std::error::Error for RadrootsNip09DeletionAdmissionError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                Self::Nip01Verification(error) => Some(error),
                Self::Projection(error) => Some(error),
            }
        }
    }

    impl From<RadrootsNip01VerificationError> for RadrootsNip09DeletionAdmissionError {
        fn from(value: RadrootsNip01VerificationError) -> Self {
            Self::Nip01Verification(value)
        }
    }

    impl From<RadrootsNip09DeletionProjectionError> for RadrootsNip09DeletionAdmissionError {
        fn from(value: RadrootsNip09DeletionProjectionError) -> Self {
            Self::Projection(value)
        }
    }

    pub fn admit_verified_nip09_deletion_request_event(
        verified_event: RadrootsSignatureVerifiedEvent,
    ) -> Result<RadrootsAdmittedNip09DeletionRequestEvent, RadrootsNip09DeletionAdmissionError>
    {
        admit_verified_nip09_deletion_request_event_v1(verified_event)
    }

    /// Admits a verified NIP-09 request with reconciliation-v1 semantics.
    pub fn admit_verified_nip09_deletion_request_event_v1(
        verified_event: RadrootsSignatureVerifiedEvent,
    ) -> Result<RadrootsAdmittedNip09DeletionRequestEventV1, RadrootsNip09DeletionAdmissionError>
    {
        let projection =
            super::inbound::project_verified_nip09_deletion_request_event_v1(&verified_event)?;
        Ok(RadrootsAdmittedNip09DeletionRequestEventV1 {
            verified_event,
            projection,
        })
    }

    pub fn verify_and_admit_nip09_deletion_request_event(
        event: EventEnvelope,
    ) -> Result<RadrootsAdmittedNip09DeletionRequestEvent, RadrootsNip09DeletionAdmissionError>
    {
        admit_verified_nip09_deletion_request_event_v1(verify_nip01_event_v1(event)?)
    }

    #[cfg(test)]
    mod tests;
}

pub mod evaluator {
    //! Frozen NIP-09 suppression semantics for reconciliation v1.

    #[cfg(not(feature = "std"))]
    use alloc::format;

    use radroots_event::{
        envelope::kind::KIND_DELETION_REQUEST,
        id::{EventId, Nip01Coordinate},
    };

    use crate::verification::v1::RadrootsSignatureVerifiedEvent;

    use super::admission::RadrootsAdmittedNip09DeletionRequestEventV1;

    /// Whether a verified event remains visible after NIP-09 evaluation.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum RadrootsNip09SuppressionOutcome {
        Visible,
        Suppressed,
    }

    impl RadrootsNip09SuppressionOutcome {
        pub const fn code(self) -> &'static str {
            match self {
                Self::Visible => "visible",
                Self::Suppressed => "suppressed",
            }
        }
    }

    /// The stable explanation for a NIP-09 suppression outcome.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum RadrootsNip09SuppressionReason {
        DeletionRequestImmune,
        NoAuthorizedReference,
        RequestAuthorMismatch,
        AddressCutoffPrecedesTarget,
        EventIdReference,
        AddressReferenceAtOrBeforeCutoff,
        EventIdAndAddressReference,
    }

    impl RadrootsNip09SuppressionReason {
        pub const fn code(self) -> &'static str {
            match self {
                Self::DeletionRequestImmune => "deletion_request_immune",
                Self::NoAuthorizedReference => "deletion_no_authorized_reference",
                Self::RequestAuthorMismatch => "deletion_request_author_mismatch",
                Self::AddressCutoffPrecedesTarget => "deletion_address_cutoff_precedes_target",
                Self::EventIdReference => "deletion_event_id_reference",
                Self::AddressReferenceAtOrBeforeCutoff => "deletion_address_reference",
                Self::EventIdAndAddressReference => "deletion_event_id_and_address_reference",
            }
        }
    }

    /// Canonical evidence for an authorized exact event-id reference.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct RadrootsNip09EventReferenceEvidence {
        request_id: EventId,
    }

    impl RadrootsNip09EventReferenceEvidence {
        pub const fn request_id(&self) -> &EventId {
            &self.request_id
        }
    }

    /// Canonical evidence for authorized address references.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct RadrootsNip09AddressReferenceEvidence {
        coordinate: Nip01Coordinate,
        inclusive_cutoff: u64,
        request_id: EventId,
    }

    impl RadrootsNip09AddressReferenceEvidence {
        pub const fn coordinate(&self) -> &Nip01Coordinate {
            &self.coordinate
        }

        pub const fn inclusive_cutoff(&self) -> u64 {
            self.inclusive_cutoff
        }

        pub const fn request_id(&self) -> &EventId {
            &self.request_id
        }
    }

    /// A pure NIP-09 visibility decision with canonical supporting evidence.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct RadrootsNip09SuppressionDecision {
        outcome: RadrootsNip09SuppressionOutcome,
        reason: RadrootsNip09SuppressionReason,
        event_reference: Option<RadrootsNip09EventReferenceEvidence>,
        address_reference: Option<RadrootsNip09AddressReferenceEvidence>,
    }

    impl RadrootsNip09SuppressionDecision {
        pub const fn outcome(&self) -> RadrootsNip09SuppressionOutcome {
            self.outcome
        }

        pub const fn reason(&self) -> RadrootsNip09SuppressionReason {
            self.reason
        }

        pub const fn event_reference(&self) -> Option<&RadrootsNip09EventReferenceEvidence> {
            self.event_reference.as_ref()
        }

        pub const fn address_reference(&self) -> Option<&RadrootsNip09AddressReferenceEvidence> {
            self.address_reference.as_ref()
        }
    }

    /// Evaluates deterministic NIP-09 suppression without mutating stored events.
    pub fn evaluate_nip09_suppression(
        target: &RadrootsSignatureVerifiedEvent,
        requests: &[RadrootsAdmittedNip09DeletionRequestEventV1],
    ) -> RadrootsNip09SuppressionDecision {
        evaluate_nip09_suppression_v1(target, requests)
    }

    /// Evaluates suppression with the semantics frozen for reconciliation v1.
    pub fn evaluate_nip09_suppression_v1(
        target: &RadrootsSignatureVerifiedEvent,
        requests: &[RadrootsAdmittedNip09DeletionRequestEventV1],
    ) -> RadrootsNip09SuppressionDecision {
        evaluate_nip09_suppression_from_borrowed_requests_v1(target, requests)
    }

    /// Evaluates reconciliation-v1 suppression from borrowed deletion requests.
    ///
    /// This entry point lets indexed stores evaluate only exact request matches
    /// without cloning admitted request payloads. Iteration order does not affect
    /// the canonical evidence reduction.
    pub fn evaluate_nip09_suppression_from_borrowed_requests_v1<'a>(
        target: &RadrootsSignatureVerifiedEvent,
        requests: impl IntoIterator<Item = &'a RadrootsAdmittedNip09DeletionRequestEventV1>,
    ) -> RadrootsNip09SuppressionDecision {
        let target_event = target.event();
        if target_event.kind_u32() == KIND_DELETION_REQUEST {
            return decision(
                RadrootsNip09SuppressionOutcome::Visible,
                RadrootsNip09SuppressionReason::DeletionRequestImmune,
                None,
                None,
            );
        }

        let target_coordinate = nip01_coordinate(target);
        let mut event_reference = None;
        let mut address_reference = None;
        let mut has_unauthorized_reference = false;

        for request in requests {
            let request_event = request.event();
            let projection = request.projection();
            let event_matches = projection
                .event_targets()
                .iter()
                .any(|reference| reference.event_id() == target_event.id());
            let address_match = target_coordinate.as_ref().filter(|coordinate| {
                projection
                    .address_targets()
                    .iter()
                    .any(|reference| reference.coordinate() == *coordinate)
            });
            if !event_matches && address_match.is_none() {
                continue;
            }
            if request_event.author() != target_event.author() {
                has_unauthorized_reference = true;
                continue;
            }

            if event_matches
                && event_reference.as_ref().is_none_or(
                    |current: &RadrootsNip09EventReferenceEvidence| {
                        request_event.id() < current.request_id()
                    },
                )
            {
                event_reference = Some(RadrootsNip09EventReferenceEvidence {
                    request_id: *request_event.id(),
                });
            }
            if let Some(coordinate) = address_match {
                let inclusive_cutoff = request_event.created_at_u64();
                if address_reference.as_ref().is_none_or(
                    |current: &RadrootsNip09AddressReferenceEvidence| {
                        inclusive_cutoff > current.inclusive_cutoff()
                            || (inclusive_cutoff == current.inclusive_cutoff()
                                && request_event.id() < current.request_id())
                    },
                ) {
                    address_reference = Some(RadrootsNip09AddressReferenceEvidence {
                        coordinate: coordinate.clone(),
                        inclusive_cutoff,
                        request_id: *request_event.id(),
                    });
                }
            }
        }

        let address_applies = address_reference
            .as_ref()
            .is_some_and(|reference| target_event.created_at_u64() <= reference.inclusive_cutoff());
        let (outcome, reason) = match (event_reference.is_some(), address_applies) {
            (true, true) => (
                RadrootsNip09SuppressionOutcome::Suppressed,
                RadrootsNip09SuppressionReason::EventIdAndAddressReference,
            ),
            (true, false) => (
                RadrootsNip09SuppressionOutcome::Suppressed,
                RadrootsNip09SuppressionReason::EventIdReference,
            ),
            (false, true) => (
                RadrootsNip09SuppressionOutcome::Suppressed,
                RadrootsNip09SuppressionReason::AddressReferenceAtOrBeforeCutoff,
            ),
            (false, false) if address_reference.is_some() => (
                RadrootsNip09SuppressionOutcome::Visible,
                RadrootsNip09SuppressionReason::AddressCutoffPrecedesTarget,
            ),
            (false, false) if has_unauthorized_reference => (
                RadrootsNip09SuppressionOutcome::Visible,
                RadrootsNip09SuppressionReason::RequestAuthorMismatch,
            ),
            (false, false) => (
                RadrootsNip09SuppressionOutcome::Visible,
                RadrootsNip09SuppressionReason::NoAuthorizedReference,
            ),
        };

        decision(outcome, reason, event_reference, address_reference)
    }

    fn nip01_coordinate(target: &RadrootsSignatureVerifiedEvent) -> Option<Nip01Coordinate> {
        let event = target.event();
        let kind = event.kind_u32();
        let identifier = if matches!(kind, 0 | 3) || (10_000..=19_999).contains(&kind) {
            ""
        } else if (30_000..=39_999).contains(&kind) {
            event
                .tag_slices()
                .iter()
                .find(|tag| tag.as_slice().first().is_some_and(|name| name == "d"))?
                .as_slice()
                .get(1)?
                .as_str()
        } else {
            return None;
        };
        Nip01Coordinate::parse(format!("{kind}:{}:{identifier}", event.author())).ok()
    }

    const fn decision(
        outcome: RadrootsNip09SuppressionOutcome,
        reason: RadrootsNip09SuppressionReason,
        event_reference: Option<RadrootsNip09EventReferenceEvidence>,
        address_reference: Option<RadrootsNip09AddressReferenceEvidence>,
    ) -> RadrootsNip09SuppressionDecision {
        RadrootsNip09SuppressionDecision {
            outcome,
            reason,
            event_reference,
            address_reference,
        }
    }

    #[cfg(all(test, feature = "nostr"))]
    mod tests;
}

pub mod inbound {
    //! Frozen NIP-09 request-projection semantics for reconciliation v1.

    #[cfg(not(feature = "std"))]
    use alloc::{
        collections::{BTreeMap, BTreeSet},
        string::{String, ToString},
        vec::Vec,
    };
    use core::fmt;
    #[cfg(feature = "std")]
    use std::{
        collections::{BTreeMap, BTreeSet},
        string::String,
        vec::Vec,
    };

    use radroots_event::{
        envelope::kind::KIND_DELETION_REQUEST,
        id::{EventId, Nip01Coordinate, Nip01CoordinateParseError, ParseError},
        post::deletion::{
            RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES,
            RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES,
            RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES, RADROOTS_NIP09_DELETION_TAG_MAX_COUNT,
            RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT,
            RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES, RADROOTS_NIP09_DELETION_TARGET_KIND_MAX,
        },
    };

    use crate::verification::v1::RadrootsSignatureVerifiedEvent;

    const RADROOTS_NIP09_DELETION_SIGNED_EVENT_FIXED_BYTES: usize = "{\"id\":\"".len()
        + 64
        + "\",\"pubkey\":\"".len()
        + 64
        + "\",\"created_at\":".len()
        + ",\"kind\":5,\"tags\":".len()
        + ",\"content\":".len()
        + ",\"sig\":\"".len()
        + 128
        + "\"}".len();

    #[non_exhaustive]
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum RadrootsNip09DeletionDiagnostic {
        KindAdvisoryShapeIgnored {
            tag_index: usize,
            raw_tag: Vec<String>,
        },
        KindAdvisoryInvalidIgnored {
            tag_index: usize,
            raw_tag: Vec<String>,
        },
        KindAdvisoryDuplicateIgnored {
            tag_index: usize,
            raw_tag: Vec<String>,
        },
        KindAdvisoryConflictIgnored {
            tag_index: usize,
            raw_tag: Vec<String>,
        },
    }

    impl RadrootsNip09DeletionDiagnostic {
        pub const fn code(&self) -> &'static str {
            match self {
                Self::KindAdvisoryShapeIgnored { .. } => "deletion_kind_advisory_shape_ignored",
                Self::KindAdvisoryInvalidIgnored { .. } => "deletion_kind_advisory_invalid_ignored",
                Self::KindAdvisoryDuplicateIgnored { .. } => {
                    "deletion_kind_advisory_duplicate_ignored"
                }
                Self::KindAdvisoryConflictIgnored { .. } => {
                    "deletion_kind_advisory_conflict_ignored"
                }
            }
        }

        pub const fn tag_index(&self) -> usize {
            match self {
                Self::KindAdvisoryShapeIgnored { tag_index, .. }
                | Self::KindAdvisoryInvalidIgnored { tag_index, .. }
                | Self::KindAdvisoryDuplicateIgnored { tag_index, .. }
                | Self::KindAdvisoryConflictIgnored { tag_index, .. } => *tag_index,
            }
        }

        pub fn raw_tag(&self) -> &[String] {
            match self {
                Self::KindAdvisoryShapeIgnored { raw_tag, .. }
                | Self::KindAdvisoryInvalidIgnored { raw_tag, .. }
                | Self::KindAdvisoryDuplicateIgnored { raw_tag, .. }
                | Self::KindAdvisoryConflictIgnored { raw_tag, .. } => raw_tag,
            }
        }
    }

    impl fmt::Display for RadrootsNip09DeletionDiagnostic {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(self.code())
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct RadrootsInboundNip09DeletionEventTarget {
        tag_index: usize,
        event_id: EventId,
        raw_tag: Vec<String>,
    }

    impl RadrootsInboundNip09DeletionEventTarget {
        pub const fn tag_index(&self) -> usize {
            self.tag_index
        }

        pub const fn event_id(&self) -> &EventId {
            &self.event_id
        }

        pub fn raw_tag(&self) -> &[String] {
            &self.raw_tag
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct RadrootsInboundNip09DeletionAddressTarget {
        tag_index: usize,
        coordinate: Nip01Coordinate,
        raw_tag: Vec<String>,
    }

    impl RadrootsInboundNip09DeletionAddressTarget {
        pub const fn tag_index(&self) -> usize {
            self.tag_index
        }

        pub const fn coordinate(&self) -> &Nip01Coordinate {
            &self.coordinate
        }

        pub fn raw_tag(&self) -> &[String] {
            &self.raw_tag
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct RadrootsInboundNip09DeletionKindAdvisory {
        tag_index: usize,
        kind: u32,
        raw_tag: Vec<String>,
    }

    impl RadrootsInboundNip09DeletionKindAdvisory {
        pub const fn tag_index(&self) -> usize {
            self.tag_index
        }

        pub const fn kind(&self) -> u32 {
            self.kind
        }

        pub fn raw_tag(&self) -> &[String] {
            &self.raw_tag
        }
    }

    /// Tolerant effect-free projection of one verified kind-5 request.
    ///
    /// Raw tags preserve exact source order, duplicates, trailing elements, and
    /// unknown tags. Canonical target and advisory views are unique and sorted,
    /// retaining first-seen source provenance.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct RadrootsInboundNip09DeletionProjection {
        event_targets: Vec<RadrootsInboundNip09DeletionEventTarget>,
        address_targets: Vec<RadrootsInboundNip09DeletionAddressTarget>,
        kind_advisories: Vec<RadrootsInboundNip09DeletionKindAdvisory>,
        diagnostics: Vec<RadrootsNip09DeletionDiagnostic>,
        raw_tags: Vec<Vec<String>>,
    }

    impl RadrootsInboundNip09DeletionProjection {
        pub fn event_targets(&self) -> &[RadrootsInboundNip09DeletionEventTarget] {
            &self.event_targets
        }

        pub fn address_targets(&self) -> &[RadrootsInboundNip09DeletionAddressTarget] {
            &self.address_targets
        }

        pub fn kind_advisories(&self) -> &[RadrootsInboundNip09DeletionKindAdvisory] {
            &self.kind_advisories
        }

        pub fn diagnostics(&self) -> &[RadrootsNip09DeletionDiagnostic] {
            &self.diagnostics
        }

        pub fn raw_tags(&self) -> &[Vec<String>] {
            &self.raw_tags
        }

        pub const fn contract_id(&self) -> &'static str {
            "radroots.social.deletion_request.v1"
        }
    }

    #[non_exhaustive]
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum RadrootsNip09DeletionProjectionError {
        UnsupportedKind {
            actual: u32,
        },
        ContentTooLarge {
            max: usize,
            actual: usize,
        },
        TagCountExceeded {
            max: usize,
            actual: usize,
        },
        TagElementCountExceeded {
            max: usize,
            actual: usize,
        },
        TagElementTooLarge {
            max: usize,
            actual: usize,
            tag_index: usize,
            element_index: usize,
        },
        TagBytesExceeded {
            max: usize,
            actual: usize,
        },
        EventWireTooLarge {
            max: usize,
            actual: usize,
        },
        EventTargetShape {
            tag_index: usize,
        },
        EventTargetInvalid {
            tag_index: usize,
            error: ParseError,
        },
        AddressTargetShape {
            tag_index: usize,
        },
        AddressTargetInvalid {
            tag_index: usize,
            error: Nip01CoordinateParseError,
        },
        TargetMissing,
    }

    impl RadrootsNip09DeletionProjectionError {
        pub const fn code(&self) -> &'static str {
            match self {
                Self::UnsupportedKind { .. } => "unsupported_kind",
                Self::ContentTooLarge { .. } => "deletion_content_too_large",
                Self::TagCountExceeded { .. } => "deletion_tag_count_exceeded",
                Self::TagElementCountExceeded { .. } => "deletion_tag_element_count_exceeded",
                Self::TagElementTooLarge { .. } => "deletion_tag_element_too_large",
                Self::TagBytesExceeded { .. } => "deletion_tag_bytes_exceeded",
                Self::EventWireTooLarge { .. } => "deletion_event_wire_too_large",
                Self::EventTargetShape { .. } => "deletion_event_target_shape",
                Self::EventTargetInvalid { .. } => "deletion_event_target_invalid",
                Self::AddressTargetShape { .. } => "deletion_address_target_shape",
                Self::AddressTargetInvalid { .. } => "deletion_address_target_invalid",
                Self::TargetMissing => "deletion_target_missing",
            }
        }
    }

    impl fmt::Display for RadrootsNip09DeletionProjectionError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::UnsupportedKind { actual } => {
                    write!(formatter, "NIP-09 deletion kind must be 5, got {actual}")
                }
                Self::ContentTooLarge { max, actual } => write!(
                    formatter,
                    "NIP-09 deletion content is {actual} bytes; max is {max}"
                ),
                Self::TagCountExceeded { max, actual } => {
                    write!(formatter, "NIP-09 deletion has {actual} tags; max is {max}")
                }
                Self::TagElementCountExceeded { max, actual } => write!(
                    formatter,
                    "NIP-09 deletion has {actual} total tag elements; max is {max}"
                ),
                Self::TagElementTooLarge {
                    max,
                    actual,
                    tag_index,
                    element_index,
                } => write!(
                    formatter,
                    "NIP-09 deletion tag {tag_index} element {element_index} is {actual} bytes; max is {max}"
                ),
                Self::TagBytesExceeded { max, actual } => write!(
                    formatter,
                    "NIP-09 deletion tag bytes are {actual}; max is {max}"
                ),
                Self::EventWireTooLarge { max, actual } => write!(
                    formatter,
                    "NIP-09 deletion compact signed event is {actual} bytes; max is {max}"
                ),
                Self::EventTargetShape { tag_index } => write!(
                    formatter,
                    "NIP-09 deletion event target tag {tag_index} has an invalid shape"
                ),
                Self::EventTargetInvalid { tag_index, error } => write!(
                    formatter,
                    "NIP-09 deletion event target tag {tag_index} is invalid: {error}"
                ),
                Self::AddressTargetShape { tag_index } => write!(
                    formatter,
                    "NIP-09 deletion address target tag {tag_index} has an invalid shape"
                ),
                Self::AddressTargetInvalid { tag_index, error } => write!(
                    formatter,
                    "NIP-09 deletion address target tag {tag_index} is invalid: {error}"
                ),
                Self::TargetMissing => {
                    formatter.write_str("NIP-09 deletion requires a valid event or address target")
                }
            }
        }
    }

    #[cfg(feature = "std")]
    impl std::error::Error for RadrootsNip09DeletionProjectionError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                Self::EventTargetInvalid { error, .. } => Some(error),
                Self::AddressTargetInvalid { error, .. } => Some(error),
                _ => None,
            }
        }
    }

    /// Projects a signature-and-id verified kind-5 NIP-09 deletion request.
    ///
    /// This boundary validates and canonicalizes request metadata only. It performs
    /// no target lookup, same-author authorization, suppression, store mutation,
    /// address cutoff, replacement, or deletion-request immunity evaluation.
    pub fn project_verified_nip09_deletion_request_event(
        verified_event: &RadrootsSignatureVerifiedEvent,
    ) -> Result<RadrootsInboundNip09DeletionProjection, RadrootsNip09DeletionProjectionError> {
        project_verified_nip09_deletion_request_event_v1(verified_event)
    }

    /// Projects a verified NIP-09 request with reconciliation-v1 semantics.
    pub fn project_verified_nip09_deletion_request_event_v1(
        verified_event: &RadrootsSignatureVerifiedEvent,
    ) -> Result<RadrootsInboundNip09DeletionProjection, RadrootsNip09DeletionProjectionError> {
        let event = verified_event.event();
        project_nip09_deletion_request_parts(
            event.kind_u32(),
            &event.tags_as_vec(),
            event.content(),
            event.created_at_u64(),
        )
    }

    pub(crate) fn project_nip09_deletion_request_parts(
        kind: u32,
        tags: &[Vec<String>],
        content: &str,
        created_at: u64,
    ) -> Result<RadrootsInboundNip09DeletionProjection, RadrootsNip09DeletionProjectionError> {
        if kind != KIND_DELETION_REQUEST {
            return Err(RadrootsNip09DeletionProjectionError::UnsupportedKind { actual: kind });
        }
        if content.len() > RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES {
            return Err(RadrootsNip09DeletionProjectionError::ContentTooLarge {
                max: RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES,
                actual: content.len(),
            });
        }
        validate_tag_and_wire_budgets(tags, content, decimal_digits(created_at))?;

        let mut event_targets = BTreeMap::new();
        let mut address_targets = BTreeMap::new();
        for (tag_index, tag) in tags.iter().enumerate() {
            match tag.first().map(String::as_str) {
                Some("e") => {
                    let Some(value) = tag.get(1) else {
                        return Err(RadrootsNip09DeletionProjectionError::EventTargetShape {
                            tag_index,
                        });
                    };
                    let event_id = EventId::parse(value).map_err(|error| {
                        RadrootsNip09DeletionProjectionError::EventTargetInvalid {
                            tag_index,
                            error,
                        }
                    })?;
                    event_targets.entry(event_id).or_insert_with(|| {
                        RadrootsInboundNip09DeletionEventTarget {
                            tag_index,
                            event_id,
                            raw_tag: tag.clone(),
                        }
                    });
                }
                Some("a") => {
                    let Some(value) = tag.get(1) else {
                        return Err(RadrootsNip09DeletionProjectionError::AddressTargetShape {
                            tag_index,
                        });
                    };
                    let coordinate = Nip01Coordinate::parse(value).map_err(|error| {
                        RadrootsNip09DeletionProjectionError::AddressTargetInvalid {
                            tag_index,
                            error,
                        }
                    })?;
                    if !address_targets.contains_key(&coordinate) {
                        address_targets.insert(
                            coordinate.clone(),
                            RadrootsInboundNip09DeletionAddressTarget {
                                tag_index,
                                coordinate,
                                raw_tag: tag.clone(),
                            },
                        );
                    }
                }
                _ => {}
            }
        }
        if event_targets.is_empty() && address_targets.is_empty() {
            return Err(RadrootsNip09DeletionProjectionError::TargetMissing);
        }

        let has_event_targets = !event_targets.is_empty();
        let address_kinds = address_targets
            .keys()
            .map(Nip01Coordinate::kind)
            .collect::<BTreeSet<_>>();
        let mut kind_advisories = BTreeMap::new();
        let mut diagnostics = Vec::new();
        for (tag_index, tag) in tags.iter().enumerate() {
            if !tag.first().is_some_and(|name| name == "k") {
                continue;
            }
            let Some(value) = tag.get(1) else {
                diagnostics.push(RadrootsNip09DeletionDiagnostic::KindAdvisoryShapeIgnored {
                    tag_index,
                    raw_tag: tag.clone(),
                });
                continue;
            };
            let Ok(kind) = value.parse::<u32>() else {
                diagnostics.push(
                    RadrootsNip09DeletionDiagnostic::KindAdvisoryInvalidIgnored {
                        tag_index,
                        raw_tag: tag.clone(),
                    },
                );
                continue;
            };
            if kind > RADROOTS_NIP09_DELETION_TARGET_KIND_MAX || kind.to_string() != *value {
                diagnostics.push(
                    RadrootsNip09DeletionDiagnostic::KindAdvisoryInvalidIgnored {
                        tag_index,
                        raw_tag: tag.clone(),
                    },
                );
                continue;
            }
            if kind_advisories.contains_key(&kind) {
                diagnostics.push(
                    RadrootsNip09DeletionDiagnostic::KindAdvisoryDuplicateIgnored {
                        tag_index,
                        raw_tag: tag.clone(),
                    },
                );
                continue;
            }
            kind_advisories.insert(
                kind,
                RadrootsInboundNip09DeletionKindAdvisory {
                    tag_index,
                    kind,
                    raw_tag: tag.clone(),
                },
            );
        }

        if !has_event_targets {
            for (kind, advisory) in &kind_advisories {
                if !address_kinds.contains(kind) {
                    diagnostics.push(
                        RadrootsNip09DeletionDiagnostic::KindAdvisoryConflictIgnored {
                            tag_index: advisory.tag_index,
                            raw_tag: advisory.raw_tag.clone(),
                        },
                    );
                }
            }
        }
        diagnostics.sort_by_key(RadrootsNip09DeletionDiagnostic::tag_index);

        Ok(RadrootsInboundNip09DeletionProjection {
            event_targets: event_targets.into_values().collect(),
            address_targets: address_targets.into_values().collect(),
            kind_advisories: kind_advisories.into_values().collect(),
            diagnostics,
            raw_tags: tags.to_vec(),
        })
    }

    fn validate_tag_and_wire_budgets(
        tags: &[Vec<String>],
        content: &str,
        created_at_digits: usize,
    ) -> Result<(), RadrootsNip09DeletionProjectionError> {
        if tags.len() > RADROOTS_NIP09_DELETION_TAG_MAX_COUNT {
            return Err(RadrootsNip09DeletionProjectionError::TagCountExceeded {
                max: RADROOTS_NIP09_DELETION_TAG_MAX_COUNT,
                actual: tags.len(),
            });
        }
        let tag_element_count = tags
            .iter()
            .fold(0usize, |total, tag| total.saturating_add(tag.len()));
        if tag_element_count > RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT {
            return Err(
                RadrootsNip09DeletionProjectionError::TagElementCountExceeded {
                    max: RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT,
                    actual: tag_element_count,
                },
            );
        }

        let mut tag_bytes = 0usize;
        let mut tags_json_bytes = 2usize;
        for (tag_index, tag) in tags.iter().enumerate() {
            if tag_index > 0 {
                tags_json_bytes = tags_json_bytes.saturating_add(1);
            }
            tags_json_bytes = tags_json_bytes.saturating_add(2);
            for (element_index, element) in tag.iter().enumerate() {
                if element.len() > RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES {
                    return Err(RadrootsNip09DeletionProjectionError::TagElementTooLarge {
                        max: RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES,
                        actual: element.len(),
                        tag_index,
                        element_index,
                    });
                }
                if element_index > 0 {
                    tags_json_bytes = tags_json_bytes.saturating_add(1);
                }
                tags_json_bytes =
                    tags_json_bytes.saturating_add(canonical_json_string_bytes(element));
                tag_bytes = tag_bytes.saturating_add(element.len());
            }
        }
        if tag_bytes > RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES {
            return Err(RadrootsNip09DeletionProjectionError::TagBytesExceeded {
                max: RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES,
                actual: tag_bytes,
            });
        }

        let actual = RADROOTS_NIP09_DELETION_SIGNED_EVENT_FIXED_BYTES
            .saturating_add(created_at_digits)
            .saturating_add(tags_json_bytes)
            .saturating_add(canonical_json_string_bytes(content));
        if actual > RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES {
            return Err(RadrootsNip09DeletionProjectionError::EventWireTooLarge {
                max: RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES,
                actual,
            });
        }
        Ok(())
    }

    fn canonical_json_string_bytes(value: &str) -> usize {
        value.chars().fold(2usize, |total, character| {
            total.saturating_add(match character {
                '"' | '\\' | '\u{0008}' | '\t' | '\n' | '\u{000c}' | '\r' => 2,
                '\u{0000}'..='\u{001f}' => 6,
                _ => character.len_utf8(),
            })
        })
    }

    const fn decimal_digits(mut value: u64) -> usize {
        let mut digits = 1usize;
        while value >= 10 {
            value /= 10;
            digits += 1;
        }
        digits
    }

    #[cfg(test)]
    mod tests;
}

#![forbid(unsafe_code)]

use crate::RadrootsOutbox;
use radroots_event::draft::{RadrootsSignedEvent, RadrootsVerifiedSignedEvent};
use radroots_event::wire::RadrootsNip01EventWire;
use radroots_event_codec::wire::publication::allowlist::allow_phase1_publication_canonical_json;
use radroots_event_codec::wire::publication::{
    RADROOTS_PHASE1_PUBLICATION_ARTIFACT_MAX_BYTES,
    RADROOTS_PHASE1_PUBLICATION_ARTIFACT_SCHEMA_VERSION,
    RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_MAX_BYTES,
    RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_SCHEMA_VERSION,
    RADROOTS_PHASE1_PUBLICATION_SIGNED_EVENT_MAX_BYTES,
    RadrootsPhase1MediaReadyPublicationArtifact, validate_phase1_publication_media_readiness,
};
use sha2::{Digest, Sha256};
use sqlx::{Row, Sqlite, Transaction};
use std::collections::BTreeSet;
use std::fmt;
use url::Url;

pub const RADROOTS_PHASE1_PUBLICATION_TARGET_MAX_COUNT: usize = 16;
pub const RADROOTS_PHASE1_PUBLICATION_TARGET_URI_MAX_BYTES: usize = 2_048;
pub const RADROOTS_PHASE1_PUBLICATION_DIAGNOSTIC_MAX_BYTES: usize = 4_096;
pub const RADROOTS_PHASE1_PUBLICATION_CLAIM_LEASE_MAX_MILLIS: i64 = 300_000;
pub const RADROOTS_PHASE1_PUBLICATION_ERROR_CODES: &[&str] = &[
    "phase1_publication_artifact_invalid",
    "phase1_publication_claim_invalid",
    "phase1_publication_diagnostic_too_large",
    "phase1_publication_entropy_unavailable",
    "phase1_publication_idempotency_conflict",
    "phase1_publication_integer_range",
    "phase1_publication_lease_invalid",
    "phase1_publication_not_found",
    "phase1_publication_readiness_invalid",
    "phase1_publication_required_target_count",
    "phase1_publication_revision_conflict",
    "phase1_publication_signed_event_invalid",
    "phase1_publication_signed_event_mismatch",
    "phase1_publication_sqlite",
    "phase1_publication_state_conflict",
    "phase1_publication_stored_authority_invalid",
    "phase1_publication_stored_digest_invalid",
    "phase1_publication_stored_state_invalid",
    "phase1_publication_stored_value_too_large",
    "phase1_publication_target_count",
    "phase1_publication_target_duplicate",
    "phase1_publication_target_not_found",
    "phase1_publication_target_uri_invalid",
    "phase1_publication_target_uri_too_large",
    "phase1_publication_time_invalid",
];

const OPERATION_DOMAIN: &[u8] = b"radroots.phase1.publication-operation.v1\0";
const TARGET_POLICY_DOMAIN: &[u8] = b"radroots.phase1.target-policy.v1\0";
const ENDPOINT_DOMAIN: &[u8] = b"radroots.phase1.endpoint.v1\0";
const DISPATCH_DOMAIN: &[u8] = b"radroots.phase1.relay-dispatch.v1\0";
const RECEIPT_DOMAIN: &[u8] = b"radroots.phase1.target-receipt.v1\0";
const REPAIR_DOMAIN: &[u8] = b"radroots.phase1.observation-repair.v1\0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsPhase1PublicationEventState {
    Ready,
    ClaimedForSigning,
    SignedReady,
    Dispatching,
    Published,
    FailedRetryable,
    FailedTerminal,
    Quarantined,
    Cancelled,
}

impl RadrootsPhase1PublicationEventState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::ClaimedForSigning => "claimed-for-signing",
            Self::SignedReady => "signed-ready",
            Self::Dispatching => "dispatching",
            Self::Published => "published",
            Self::FailedRetryable => "failed-retryable",
            Self::FailedTerminal => "failed-terminal",
            Self::Quarantined => "quarantined",
            Self::Cancelled => "cancelled",
        }
    }

    fn parse(value: &str) -> Result<Self, RadrootsPhase1PublicationError> {
        match value {
            "ready" => Ok(Self::Ready),
            "claimed-for-signing" => Ok(Self::ClaimedForSigning),
            "signed-ready" => Ok(Self::SignedReady),
            "dispatching" => Ok(Self::Dispatching),
            "published" => Ok(Self::Published),
            "failed-retryable" => Ok(Self::FailedRetryable),
            "failed-terminal" => Ok(Self::FailedTerminal),
            "quarantined" => Ok(Self::Quarantined),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(RadrootsPhase1PublicationError::StoredStateInvalid),
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Published | Self::FailedTerminal | Self::Quarantined | Self::Cancelled
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsPhase1PublicationTargetState {
    Pending,
    InFlight,
    AcceptedObservationPending,
    AcceptedObserved,
    FailedRetryable,
    FailedTerminal,
    Uncertain,
    Cancelled,
}

impl RadrootsPhase1PublicationTargetState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InFlight => "in-flight",
            Self::AcceptedObservationPending => "accepted-observation-pending",
            Self::AcceptedObserved => "accepted-observed",
            Self::FailedRetryable => "failed-retryable",
            Self::FailedTerminal => "failed-terminal",
            Self::Uncertain => "uncertain",
            Self::Cancelled => "cancelled",
        }
    }

    fn parse(value: &str) -> Result<Self, RadrootsPhase1PublicationError> {
        match value {
            "pending" => Ok(Self::Pending),
            "in-flight" => Ok(Self::InFlight),
            "accepted-observation-pending" => Ok(Self::AcceptedObservationPending),
            "accepted-observed" => Ok(Self::AcceptedObserved),
            "failed-retryable" => Ok(Self::FailedRetryable),
            "failed-terminal" => Ok(Self::FailedTerminal),
            "uncertain" => Ok(Self::Uncertain),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(RadrootsPhase1PublicationError::StoredStateInvalid),
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::AcceptedObserved | Self::FailedTerminal | Self::Cancelled
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsPhase1PublicationTransitionScope {
    Event,
    Target,
}

impl RadrootsPhase1PublicationTransitionScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::Target => "target",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsPhase1PublicationTransitionRetryClass {
    None,
    Retryable,
    Repair,
    Terminal,
}

impl RadrootsPhase1PublicationTransitionRetryClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Retryable => "retryable",
            Self::Repair => "repair",
            Self::Terminal => "terminal",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsPhase1PublicationTransition {
    pub id: &'static str,
    pub scope: RadrootsPhase1PublicationTransitionScope,
    pub from: &'static str,
    pub to: &'static str,
    pub revision_cas: bool,
    pub lease_predicate: &'static str,
    pub durable_side_effect: &'static str,
    pub retry_class: RadrootsPhase1PublicationTransitionRetryClass,
    pub repair_edge: bool,
    pub terminal_destination: bool,
}

macro_rules! transition {
    ($id:literal, $scope:ident, $from:literal, $to:literal, $lease:literal, $effect:literal, $retry:ident, $repair:literal, $terminal:literal) => {
        RadrootsPhase1PublicationTransition {
            id: $id,
            scope: RadrootsPhase1PublicationTransitionScope::$scope,
            from: $from,
            to: $to,
            revision_cas: true,
            lease_predicate: $lease,
            durable_side_effect: $effect,
            retry_class: RadrootsPhase1PublicationTransitionRetryClass::$retry,
            repair_edge: $repair,
            terminal_destination: $terminal,
        }
    };
}

pub const RADROOTS_PHASE1_PUBLICATION_TRANSITIONS: &[RadrootsPhase1PublicationTransition] = &[
    transition!(
        "claim-ready",
        Event,
        "ready",
        "claimed-for-signing",
        "absent-or-expired",
        "replace-claim",
        None,
        false,
        false
    ),
    transition!(
        "reclaim-signing",
        Event,
        "claimed-for-signing",
        "claimed-for-signing",
        "expired",
        "replace-claim",
        Retryable,
        false,
        false
    ),
    transition!(
        "claim-sign-retry",
        Event,
        "failed-retryable",
        "claimed-for-signing",
        "absent-or-expired",
        "replace-claim",
        Retryable,
        false,
        false
    ),
    transition!(
        "renew-signing",
        Event,
        "claimed-for-signing",
        "claimed-for-signing",
        "matching-live-token",
        "extend-claim",
        None,
        false,
        false
    ),
    transition!(
        "release-signing",
        Event,
        "claimed-for-signing",
        "ready",
        "matching-live-token",
        "clear-claim",
        None,
        false,
        false
    ),
    transition!(
        "complete-signing",
        Event,
        "claimed-for-signing",
        "signed-ready",
        "matching-live-token",
        "persist-immutable-signed-bytes",
        None,
        false,
        false
    ),
    transition!(
        "retry-signing",
        Event,
        "claimed-for-signing",
        "failed-retryable",
        "matching-live-token",
        "persist-bounded-error",
        Retryable,
        false,
        false
    ),
    transition!(
        "fail-signing",
        Event,
        "claimed-for-signing",
        "failed-terminal",
        "matching-live-token",
        "persist-bounded-error",
        Terminal,
        false,
        true
    ),
    transition!(
        "quarantine-signing",
        Event,
        "claimed-for-signing",
        "quarantined",
        "matching-live-token",
        "persist-bounded-error",
        Terminal,
        false,
        true
    ),
    transition!(
        "cancel-signing",
        Event,
        "claimed-for-signing",
        "cancelled",
        "matching-live-token",
        "clear-claim",
        Terminal,
        false,
        true
    ),
    transition!(
        "begin-dispatch",
        Event,
        "signed-ready",
        "dispatching",
        "target-matching-live-token",
        "persist-dispatch-intent",
        None,
        false,
        false
    ),
    transition!(
        "continue-dispatch",
        Event,
        "dispatching",
        "dispatching",
        "target-matching-live-token",
        "persist-dispatch-intent",
        Retryable,
        false,
        false
    ),
    transition!(
        "dispatch-published",
        Event,
        "dispatching",
        "published",
        "target-matching-live-token",
        "persist-target-receipt",
        None,
        false,
        true
    ),
    transition!(
        "dispatch-waiting",
        Event,
        "dispatching",
        "signed-ready",
        "target-matching-live-token",
        "persist-target-result",
        Retryable,
        false,
        false
    ),
    transition!(
        "dispatch-exhausted",
        Event,
        "dispatching",
        "failed-terminal",
        "target-matching-live-token",
        "persist-target-result",
        Terminal,
        false,
        true
    ),
    transition!(
        "claim-target",
        Target,
        "pending",
        "in-flight",
        "absent-or-expired",
        "persist-dispatch-intent",
        None,
        false,
        false
    ),
    transition!(
        "retry-target",
        Target,
        "failed-retryable",
        "in-flight",
        "absent-or-expired",
        "reuse-dispatch-intent",
        Retryable,
        false,
        false
    ),
    transition!(
        "repair-uncertain-target",
        Target,
        "uncertain",
        "in-flight",
        "absent-or-expired",
        "reuse-dispatch-intent",
        Repair,
        true,
        false
    ),
    transition!(
        "target-accepted-pending",
        Target,
        "in-flight",
        "accepted-observation-pending",
        "matching-live-token",
        "persist-receipt-and-repair",
        Repair,
        true,
        false
    ),
    transition!(
        "target-accepted",
        Target,
        "in-flight",
        "accepted-observed",
        "matching-live-token",
        "persist-receipt",
        None,
        false,
        true
    ),
    transition!(
        "target-retryable",
        Target,
        "in-flight",
        "failed-retryable",
        "matching-live-token",
        "persist-bounded-error",
        Retryable,
        false,
        false
    ),
    transition!(
        "target-terminal",
        Target,
        "in-flight",
        "failed-terminal",
        "matching-live-token",
        "persist-bounded-error",
        Terminal,
        false,
        true
    ),
    transition!(
        "target-uncertain",
        Target,
        "in-flight",
        "uncertain",
        "matching-live-token",
        "persist-bounded-error",
        Repair,
        true,
        false
    ),
    transition!(
        "target-cancelled",
        Target,
        "in-flight",
        "cancelled",
        "matching-live-token",
        "clear-claim",
        Terminal,
        false,
        true
    ),
    transition!(
        "observation-repaired",
        Target,
        "accepted-observation-pending",
        "accepted-observed",
        "repair-revision-cas",
        "complete-repair-and-receipt",
        Repair,
        true,
        true
    ),
];

#[derive(Debug)]
#[non_exhaustive]
pub enum RadrootsPhase1PublicationError {
    Sqlite(sqlx::Error),
    ArtifactInvalid {
        source_code: &'static str,
    },
    ReadinessInvalid {
        source_code: &'static str,
    },
    TargetCount {
        max: usize,
        actual: usize,
    },
    RequiredTargetCount {
        target_count: usize,
        required: usize,
    },
    TargetUriTooLarge {
        max: usize,
        actual: usize,
    },
    TargetUriInvalid,
    DuplicateTarget,
    InvalidTime,
    InvalidLease {
        max_millis: i64,
        actual: i64,
    },
    EntropyUnavailable,
    PublicationNotFound {
        publication_id: i64,
    },
    TargetNotFound {
        target_id: i64,
    },
    IdempotencyConflict,
    RevisionConflict,
    ClaimInvalid,
    StateConflict,
    SignedEventMismatch,
    SignedEventInvalid,
    DiagnosticTooLarge {
        max: usize,
        actual: usize,
    },
    StoredValueTooLarge {
        field: &'static str,
        max: usize,
        actual: usize,
    },
    StoredDigestInvalid {
        field: &'static str,
    },
    StoredStateInvalid,
    StoredAuthorityInvalid,
    IntegerRange {
        field: &'static str,
    },
}

impl RadrootsPhase1PublicationError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Sqlite(_) => "phase1_publication_sqlite",
            Self::ArtifactInvalid { .. } => "phase1_publication_artifact_invalid",
            Self::ReadinessInvalid { .. } => "phase1_publication_readiness_invalid",
            Self::TargetCount { .. } => "phase1_publication_target_count",
            Self::RequiredTargetCount { .. } => "phase1_publication_required_target_count",
            Self::TargetUriTooLarge { .. } => "phase1_publication_target_uri_too_large",
            Self::TargetUriInvalid => "phase1_publication_target_uri_invalid",
            Self::DuplicateTarget => "phase1_publication_target_duplicate",
            Self::InvalidTime => "phase1_publication_time_invalid",
            Self::InvalidLease { .. } => "phase1_publication_lease_invalid",
            Self::EntropyUnavailable => "phase1_publication_entropy_unavailable",
            Self::PublicationNotFound { .. } => "phase1_publication_not_found",
            Self::TargetNotFound { .. } => "phase1_publication_target_not_found",
            Self::IdempotencyConflict => "phase1_publication_idempotency_conflict",
            Self::RevisionConflict => "phase1_publication_revision_conflict",
            Self::ClaimInvalid => "phase1_publication_claim_invalid",
            Self::StateConflict => "phase1_publication_state_conflict",
            Self::SignedEventMismatch => "phase1_publication_signed_event_mismatch",
            Self::SignedEventInvalid => "phase1_publication_signed_event_invalid",
            Self::DiagnosticTooLarge { .. } => "phase1_publication_diagnostic_too_large",
            Self::StoredValueTooLarge { .. } => "phase1_publication_stored_value_too_large",
            Self::StoredDigestInvalid { .. } => "phase1_publication_stored_digest_invalid",
            Self::StoredStateInvalid => "phase1_publication_stored_state_invalid",
            Self::StoredAuthorityInvalid => "phase1_publication_stored_authority_invalid",
            Self::IntegerRange { .. } => "phase1_publication_integer_range",
        }
    }

    pub fn public_diagnostic(&self) -> String {
        let diagnostic = self.to_string();
        if diagnostic.len() <= RADROOTS_PHASE1_PUBLICATION_DIAGNOSTIC_MAX_BYTES {
            return diagnostic;
        }
        let suffix = "…";
        let mut end = RADROOTS_PHASE1_PUBLICATION_DIAGNOSTIC_MAX_BYTES - suffix.len();
        while !diagnostic.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}{}", &diagnostic[..end], suffix)
    }
}

impl fmt::Display for RadrootsPhase1PublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetCount { max, actual } => {
                write!(
                    formatter,
                    "target count {actual} exceeds valid range 1..={max}"
                )
            }
            Self::RequiredTargetCount {
                target_count,
                required,
            } => write!(
                formatter,
                "required target count {required} is invalid for {target_count} targets"
            ),
            Self::TargetUriTooLarge { max, actual } => {
                write!(formatter, "target URI is {actual} bytes; maximum is {max}")
            }
            Self::InvalidLease { max_millis, actual } => write!(
                formatter,
                "claim lease {actual} ms is outside 1..={max_millis}"
            ),
            Self::PublicationNotFound { publication_id } => {
                write!(
                    formatter,
                    "Phase 1 publication {publication_id} was not found"
                )
            }
            Self::TargetNotFound { target_id } => {
                write!(
                    formatter,
                    "Phase 1 publication target {target_id} was not found"
                )
            }
            Self::DiagnosticTooLarge { max, actual } => {
                write!(formatter, "diagnostic is {actual} bytes; maximum is {max}")
            }
            Self::StoredValueTooLarge { field, max, actual } => write!(
                formatter,
                "stored {field} is {actual} bytes; maximum is {max}"
            ),
            Self::StoredDigestInvalid { field } => write!(formatter, "stored {field} is invalid"),
            Self::IntegerRange { field } => write!(formatter, "stored {field} is out of range"),
            Self::Sqlite(_)
            | Self::ArtifactInvalid { .. }
            | Self::ReadinessInvalid { .. }
            | Self::TargetUriInvalid
            | Self::DuplicateTarget
            | Self::InvalidTime
            | Self::EntropyUnavailable
            | Self::IdempotencyConflict
            | Self::RevisionConflict
            | Self::ClaimInvalid
            | Self::StateConflict
            | Self::SignedEventMismatch
            | Self::SignedEventInvalid
            | Self::StoredStateInvalid
            | Self::StoredAuthorityInvalid => formatter.write_str(self.code()),
        }
    }
}

impl std::error::Error for RadrootsPhase1PublicationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(error) => Some(error),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for RadrootsPhase1PublicationError {
    fn from(value: sqlx::Error) -> Self {
        Self::Sqlite(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsPhase1PublicationTargetPolicy {
    targets: Vec<String>,
    required_target_count: usize,
    digest: [u8; 32],
}

impl RadrootsPhase1PublicationTargetPolicy {
    pub fn new<I, S>(
        targets: I,
        required_target_count: usize,
    ) -> Result<Self, RadrootsPhase1PublicationError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut canonical = Vec::new();
        let mut targets = targets.into_iter();
        let (minimum, _) = targets.size_hint();
        if minimum > RADROOTS_PHASE1_PUBLICATION_TARGET_MAX_COUNT {
            return Err(RadrootsPhase1PublicationError::TargetCount {
                max: RADROOTS_PHASE1_PUBLICATION_TARGET_MAX_COUNT,
                actual: minimum,
            });
        }
        canonical.try_reserve_exact(minimum).map_err(|_| {
            RadrootsPhase1PublicationError::TargetCount {
                max: RADROOTS_PHASE1_PUBLICATION_TARGET_MAX_COUNT,
                actual: minimum,
            }
        })?;
        for target in &mut targets {
            if canonical.len() == RADROOTS_PHASE1_PUBLICATION_TARGET_MAX_COUNT {
                return Err(RadrootsPhase1PublicationError::TargetCount {
                    max: RADROOTS_PHASE1_PUBLICATION_TARGET_MAX_COUNT,
                    actual: canonical.len() + 1,
                });
            }
            canonical.push(canonical_target_uri(target.as_ref())?);
        }
        canonical.sort();
        if canonical.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(RadrootsPhase1PublicationError::DuplicateTarget);
        }
        if canonical.is_empty()
            || required_target_count == 0
            || required_target_count > canonical.len()
        {
            return Err(RadrootsPhase1PublicationError::RequiredTargetCount {
                target_count: canonical.len(),
                required: required_target_count,
            });
        }
        let digest = target_policy_digest(&canonical, required_target_count)?;
        Ok(Self {
            targets: canonical,
            required_target_count,
            digest,
        })
    }

    pub fn targets(&self) -> &[String] {
        &self.targets
    }

    pub const fn required_target_count(&self) -> usize {
        self.required_target_count
    }

    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsPhase1PublicationEnqueueStatus {
    Inserted,
    Existing,
}

#[derive(Clone, Debug)]
pub struct RadrootsPhase1PublicationEnqueueReceipt {
    status: RadrootsPhase1PublicationEnqueueStatus,
    record: RadrootsPhase1PublicationRecord,
}

impl RadrootsPhase1PublicationEnqueueReceipt {
    pub const fn status(&self) -> RadrootsPhase1PublicationEnqueueStatus {
        self.status
    }

    pub const fn record(&self) -> &RadrootsPhase1PublicationRecord {
        &self.record
    }

    pub fn into_record(self) -> RadrootsPhase1PublicationRecord {
        self.record
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsPhase1PublicationTarget {
    target_id: i64,
    ordinal: usize,
    endpoint_uri: String,
    endpoint_fingerprint: [u8; 32],
    dispatch_digest: [u8; 32],
    state: RadrootsPhase1PublicationTargetState,
    revision: u64,
    next_attempt_after_ms: i64,
}

impl RadrootsPhase1PublicationTarget {
    pub const fn target_id(&self) -> i64 {
        self.target_id
    }

    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub fn endpoint_uri(&self) -> &str {
        &self.endpoint_uri
    }

    pub const fn endpoint_fingerprint(&self) -> &[u8; 32] {
        &self.endpoint_fingerprint
    }

    pub const fn dispatch_digest(&self) -> &[u8; 32] {
        &self.dispatch_digest
    }

    pub const fn state(&self) -> RadrootsPhase1PublicationTargetState {
        self.state
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub const fn next_attempt_after_ms(&self) -> i64 {
        self.next_attempt_after_ms
    }
}

#[derive(Clone, Debug)]
pub struct RadrootsPhase1PublicationRecord {
    publication_id: i64,
    operation_digest: [u8; 32],
    ready_artifact: RadrootsPhase1MediaReadyPublicationArtifact,
    target_policy: RadrootsPhase1PublicationTargetPolicy,
    targets: Vec<RadrootsPhase1PublicationTarget>,
    state: RadrootsPhase1PublicationEventState,
    revision: u64,
    next_attempt_after_ms: i64,
    signed_event: Option<RadrootsVerifiedSignedEvent>,
}

impl RadrootsPhase1PublicationRecord {
    pub const fn publication_id(&self) -> i64 {
        self.publication_id
    }

    pub const fn operation_digest(&self) -> &[u8; 32] {
        &self.operation_digest
    }

    pub const fn ready_artifact(&self) -> &RadrootsPhase1MediaReadyPublicationArtifact {
        &self.ready_artifact
    }

    pub const fn target_policy(&self) -> &RadrootsPhase1PublicationTargetPolicy {
        &self.target_policy
    }

    pub fn targets(&self) -> &[RadrootsPhase1PublicationTarget] {
        &self.targets
    }

    pub const fn state(&self) -> RadrootsPhase1PublicationEventState {
        self.state
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub const fn next_attempt_after_ms(&self) -> i64 {
        self.next_attempt_after_ms
    }

    pub const fn signed_event(&self) -> Option<&RadrootsVerifiedSignedEvent> {
        self.signed_event.as_ref()
    }
}

#[derive(Clone, Debug)]
pub struct RadrootsPhase1PublicationClaim {
    publication_id: i64,
    revision: u64,
    token: [u8; 32],
    expires_at_ms: i64,
}

impl RadrootsPhase1PublicationClaim {
    pub const fn publication_id(&self) -> i64 {
        self.publication_id
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }
}

#[derive(Clone, Debug)]
pub struct RadrootsPhase1PublicationTargetClaim {
    publication_id: i64,
    publication_revision: u64,
    target_id: i64,
    target_revision: u64,
    token: [u8; 32],
    expires_at_ms: i64,
}

impl RadrootsPhase1PublicationTargetClaim {
    pub const fn publication_id(&self) -> i64 {
        self.publication_id
    }

    pub const fn publication_revision(&self) -> u64 {
        self.publication_revision
    }

    pub const fn target_id(&self) -> i64 {
        self.target_id
    }

    pub const fn target_revision(&self) -> u64 {
        self.target_revision
    }

    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }
}

struct PreparedPublication {
    artifact_json: Vec<u8>,
    artifact_digest: [u8; 32],
    readiness_json: Vec<u8>,
    readiness_digest: [u8; 32],
    semantic_role: &'static str,
    expected_author: [u8; 32],
    expected_event_id: [u8; 32],
    operation_digest: [u8; 32],
}

impl RadrootsOutbox {
    pub async fn enqueue_phase1_publication(
        &self,
        ready: &RadrootsPhase1MediaReadyPublicationArtifact,
        target_policy: &RadrootsPhase1PublicationTargetPolicy,
        now_ms: i64,
    ) -> Result<RadrootsPhase1PublicationEnqueueReceipt, RadrootsPhase1PublicationError> {
        validate_time(now_ms)?;
        let prepared = prepare_publication(ready, target_policy)?;
        let mut transaction = self.pool.begin().await?;
        if let Some(row) = sqlx::query(
            "SELECT publication_id FROM outbox_phase1_publication WHERE operation_digest = ?",
        )
        .bind(prepared.operation_digest.as_slice())
        .fetch_optional(&mut *transaction)
        .await?
        {
            let publication_id = row.try_get("publication_id")?;
            transaction.commit().await?;
            let record = self.load_phase1_publication(publication_id).await?;
            return Ok(RadrootsPhase1PublicationEnqueueReceipt {
                status: RadrootsPhase1PublicationEnqueueStatus::Existing,
                record,
            });
        }
        if sqlx::query(
            "SELECT publication_id FROM outbox_phase1_publication WHERE artifact_digest = ? AND expected_author = ? LIMIT 1",
        )
        .bind(prepared.artifact_digest.as_slice())
        .bind(prepared.expected_author.as_slice())
        .fetch_optional(&mut *transaction)
        .await?
        .is_some()
        {
            return Err(RadrootsPhase1PublicationError::IdempotencyConflict);
        }
        let result = sqlx::query(
            "INSERT INTO outbox_phase1_publication(operation_digest, artifact_schema_version, artifact_json, artifact_digest, readiness_schema_version, readiness_json, readiness_digest, semantic_role, expected_author, expected_event_id, target_policy_digest, target_count, required_target_count, state, state_revision, next_attempt_after_ms, created_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'ready', 0, ?, ?, ?)",
        )
        .bind(prepared.operation_digest.as_slice())
        .bind(i64::from(RADROOTS_PHASE1_PUBLICATION_ARTIFACT_SCHEMA_VERSION))
        .bind(prepared.artifact_json)
        .bind(prepared.artifact_digest.as_slice())
        .bind(i64::from(
            RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_SCHEMA_VERSION,
        ))
        .bind(prepared.readiness_json)
        .bind(prepared.readiness_digest.as_slice())
        .bind(prepared.semantic_role)
        .bind(prepared.expected_author.as_slice())
        .bind(prepared.expected_event_id.as_slice())
        .bind(target_policy.digest.as_slice())
        .bind(i64::try_from(target_policy.targets.len()).map_err(|_| {
            RadrootsPhase1PublicationError::IntegerRange {
                field: "target_count",
            }
        })?)
        .bind(i64::try_from(target_policy.required_target_count).map_err(|_| {
            RadrootsPhase1PublicationError::IntegerRange {
                field: "required_target_count",
            }
        })?)
        .bind(now_ms)
        .bind(now_ms)
        .bind(now_ms)
        .execute(&mut *transaction)
        .await?;
        let publication_id = result.last_insert_rowid();
        for (ordinal, endpoint_uri) in target_policy.targets.iter().enumerate() {
            let endpoint_fingerprint = endpoint_fingerprint(endpoint_uri);
            let dispatch_digest = dispatch_digest(
                &prepared.expected_event_id,
                &target_policy.digest,
                &endpoint_fingerprint,
            );
            sqlx::query(
                "INSERT INTO outbox_phase1_delivery_target(publication_id, target_ordinal, endpoint_uri, endpoint_fingerprint, dispatch_digest, state, state_revision, next_attempt_after_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, 'pending', 0, ?, ?)",
            )
            .bind(publication_id)
            .bind(i64::try_from(ordinal).map_err(|_| {
                RadrootsPhase1PublicationError::IntegerRange {
                    field: "target_ordinal",
                }
            })?)
            .bind(endpoint_uri)
            .bind(endpoint_fingerprint.as_slice())
            .bind(dispatch_digest.as_slice())
            .bind(now_ms)
            .bind(now_ms)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        let record = self.load_phase1_publication(publication_id).await?;
        Ok(RadrootsPhase1PublicationEnqueueReceipt {
            status: RadrootsPhase1PublicationEnqueueStatus::Inserted,
            record,
        })
    }

    pub async fn load_phase1_publication(
        &self,
        publication_id: i64,
    ) -> Result<RadrootsPhase1PublicationRecord, RadrootsPhase1PublicationError> {
        let row = sqlx::query(
            "SELECT publication_id, operation_digest,
                length(artifact_json) AS artifact_bytes,
                CASE WHEN length(artifact_json) <= ? THEN artifact_json END AS bounded_artifact_json,
                artifact_digest,
                length(readiness_json) AS readiness_bytes,
                CASE WHEN length(readiness_json) <= ? THEN readiness_json END AS bounded_readiness_json,
                readiness_digest, semantic_role, expected_author, expected_event_id,
                target_policy_digest, target_count, required_target_count, state, state_revision,
                next_attempt_after_ms,
                length(signed_event_json) AS signed_event_bytes,
                CASE WHEN signed_event_json IS NULL OR length(signed_event_json) <= ? THEN signed_event_json END AS bounded_signed_event_json,
                signed_event_digest, signed_event_id
             FROM outbox_phase1_publication WHERE publication_id = ?",
        )
        .bind(i64::try_from(RADROOTS_PHASE1_PUBLICATION_ARTIFACT_MAX_BYTES).unwrap_or(i64::MAX))
        .bind(
            i64::try_from(RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_MAX_BYTES)
                .unwrap_or(i64::MAX),
        )
        .bind(i64::try_from(RADROOTS_PHASE1_PUBLICATION_SIGNED_EVENT_MAX_BYTES).unwrap_or(i64::MAX))
        .bind(publication_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(RadrootsPhase1PublicationError::PublicationNotFound { publication_id })?;
        let artifact_json = bounded_blob(
            &row,
            "bounded_artifact_json",
            "artifact_bytes",
            "artifact_json",
            RADROOTS_PHASE1_PUBLICATION_ARTIFACT_MAX_BYTES,
        )?
        .ok_or(RadrootsPhase1PublicationError::StoredAuthorityInvalid)?;
        let readiness_json = bounded_blob(
            &row,
            "bounded_readiness_json",
            "readiness_bytes",
            "readiness_json",
            RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_MAX_BYTES,
        )?
        .ok_or(RadrootsPhase1PublicationError::StoredAuthorityInvalid)?;
        let allowlisted =
            allow_phase1_publication_canonical_json(&artifact_json).map_err(|error| {
                RadrootsPhase1PublicationError::ArtifactInvalid {
                    source_code: error.code(),
                }
            })?;
        let ready_artifact = RadrootsPhase1MediaReadyPublicationArtifact::from_canonical_json(
            allowlisted,
            &readiness_json,
        )
        .map_err(|error| RadrootsPhase1PublicationError::ReadinessInvalid {
            source_code: error.code(),
        })?;
        validate_phase1_publication_media_readiness(&ready_artifact).map_err(|error| {
            RadrootsPhase1PublicationError::ReadinessInvalid {
                source_code: error.code(),
            }
        })?;

        let targets = load_targets(&self.pool, publication_id).await?;
        let required_target_count = usize_from_i64(
            row.try_get("required_target_count")?,
            "required_target_count",
        )?;
        let target_policy = RadrootsPhase1PublicationTargetPolicy::new(
            targets.iter().map(|target| target.endpoint_uri.as_str()),
            required_target_count,
        )?;
        if targets.len() != usize_from_i64(row.try_get("target_count")?, "target_count")? {
            return Err(RadrootsPhase1PublicationError::StoredAuthorityInvalid);
        }
        let operation_digest = blob32(&row, "operation_digest")?;
        let artifact_digest = blob32(&row, "artifact_digest")?;
        let readiness_digest = blob32(&row, "readiness_digest")?;
        let expected_author = blob32(&row, "expected_author")?;
        let expected_event_id = blob32(&row, "expected_event_id")?;
        let stored_policy_digest = blob32(&row, "target_policy_digest")?;
        let artifact = ready_artifact.artifact();
        if artifact_digest != *artifact.artifact_digest().as_bytes()
            || readiness_digest != *ready_artifact.binding_digest().as_bytes()
            || expected_author
                != decode_hex32(artifact.expected_author().as_str(), "expected_author")?
            || expected_event_id
                != decode_hex32(artifact.expected_event_id().as_str(), "expected_event_id")?
            || stored_policy_digest != target_policy.digest
            || row.try_get::<String, _>("semantic_role")? != artifact.semantic_variant().as_str()
        {
            return Err(RadrootsPhase1PublicationError::StoredAuthorityInvalid);
        }
        let expected_operation = operation_digest_for(
            &artifact_digest,
            &readiness_digest,
            &expected_author,
            &stored_policy_digest,
        );
        if operation_digest != expected_operation {
            return Err(RadrootsPhase1PublicationError::StoredAuthorityInvalid);
        }
        validate_target_identities(&targets, &expected_event_id, &stored_policy_digest)?;

        let signed_json = bounded_blob(
            &row,
            "bounded_signed_event_json",
            "signed_event_bytes",
            "signed_event_json",
            RADROOTS_PHASE1_PUBLICATION_SIGNED_EVENT_MAX_BYTES,
        )?;
        let signed_digest: Option<Vec<u8>> = row.try_get("signed_event_digest")?;
        let signed_event_id: Option<Vec<u8>> = row.try_get("signed_event_id")?;
        let signed_event =
            reload_signed_event(signed_json, signed_digest, signed_event_id, &ready_artifact)?;
        let state =
            RadrootsPhase1PublicationEventState::parse(&row.try_get::<String, _>("state")?)?;
        let signed_required = matches!(
            state,
            RadrootsPhase1PublicationEventState::SignedReady
                | RadrootsPhase1PublicationEventState::Dispatching
                | RadrootsPhase1PublicationEventState::Published
        );
        let signed_forbidden = matches!(
            state,
            RadrootsPhase1PublicationEventState::Ready
                | RadrootsPhase1PublicationEventState::ClaimedForSigning
                | RadrootsPhase1PublicationEventState::FailedRetryable
        );
        if (signed_required && signed_event.is_none())
            || (signed_forbidden && signed_event.is_some())
        {
            return Err(RadrootsPhase1PublicationError::StoredAuthorityInvalid);
        }
        Ok(RadrootsPhase1PublicationRecord {
            publication_id,
            operation_digest,
            ready_artifact,
            target_policy,
            targets,
            state,
            revision: u64_from_i64(row.try_get("state_revision")?, "state_revision")?,
            next_attempt_after_ms: row.try_get("next_attempt_after_ms")?,
            signed_event,
        })
    }

    pub async fn claim_phase1_publication_for_signing(
        &self,
        publication_id: i64,
        observed_revision: u64,
        now_ms: i64,
        lease_millis: i64,
    ) -> Result<RadrootsPhase1PublicationClaim, RadrootsPhase1PublicationError> {
        let record = self.load_phase1_publication(publication_id).await?;
        if record.revision != observed_revision {
            return Err(RadrootsPhase1PublicationError::RevisionConflict);
        }
        let expires_at_ms = validated_expiry(now_ms, lease_millis)?;
        let token = new_claim_token()?;
        let affected = sqlx::query(
            "UPDATE outbox_phase1_publication
             SET state = 'claimed-for-signing', state_revision = state_revision + 1,
                 claim_token = ?, claim_expires_at_ms = ?, updated_at_ms = ?
             WHERE publication_id = ? AND state_revision = ? AND signed_event_json IS NULL
               AND state IN ('ready', 'claimed-for-signing', 'failed-retryable')
               AND (claim_token IS NULL OR claim_expires_at_ms <= ?)",
        )
        .bind(token.as_slice())
        .bind(expires_at_ms)
        .bind(now_ms)
        .bind(publication_id)
        .bind(i64_from_u64(observed_revision, "state_revision")?)
        .bind(now_ms)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if affected != 1 {
            return Err(RadrootsPhase1PublicationError::RevisionConflict);
        }
        Ok(RadrootsPhase1PublicationClaim {
            publication_id,
            revision: observed_revision + 1,
            token,
            expires_at_ms,
        })
    }

    pub async fn renew_phase1_publication_claim(
        &self,
        claim: &RadrootsPhase1PublicationClaim,
        now_ms: i64,
        lease_millis: i64,
    ) -> Result<RadrootsPhase1PublicationClaim, RadrootsPhase1PublicationError> {
        self.load_phase1_publication(claim.publication_id).await?;
        let expires_at_ms = validated_expiry(now_ms, lease_millis)?;
        let affected = sqlx::query(
            "UPDATE outbox_phase1_publication
             SET state_revision = state_revision + 1, claim_expires_at_ms = ?, updated_at_ms = ?
             WHERE publication_id = ? AND state = 'claimed-for-signing' AND state_revision = ?
               AND claim_token = ? AND claim_expires_at_ms > ?",
        )
        .bind(expires_at_ms)
        .bind(now_ms)
        .bind(claim.publication_id)
        .bind(i64_from_u64(claim.revision, "state_revision")?)
        .bind(claim.token.as_slice())
        .bind(now_ms)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if affected != 1 {
            return Err(RadrootsPhase1PublicationError::ClaimInvalid);
        }
        Ok(RadrootsPhase1PublicationClaim {
            publication_id: claim.publication_id,
            revision: claim.revision + 1,
            token: claim.token,
            expires_at_ms,
        })
    }

    pub async fn release_phase1_publication_claim(
        &self,
        claim: &RadrootsPhase1PublicationClaim,
        now_ms: i64,
    ) -> Result<RadrootsPhase1PublicationRecord, RadrootsPhase1PublicationError> {
        transition_signing_claim(self, claim, now_ms, "ready", None, now_ms).await
    }

    pub async fn fail_phase1_publication_signing_retryable(
        &self,
        claim: &RadrootsPhase1PublicationClaim,
        now_ms: i64,
        next_attempt_after_ms: i64,
        diagnostic: &str,
    ) -> Result<RadrootsPhase1PublicationRecord, RadrootsPhase1PublicationError> {
        validate_time(next_attempt_after_ms)?;
        validate_diagnostic(diagnostic)?;
        transition_signing_claim(
            self,
            claim,
            now_ms,
            "failed-retryable",
            Some(diagnostic),
            next_attempt_after_ms,
        )
        .await
    }

    pub async fn fail_phase1_publication_signing_terminal(
        &self,
        claim: &RadrootsPhase1PublicationClaim,
        now_ms: i64,
        diagnostic: &str,
    ) -> Result<RadrootsPhase1PublicationRecord, RadrootsPhase1PublicationError> {
        validate_diagnostic(diagnostic)?;
        transition_signing_claim(
            self,
            claim,
            now_ms,
            "failed-terminal",
            Some(diagnostic),
            now_ms,
        )
        .await
    }

    pub async fn quarantine_phase1_publication(
        &self,
        claim: &RadrootsPhase1PublicationClaim,
        now_ms: i64,
        diagnostic: &str,
    ) -> Result<RadrootsPhase1PublicationRecord, RadrootsPhase1PublicationError> {
        validate_diagnostic(diagnostic)?;
        transition_signing_claim(self, claim, now_ms, "quarantined", Some(diagnostic), now_ms).await
    }

    pub async fn cancel_phase1_publication(
        &self,
        claim: &RadrootsPhase1PublicationClaim,
        now_ms: i64,
    ) -> Result<RadrootsPhase1PublicationRecord, RadrootsPhase1PublicationError> {
        transition_signing_claim(self, claim, now_ms, "cancelled", None, now_ms).await
    }

    pub async fn complete_phase1_publication_signing(
        &self,
        claim: &RadrootsPhase1PublicationClaim,
        verified: &RadrootsVerifiedSignedEvent,
        now_ms: i64,
    ) -> Result<RadrootsPhase1PublicationRecord, RadrootsPhase1PublicationError> {
        validate_time(now_ms)?;
        let record = self.load_phase1_publication(claim.publication_id).await?;
        validate_signed_matches_artifact(verified.signed_event(), &record.ready_artifact)?;
        let signed_json = verified.signed_event().raw_json().as_bytes();
        if signed_json.is_empty()
            || signed_json.len() > RADROOTS_PHASE1_PUBLICATION_SIGNED_EVENT_MAX_BYTES
        {
            return Err(RadrootsPhase1PublicationError::SignedEventInvalid);
        }
        let signed_digest: [u8; 32] = Sha256::digest(signed_json).into();
        let signed_event_id = decode_hex32(verified.signed_event().id_str(), "signed_event_id")?;
        let affected = sqlx::query(
            "UPDATE outbox_phase1_publication
             SET state = 'signed-ready', state_revision = state_revision + 1,
                 claim_token = NULL, claim_expires_at_ms = NULL,
                 signed_event_json = ?, signed_event_digest = ?, signed_event_id = ?,
                 last_error = NULL, next_attempt_after_ms = ?, updated_at_ms = ?
             WHERE publication_id = ? AND state = 'claimed-for-signing' AND state_revision = ?
               AND claim_token = ? AND claim_expires_at_ms > ? AND signed_event_json IS NULL",
        )
        .bind(signed_json)
        .bind(signed_digest.as_slice())
        .bind(signed_event_id.as_slice())
        .bind(now_ms)
        .bind(now_ms)
        .bind(claim.publication_id)
        .bind(i64_from_u64(claim.revision, "state_revision")?)
        .bind(claim.token.as_slice())
        .bind(now_ms)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if affected != 1 {
            return Err(RadrootsPhase1PublicationError::ClaimInvalid);
        }
        self.load_phase1_publication(claim.publication_id).await
    }

    pub async fn claim_phase1_publication_target(
        &self,
        publication_id: i64,
        observed_publication_revision: u64,
        target_id: i64,
        observed_target_revision: u64,
        now_ms: i64,
        lease_millis: i64,
    ) -> Result<RadrootsPhase1PublicationTargetClaim, RadrootsPhase1PublicationError> {
        let record = self.load_phase1_publication(publication_id).await?;
        if record.revision != observed_publication_revision
            || record.signed_event.is_none()
            || !matches!(
                record.state,
                RadrootsPhase1PublicationEventState::SignedReady
                    | RadrootsPhase1PublicationEventState::Dispatching
            )
        {
            return Err(RadrootsPhase1PublicationError::RevisionConflict);
        }
        let target = record
            .targets
            .iter()
            .find(|target| target.target_id == target_id)
            .ok_or(RadrootsPhase1PublicationError::TargetNotFound { target_id })?;
        if target.revision != observed_target_revision {
            return Err(RadrootsPhase1PublicationError::RevisionConflict);
        }
        let expires_at_ms = validated_expiry(now_ms, lease_millis)?;
        let token = new_claim_token()?;
        let signed_digest: [u8; 32] = Sha256::digest(
            record
                .signed_event
                .as_ref()
                .expect("checked signed event")
                .signed_event()
                .raw_json()
                .as_bytes(),
        )
        .into();
        let mut transaction = self.pool.begin().await?;
        let target_affected = sqlx::query(
            "UPDATE outbox_phase1_delivery_target
             SET state = 'in-flight', state_revision = state_revision + 1,
                 claim_token = ?, claim_expires_at_ms = ?, updated_at_ms = ?
             WHERE target_id = ? AND publication_id = ? AND state_revision = ?
               AND state IN ('pending', 'failed-retryable', 'uncertain')
               AND (claim_token IS NULL OR claim_expires_at_ms <= ?)",
        )
        .bind(token.as_slice())
        .bind(expires_at_ms)
        .bind(now_ms)
        .bind(target_id)
        .bind(publication_id)
        .bind(i64_from_u64(observed_target_revision, "target_revision")?)
        .bind(now_ms)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        if target_affected != 1 {
            return Err(RadrootsPhase1PublicationError::RevisionConflict);
        }
        let publication_affected = sqlx::query(
            "UPDATE outbox_phase1_publication
             SET state = 'dispatching', state_revision = state_revision + 1, updated_at_ms = ?
             WHERE publication_id = ? AND state_revision = ? AND state IN ('signed-ready', 'dispatching')",
        )
        .bind(now_ms)
        .bind(publication_id)
        .bind(i64_from_u64(
            observed_publication_revision,
            "publication_revision",
        )?)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        if publication_affected != 1 {
            return Err(RadrootsPhase1PublicationError::RevisionConflict);
        }
        sqlx::query(
            "INSERT INTO outbox_phase1_dispatch_intent(intent_digest, target_id, signed_event_digest, state, state_revision, created_at_ms, updated_at_ms)
             VALUES (?, ?, ?, 'in-flight', 0, ?, ?)
             ON CONFLICT(intent_digest) DO UPDATE SET
               state = 'in-flight', state_revision = outbox_phase1_dispatch_intent.state_revision + 1,
               updated_at_ms = excluded.updated_at_ms
             WHERE outbox_phase1_dispatch_intent.target_id = excluded.target_id
               AND outbox_phase1_dispatch_intent.signed_event_digest = excluded.signed_event_digest",
        )
        .bind(target.dispatch_digest.as_slice())
        .bind(target_id)
        .bind(signed_digest.as_slice())
        .bind(now_ms)
        .bind(now_ms)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(RadrootsPhase1PublicationTargetClaim {
            publication_id,
            publication_revision: observed_publication_revision + 1,
            target_id,
            target_revision: observed_target_revision + 1,
            token,
            expires_at_ms,
        })
    }

    pub async fn complete_phase1_target_accepted_pending(
        &self,
        claim: &RadrootsPhase1PublicationTargetClaim,
        now_ms: i64,
    ) -> Result<RadrootsPhase1PublicationRecord, RadrootsPhase1PublicationError> {
        complete_target(
            self,
            claim,
            now_ms,
            RadrootsPhase1PublicationTargetState::AcceptedObservationPending,
            None,
            now_ms,
        )
        .await
    }

    pub async fn complete_phase1_target_accepted_observed(
        &self,
        claim: &RadrootsPhase1PublicationTargetClaim,
        now_ms: i64,
    ) -> Result<RadrootsPhase1PublicationRecord, RadrootsPhase1PublicationError> {
        complete_target(
            self,
            claim,
            now_ms,
            RadrootsPhase1PublicationTargetState::AcceptedObserved,
            None,
            now_ms,
        )
        .await
    }

    pub async fn fail_phase1_target_retryable(
        &self,
        claim: &RadrootsPhase1PublicationTargetClaim,
        now_ms: i64,
        next_attempt_after_ms: i64,
        diagnostic: &str,
    ) -> Result<RadrootsPhase1PublicationRecord, RadrootsPhase1PublicationError> {
        validate_time(next_attempt_after_ms)?;
        validate_diagnostic(diagnostic)?;
        complete_target(
            self,
            claim,
            now_ms,
            RadrootsPhase1PublicationTargetState::FailedRetryable,
            Some(diagnostic),
            next_attempt_after_ms,
        )
        .await
    }

    pub async fn fail_phase1_target_terminal(
        &self,
        claim: &RadrootsPhase1PublicationTargetClaim,
        now_ms: i64,
        diagnostic: &str,
    ) -> Result<RadrootsPhase1PublicationRecord, RadrootsPhase1PublicationError> {
        validate_diagnostic(diagnostic)?;
        complete_target(
            self,
            claim,
            now_ms,
            RadrootsPhase1PublicationTargetState::FailedTerminal,
            Some(diagnostic),
            now_ms,
        )
        .await
    }

    pub async fn mark_phase1_target_uncertain(
        &self,
        claim: &RadrootsPhase1PublicationTargetClaim,
        now_ms: i64,
        diagnostic: &str,
    ) -> Result<RadrootsPhase1PublicationRecord, RadrootsPhase1PublicationError> {
        validate_diagnostic(diagnostic)?;
        complete_target(
            self,
            claim,
            now_ms,
            RadrootsPhase1PublicationTargetState::Uncertain,
            Some(diagnostic),
            now_ms,
        )
        .await
    }

    pub async fn cancel_phase1_target(
        &self,
        claim: &RadrootsPhase1PublicationTargetClaim,
        now_ms: i64,
    ) -> Result<RadrootsPhase1PublicationRecord, RadrootsPhase1PublicationError> {
        complete_target(
            self,
            claim,
            now_ms,
            RadrootsPhase1PublicationTargetState::Cancelled,
            None,
            now_ms,
        )
        .await
    }

    pub async fn complete_phase1_observation_repair(
        &self,
        publication_id: i64,
        target_id: i64,
        observed_target_revision: u64,
        now_ms: i64,
    ) -> Result<RadrootsPhase1PublicationRecord, RadrootsPhase1PublicationError> {
        validate_time(now_ms)?;
        let record = self.load_phase1_publication(publication_id).await?;
        let target = record
            .targets
            .iter()
            .find(|target| target.target_id == target_id)
            .ok_or(RadrootsPhase1PublicationError::TargetNotFound { target_id })?;
        if target.revision != observed_target_revision
            || target.state != RadrootsPhase1PublicationTargetState::AcceptedObservationPending
        {
            return Err(RadrootsPhase1PublicationError::RevisionConflict);
        }
        let repair_digest = repair_digest(&target.dispatch_digest);
        let receipt_digest = receipt_digest(&target.dispatch_digest, "accepted-observed");
        let mut transaction = self.pool.begin().await?;
        let affected = sqlx::query(
            "UPDATE outbox_phase1_delivery_target SET state = 'accepted-observed', state_revision = state_revision + 1, updated_at_ms = ? WHERE target_id = ? AND publication_id = ? AND state = 'accepted-observation-pending' AND state_revision = ?",
        )
        .bind(now_ms)
        .bind(target_id)
        .bind(publication_id)
        .bind(i64_from_u64(observed_target_revision, "target_revision")?)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        if affected != 1 {
            return Err(RadrootsPhase1PublicationError::RevisionConflict);
        }
        let repaired = sqlx::query(
            "UPDATE outbox_phase1_observation_repair SET state = 'complete', state_revision = state_revision + 1, updated_at_ms = ? WHERE repair_digest = ? AND target_id = ? AND state = 'pending'",
        )
        .bind(now_ms)
        .bind(repair_digest.as_slice())
        .bind(target_id)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        if repaired != 1 {
            return Err(RadrootsPhase1PublicationError::StoredAuthorityInvalid);
        }
        sqlx::query(
            "INSERT INTO outbox_phase1_target_receipt(receipt_digest, target_id, observation_kind, observed_at_ms) VALUES (?, ?, 'accepted-observed', ?)",
        )
        .bind(receipt_digest.as_slice())
        .bind(target_id)
        .bind(now_ms)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        self.load_phase1_publication(publication_id).await
    }
}

async fn transition_signing_claim(
    outbox: &RadrootsOutbox,
    claim: &RadrootsPhase1PublicationClaim,
    now_ms: i64,
    destination: &'static str,
    diagnostic: Option<&str>,
    next_attempt_after_ms: i64,
) -> Result<RadrootsPhase1PublicationRecord, RadrootsPhase1PublicationError> {
    validate_time(now_ms)?;
    outbox.load_phase1_publication(claim.publication_id).await?;
    let affected = sqlx::query(
        "UPDATE outbox_phase1_publication
         SET state = ?, state_revision = state_revision + 1, claim_token = NULL,
             claim_expires_at_ms = NULL, last_error = ?, next_attempt_after_ms = ?, updated_at_ms = ?
         WHERE publication_id = ? AND state = 'claimed-for-signing' AND state_revision = ?
           AND claim_token = ? AND claim_expires_at_ms > ? AND signed_event_json IS NULL",
    )
    .bind(destination)
    .bind(diagnostic)
    .bind(next_attempt_after_ms)
    .bind(now_ms)
    .bind(claim.publication_id)
    .bind(i64_from_u64(claim.revision, "state_revision")?)
    .bind(claim.token.as_slice())
    .bind(now_ms)
    .execute(&outbox.pool)
    .await?
    .rows_affected();
    if affected != 1 {
        return Err(RadrootsPhase1PublicationError::ClaimInvalid);
    }
    outbox.load_phase1_publication(claim.publication_id).await
}

async fn complete_target(
    outbox: &RadrootsOutbox,
    claim: &RadrootsPhase1PublicationTargetClaim,
    now_ms: i64,
    destination: RadrootsPhase1PublicationTargetState,
    diagnostic: Option<&str>,
    next_attempt_after_ms: i64,
) -> Result<RadrootsPhase1PublicationRecord, RadrootsPhase1PublicationError> {
    validate_time(now_ms)?;
    let record = outbox.load_phase1_publication(claim.publication_id).await?;
    if record.revision != claim.publication_revision {
        return Err(RadrootsPhase1PublicationError::RevisionConflict);
    }
    let target = record
        .targets
        .iter()
        .find(|target| target.target_id == claim.target_id)
        .ok_or(RadrootsPhase1PublicationError::TargetNotFound {
            target_id: claim.target_id,
        })?;
    if target.revision != claim.target_revision {
        return Err(RadrootsPhase1PublicationError::RevisionConflict);
    }
    let mut transaction = outbox.pool.begin().await?;
    let affected = sqlx::query(
        "UPDATE outbox_phase1_delivery_target
         SET state = ?, state_revision = state_revision + 1, claim_token = NULL,
             claim_expires_at_ms = NULL, last_error = ?, next_attempt_after_ms = ?, updated_at_ms = ?
         WHERE target_id = ? AND publication_id = ? AND state = 'in-flight' AND state_revision = ?
           AND claim_token = ? AND claim_expires_at_ms > ?",
    )
    .bind(destination.as_str())
    .bind(diagnostic)
    .bind(next_attempt_after_ms)
    .bind(now_ms)
    .bind(claim.target_id)
    .bind(claim.publication_id)
    .bind(i64_from_u64(claim.target_revision, "target_revision")?)
    .bind(claim.token.as_slice())
    .bind(now_ms)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    if affected != 1 {
        return Err(RadrootsPhase1PublicationError::ClaimInvalid);
    }
    match destination {
        RadrootsPhase1PublicationTargetState::AcceptedObservationPending
        | RadrootsPhase1PublicationTargetState::AcceptedObserved => {
            let observation_kind = if destination
                == RadrootsPhase1PublicationTargetState::AcceptedObservationPending
            {
                "accepted-pending"
            } else {
                "accepted-observed"
            };
            sqlx::query(
                "INSERT INTO outbox_phase1_target_receipt(receipt_digest, target_id, observation_kind, observed_at_ms) VALUES (?, ?, ?, ?)",
            )
            .bind(receipt_digest(&target.dispatch_digest, observation_kind).as_slice())
            .bind(claim.target_id)
            .bind(observation_kind)
            .bind(now_ms)
            .execute(&mut *transaction)
            .await?;
            if destination == RadrootsPhase1PublicationTargetState::AcceptedObservationPending {
                sqlx::query(
                    "INSERT INTO outbox_phase1_observation_repair(repair_digest, target_id, state, state_revision, created_at_ms, updated_at_ms) VALUES (?, ?, 'pending', 0, ?, ?)",
                )
                .bind(repair_digest(&target.dispatch_digest).as_slice())
                .bind(claim.target_id)
                .bind(now_ms)
                .bind(now_ms)
                .execute(&mut *transaction)
                .await?;
            }
            sqlx::query(
                "UPDATE outbox_phase1_dispatch_intent SET state = 'completed', state_revision = state_revision + 1, updated_at_ms = ? WHERE intent_digest = ? AND target_id = ?",
            )
            .bind(now_ms)
            .bind(target.dispatch_digest.as_slice())
            .bind(claim.target_id)
            .execute(&mut *transaction)
            .await?;
        }
        RadrootsPhase1PublicationTargetState::Uncertain => {
            sqlx::query(
                "UPDATE outbox_phase1_dispatch_intent SET state = 'uncertain', state_revision = state_revision + 1, updated_at_ms = ? WHERE intent_digest = ? AND target_id = ?",
            )
            .bind(now_ms)
            .bind(target.dispatch_digest.as_slice())
            .bind(claim.target_id)
            .execute(&mut *transaction)
            .await?;
        }
        RadrootsPhase1PublicationTargetState::Cancelled => {
            sqlx::query(
                "UPDATE outbox_phase1_dispatch_intent SET state = 'cancelled', state_revision = state_revision + 1, updated_at_ms = ? WHERE intent_digest = ? AND target_id = ?",
            )
            .bind(now_ms)
            .bind(target.dispatch_digest.as_slice())
            .bind(claim.target_id)
            .execute(&mut *transaction)
            .await?;
        }
        RadrootsPhase1PublicationTargetState::FailedRetryable
        | RadrootsPhase1PublicationTargetState::FailedTerminal => {
            sqlx::query(
                "UPDATE outbox_phase1_dispatch_intent SET state = 'completed', state_revision = state_revision + 1, updated_at_ms = ? WHERE intent_digest = ? AND target_id = ?",
            )
            .bind(now_ms)
            .bind(target.dispatch_digest.as_slice())
            .bind(claim.target_id)
            .execute(&mut *transaction)
            .await?;
        }
        RadrootsPhase1PublicationTargetState::Pending
        | RadrootsPhase1PublicationTargetState::InFlight => {
            return Err(RadrootsPhase1PublicationError::StateConflict);
        }
    }
    let destination_event = aggregate_publication_state(
        &mut transaction,
        claim.publication_id,
        record.target_policy.required_target_count,
    )
    .await?;
    let publication_affected = sqlx::query(
        "UPDATE outbox_phase1_publication SET state = ?, state_revision = state_revision + 1, updated_at_ms = ? WHERE publication_id = ? AND state_revision = ? AND state = 'dispatching'",
    )
    .bind(destination_event.as_str())
    .bind(now_ms)
    .bind(claim.publication_id)
    .bind(i64_from_u64(
        claim.publication_revision,
        "publication_revision",
    )?)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    if publication_affected != 1 {
        return Err(RadrootsPhase1PublicationError::RevisionConflict);
    }
    transaction.commit().await?;
    outbox.load_phase1_publication(claim.publication_id).await
}

async fn aggregate_publication_state(
    transaction: &mut Transaction<'_, Sqlite>,
    publication_id: i64,
    required_target_count: usize,
) -> Result<RadrootsPhase1PublicationEventState, RadrootsPhase1PublicationError> {
    let row = sqlx::query(
        "SELECT
           SUM(CASE WHEN state IN ('accepted-observation-pending', 'accepted-observed') THEN 1 ELSE 0 END) AS accepted,
           SUM(CASE WHEN state IN ('pending', 'in-flight', 'failed-retryable', 'uncertain') THEN 1 ELSE 0 END) AS recoverable
         FROM outbox_phase1_delivery_target WHERE publication_id = ?",
    )
    .bind(publication_id)
    .fetch_one(&mut **transaction)
    .await?;
    let accepted = usize_from_i64(row.try_get("accepted")?, "accepted_target_count")?;
    let recoverable = usize_from_i64(row.try_get("recoverable")?, "recoverable_target_count")?;
    if accepted >= required_target_count {
        Ok(RadrootsPhase1PublicationEventState::Published)
    } else if accepted + recoverable < required_target_count {
        Ok(RadrootsPhase1PublicationEventState::FailedTerminal)
    } else {
        Ok(RadrootsPhase1PublicationEventState::SignedReady)
    }
}

async fn load_targets(
    pool: &sqlx::SqlitePool,
    publication_id: i64,
) -> Result<Vec<RadrootsPhase1PublicationTarget>, RadrootsPhase1PublicationError> {
    let rows = sqlx::query(
        "SELECT target_id, target_ordinal,
            length(CAST(endpoint_uri AS BLOB)) AS endpoint_uri_bytes,
            CASE WHEN length(CAST(endpoint_uri AS BLOB)) <= ? THEN endpoint_uri END AS bounded_endpoint_uri,
            endpoint_fingerprint, dispatch_digest, state, state_revision, next_attempt_after_ms
         FROM outbox_phase1_delivery_target WHERE publication_id = ?
         ORDER BY target_ordinal LIMIT 17",
    )
    .bind(i64::try_from(RADROOTS_PHASE1_PUBLICATION_TARGET_URI_MAX_BYTES).unwrap_or(i64::MAX))
    .bind(publication_id)
    .fetch_all(pool)
    .await?;
    if rows.len() > RADROOTS_PHASE1_PUBLICATION_TARGET_MAX_COUNT {
        return Err(RadrootsPhase1PublicationError::TargetCount {
            max: RADROOTS_PHASE1_PUBLICATION_TARGET_MAX_COUNT,
            actual: rows.len(),
        });
    }
    rows.into_iter()
        .map(|row| {
            let actual = usize_from_i64(row.try_get("endpoint_uri_bytes")?, "endpoint_uri_bytes")?;
            if actual > RADROOTS_PHASE1_PUBLICATION_TARGET_URI_MAX_BYTES {
                return Err(RadrootsPhase1PublicationError::StoredValueTooLarge {
                    field: "endpoint_uri",
                    max: RADROOTS_PHASE1_PUBLICATION_TARGET_URI_MAX_BYTES,
                    actual,
                });
            }
            let endpoint_uri: Option<String> = row.try_get("bounded_endpoint_uri")?;
            let endpoint_uri =
                endpoint_uri.ok_or(RadrootsPhase1PublicationError::StoredAuthorityInvalid)?;
            Ok(RadrootsPhase1PublicationTarget {
                target_id: row.try_get("target_id")?,
                ordinal: usize_from_i64(row.try_get("target_ordinal")?, "target_ordinal")?,
                endpoint_uri,
                endpoint_fingerprint: blob32(&row, "endpoint_fingerprint")?,
                dispatch_digest: blob32(&row, "dispatch_digest")?,
                state: RadrootsPhase1PublicationTargetState::parse(
                    &row.try_get::<String, _>("state")?,
                )?,
                revision: u64_from_i64(row.try_get("state_revision")?, "target_revision")?,
                next_attempt_after_ms: row.try_get("next_attempt_after_ms")?,
            })
        })
        .collect()
}

fn prepare_publication(
    ready: &RadrootsPhase1MediaReadyPublicationArtifact,
    target_policy: &RadrootsPhase1PublicationTargetPolicy,
) -> Result<PreparedPublication, RadrootsPhase1PublicationError> {
    validate_phase1_publication_media_readiness(ready).map_err(|error| {
        RadrootsPhase1PublicationError::ReadinessInvalid {
            source_code: error.code(),
        }
    })?;
    let artifact_json = ready.artifact().to_canonical_json();
    let allowlisted = allow_phase1_publication_canonical_json(&artifact_json).map_err(|error| {
        RadrootsPhase1PublicationError::ArtifactInvalid {
            source_code: error.code(),
        }
    })?;
    let readiness_json = ready.to_canonical_json();
    let reloaded = RadrootsPhase1MediaReadyPublicationArtifact::from_canonical_json(
        allowlisted,
        &readiness_json,
    )
    .map_err(|error| RadrootsPhase1PublicationError::ReadinessInvalid {
        source_code: error.code(),
    })?;
    if &reloaded != ready {
        return Err(RadrootsPhase1PublicationError::StoredAuthorityInvalid);
    }
    let artifact = ready.artifact();
    let artifact_digest = *artifact.artifact_digest().as_bytes();
    let readiness_digest = *ready.binding_digest().as_bytes();
    let expected_author = decode_hex32(artifact.expected_author().as_str(), "expected_author")?;
    let expected_event_id =
        decode_hex32(artifact.expected_event_id().as_str(), "expected_event_id")?;
    let operation_digest = operation_digest_for(
        &artifact_digest,
        &readiness_digest,
        &expected_author,
        &target_policy.digest,
    );
    Ok(PreparedPublication {
        artifact_json,
        artifact_digest,
        readiness_json,
        readiness_digest,
        semantic_role: artifact.semantic_variant().as_str(),
        expected_author,
        expected_event_id,
        operation_digest,
    })
}

fn validate_signed_matches_artifact(
    signed: &RadrootsSignedEvent,
    ready: &RadrootsPhase1MediaReadyPublicationArtifact,
) -> Result<(), RadrootsPhase1PublicationError> {
    let artifact = ready.artifact();
    let draft = artifact.draft();
    if signed.pubkey_str() != artifact.expected_author().as_str()
        || signed.id_str() != artifact.expected_event_id().as_str()
        || signed.created_at() != draft.created_at()
        || signed.kind() != draft.kind()
        || signed.tags_as_vec() != draft.tags()
        || signed.content() != draft.content()
    {
        return Err(RadrootsPhase1PublicationError::SignedEventMismatch);
    }
    Ok(())
}

fn reload_signed_event(
    signed_json: Option<Vec<u8>>,
    signed_digest: Option<Vec<u8>>,
    signed_event_id: Option<Vec<u8>>,
    ready: &RadrootsPhase1MediaReadyPublicationArtifact,
) -> Result<Option<RadrootsVerifiedSignedEvent>, RadrootsPhase1PublicationError> {
    match (signed_json, signed_digest, signed_event_id) {
        (None, None, None) => Ok(None),
        (Some(bytes), Some(stored_digest), Some(stored_event_id)) => {
            if stored_digest.as_slice() != Sha256::digest(&bytes).as_slice()
                || stored_event_id.len() != 32
            {
                return Err(RadrootsPhase1PublicationError::StoredAuthorityInvalid);
            }
            let raw = String::from_utf8(bytes)
                .map_err(|_| RadrootsPhase1PublicationError::SignedEventInvalid)?;
            let wire = RadrootsNip01EventWire::parse_json(&raw)
                .map_err(|_| RadrootsPhase1PublicationError::SignedEventInvalid)?;
            let signed = RadrootsSignedEvent::from_wire_verified_id(wire, raw)
                .map_err(|_| RadrootsPhase1PublicationError::SignedEventInvalid)?;
            if decode_hex32(signed.id_str(), "signed_event_id")?.as_slice()
                != stored_event_id.as_slice()
            {
                return Err(RadrootsPhase1PublicationError::StoredAuthorityInvalid);
            }
            validate_signed_matches_artifact(&signed, ready)?;
            let verified = signed
                .verify_signature()
                .map_err(|_| RadrootsPhase1PublicationError::SignedEventInvalid)?;
            Ok(Some(verified))
        }
        _ => Err(RadrootsPhase1PublicationError::StoredAuthorityInvalid),
    }
}

fn validate_target_identities(
    targets: &[RadrootsPhase1PublicationTarget],
    expected_event_id: &[u8; 32],
    target_policy_digest: &[u8; 32],
) -> Result<(), RadrootsPhase1PublicationError> {
    let mut ids = BTreeSet::new();
    for (ordinal, target) in targets.iter().enumerate() {
        if target.ordinal != ordinal
            || canonical_target_uri(&target.endpoint_uri)? != target.endpoint_uri
            || target.endpoint_fingerprint != endpoint_fingerprint(&target.endpoint_uri)
            || target.dispatch_digest
                != dispatch_digest(
                    expected_event_id,
                    target_policy_digest,
                    &target.endpoint_fingerprint,
                )
            || !ids.insert(target.target_id)
        {
            return Err(RadrootsPhase1PublicationError::StoredAuthorityInvalid);
        }
    }
    Ok(())
}

fn canonical_target_uri(value: &str) -> Result<String, RadrootsPhase1PublicationError> {
    if value.is_empty() || value.len() > RADROOTS_PHASE1_PUBLICATION_TARGET_URI_MAX_BYTES {
        return Err(RadrootsPhase1PublicationError::TargetUriTooLarge {
            max: RADROOTS_PHASE1_PUBLICATION_TARGET_URI_MAX_BYTES,
            actual: value.len(),
        });
    }
    let parsed = Url::parse(value).map_err(|_| RadrootsPhase1PublicationError::TargetUriInvalid)?;
    if !matches!(parsed.scheme(), "ws" | "wss")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
    {
        return Err(RadrootsPhase1PublicationError::TargetUriInvalid);
    }
    let canonical = parsed.to_string();
    if canonical.len() > RADROOTS_PHASE1_PUBLICATION_TARGET_URI_MAX_BYTES {
        return Err(RadrootsPhase1PublicationError::TargetUriTooLarge {
            max: RADROOTS_PHASE1_PUBLICATION_TARGET_URI_MAX_BYTES,
            actual: canonical.len(),
        });
    }
    Ok(canonical)
}

fn target_policy_digest(
    targets: &[String],
    required_target_count: usize,
) -> Result<[u8; 32], RadrootsPhase1PublicationError> {
    let mut digest = Sha256::new();
    digest.update(TARGET_POLICY_DOMAIN);
    digest.update(
        u16::try_from(required_target_count)
            .map_err(|_| RadrootsPhase1PublicationError::IntegerRange {
                field: "required_target_count",
            })?
            .to_be_bytes(),
    );
    digest.update(
        u16::try_from(targets.len())
            .map_err(|_| RadrootsPhase1PublicationError::IntegerRange {
                field: "target_count",
            })?
            .to_be_bytes(),
    );
    for target in targets {
        digest.update(endpoint_fingerprint(target));
    }
    Ok(digest.finalize().into())
}

fn endpoint_fingerprint(target: &str) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(ENDPOINT_DOMAIN);
    digest.update((target.len() as u64).to_be_bytes());
    digest.update(target.as_bytes());
    digest.finalize().into()
}

fn operation_digest_for(
    artifact_digest: &[u8; 32],
    readiness_digest: &[u8; 32],
    expected_author: &[u8; 32],
    target_policy_digest: &[u8; 32],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(OPERATION_DOMAIN);
    digest.update(artifact_digest);
    digest.update(readiness_digest);
    digest.update(expected_author);
    digest.update(target_policy_digest);
    digest.finalize().into()
}

fn dispatch_digest(
    event_id: &[u8; 32],
    target_policy_digest: &[u8; 32],
    endpoint_fingerprint: &[u8; 32],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(DISPATCH_DOMAIN);
    digest.update(event_id);
    digest.update(target_policy_digest);
    digest.update(endpoint_fingerprint);
    digest.finalize().into()
}

fn receipt_digest(dispatch_digest: &[u8; 32], observation_kind: &str) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(RECEIPT_DOMAIN);
    digest.update(dispatch_digest);
    digest.update((observation_kind.len() as u64).to_be_bytes());
    digest.update(observation_kind.as_bytes());
    digest.finalize().into()
}

fn repair_digest(dispatch_digest: &[u8; 32]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(REPAIR_DOMAIN);
    digest.update(dispatch_digest);
    digest.finalize().into()
}

fn new_claim_token() -> Result<[u8; 32], RadrootsPhase1PublicationError> {
    let mut token = [0_u8; 32];
    getrandom::getrandom(&mut token)
        .map_err(|_| RadrootsPhase1PublicationError::EntropyUnavailable)?;
    Ok(token)
}

fn validated_expiry(now_ms: i64, lease_millis: i64) -> Result<i64, RadrootsPhase1PublicationError> {
    validate_time(now_ms)?;
    if !(1..=RADROOTS_PHASE1_PUBLICATION_CLAIM_LEASE_MAX_MILLIS).contains(&lease_millis) {
        return Err(RadrootsPhase1PublicationError::InvalidLease {
            max_millis: RADROOTS_PHASE1_PUBLICATION_CLAIM_LEASE_MAX_MILLIS,
            actual: lease_millis,
        });
    }
    now_ms
        .checked_add(lease_millis)
        .ok_or(RadrootsPhase1PublicationError::InvalidTime)
}

fn validate_time(value: i64) -> Result<(), RadrootsPhase1PublicationError> {
    if value < 0 {
        Err(RadrootsPhase1PublicationError::InvalidTime)
    } else {
        Ok(())
    }
}

fn validate_diagnostic(value: &str) -> Result<(), RadrootsPhase1PublicationError> {
    if value.len() > RADROOTS_PHASE1_PUBLICATION_DIAGNOSTIC_MAX_BYTES {
        Err(RadrootsPhase1PublicationError::DiagnosticTooLarge {
            max: RADROOTS_PHASE1_PUBLICATION_DIAGNOSTIC_MAX_BYTES,
            actual: value.len(),
        })
    } else {
        Ok(())
    }
}

fn decode_hex32(
    value: &str,
    field: &'static str,
) -> Result<[u8; 32], RadrootsPhase1PublicationError> {
    let bytes = hex::decode(value)
        .map_err(|_| RadrootsPhase1PublicationError::StoredDigestInvalid { field })?;
    bytes
        .try_into()
        .map_err(|_| RadrootsPhase1PublicationError::StoredDigestInvalid { field })
}

fn blob32(
    row: &sqlx::sqlite::SqliteRow,
    field: &'static str,
) -> Result<[u8; 32], RadrootsPhase1PublicationError> {
    let bytes: Vec<u8> = row.try_get(field)?;
    bytes
        .try_into()
        .map_err(|_| RadrootsPhase1PublicationError::StoredDigestInvalid { field })
}

fn bounded_blob(
    row: &sqlx::sqlite::SqliteRow,
    value_field: &'static str,
    length_field: &'static str,
    authority_field: &'static str,
    max: usize,
) -> Result<Option<Vec<u8>>, RadrootsPhase1PublicationError> {
    let length: Option<i64> = row.try_get(length_field)?;
    let Some(length) = length else {
        return Ok(None);
    };
    let actual = usize_from_i64(length, length_field)?;
    if actual > max {
        return Err(RadrootsPhase1PublicationError::StoredValueTooLarge {
            field: authority_field,
            max,
            actual,
        });
    }
    row.try_get(value_field).map_err(Into::into)
}

fn usize_from_i64(
    value: i64,
    field: &'static str,
) -> Result<usize, RadrootsPhase1PublicationError> {
    usize::try_from(value).map_err(|_| RadrootsPhase1PublicationError::IntegerRange { field })
}

fn u64_from_i64(value: i64, field: &'static str) -> Result<u64, RadrootsPhase1PublicationError> {
    u64::try_from(value).map_err(|_| RadrootsPhase1PublicationError::IntegerRange { field })
}

fn i64_from_u64(value: u64, field: &'static str) -> Result<i64, RadrootsPhase1PublicationError> {
    i64::try_from(value).map_err(|_| RadrootsPhase1PublicationError::IntegerRange { field })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RadrootsOutboxRollbackConfirmation;
    use radroots_event::post::RadrootsAuthoredUpdate;
    use radroots_event_codec::wire::publication::allowlist::allow_phase1_publication_artifact;
    use radroots_event_codec::wire::publication::{
        RadrootsPhase1PublicationArtifact, bind_phase1_publication_media_readiness,
    };
    use radroots_nostr::prelude::{
        RadrootsNostrKeys, RadrootsNostrSecretKey, RadrootsNostrTimestamp,
        radroots_nostr_build_update_event,
    };

    const ALICE_SECRET_KEY: &str =
        "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
    const ALICE_PUBLIC_KEY: &str =
        "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
    const DESCRIPTOR: &[u8] = include_bytes!("../contracts/phase1_publication_v1.descriptor.json");

    fn ready_update() -> RadrootsPhase1MediaReadyPublicationArtifact {
        ready_update_with_content("Carrots harvested in Victoria today")
    }

    fn ready_update_with_content(content: &str) -> RadrootsPhase1MediaReadyPublicationArtifact {
        let update = RadrootsAuthoredUpdate::new(content).unwrap();
        let artifact = RadrootsPhase1PublicationArtifact::from_update(
            &update,
            1_780_000_000,
            ALICE_PUBLIC_KEY,
        )
        .unwrap();
        let allowlisted = allow_phase1_publication_artifact(artifact).unwrap();
        bind_phase1_publication_media_readiness(allowlisted, Vec::new()).unwrap()
    }

    fn signed_update(
        ready: &RadrootsPhase1MediaReadyPublicationArtifact,
    ) -> RadrootsVerifiedSignedEvent {
        let artifact = ready.artifact();
        let draft = artifact.draft();
        let update = RadrootsAuthoredUpdate::new("Carrots harvested in Victoria today").unwrap();
        let secret = RadrootsNostrSecretKey::from_hex(ALICE_SECRET_KEY).unwrap();
        let event = radroots_nostr_build_update_event(&update)
            .unwrap()
            .custom_created_at(RadrootsNostrTimestamp::from_secs(draft.created_at()))
            .sign_with_keys(&RadrootsNostrKeys::new(secret))
            .unwrap();
        let raw = serde_json::to_string(&event).unwrap();
        let wire = RadrootsNip01EventWire::parse_json(&raw).unwrap();
        let signed = RadrootsSignedEvent::from_wire_verified_id(wire, raw).unwrap();
        assert_eq!(signed.id_str(), artifact.expected_event_id().as_str());
        signed.verify_signature().unwrap()
    }

    #[test]
    fn phase1_publication_transition_matrix_is_closed_and_unique() {
        let event_states = BTreeSet::from([
            "ready",
            "claimed-for-signing",
            "signed-ready",
            "dispatching",
            "published",
            "failed-retryable",
            "failed-terminal",
            "quarantined",
            "cancelled",
        ]);
        let target_states = BTreeSet::from([
            "pending",
            "in-flight",
            "accepted-observation-pending",
            "accepted-observed",
            "failed-retryable",
            "failed-terminal",
            "uncertain",
            "cancelled",
        ]);
        let mut ids = BTreeSet::new();
        for transition in RADROOTS_PHASE1_PUBLICATION_TRANSITIONS {
            assert!(ids.insert(transition.id));
            let states = match transition.scope {
                RadrootsPhase1PublicationTransitionScope::Event => &event_states,
                RadrootsPhase1PublicationTransitionScope::Target => &target_states,
            };
            assert!(states.contains(transition.from));
            assert!(states.contains(transition.to));
            assert!(transition.revision_cas);
            assert!(!transition.lease_predicate.is_empty());
            assert!(!transition.durable_side_effect.is_empty());
        }
        assert_eq!(ids.len(), 25);
    }

    #[test]
    fn phase1_publication_descriptor_matches_runtime_authority() {
        let descriptor: serde_json::Value = serde_json::from_slice(DESCRIPTOR).unwrap();
        assert_eq!(
            descriptor["resource_limits"],
            serde_json::json!({
                "target_count": RADROOTS_PHASE1_PUBLICATION_TARGET_MAX_COUNT,
                "target_uri_bytes": RADROOTS_PHASE1_PUBLICATION_TARGET_URI_MAX_BYTES,
                "diagnostic_bytes": RADROOTS_PHASE1_PUBLICATION_DIAGNOSTIC_MAX_BYTES,
                "claim_lease_millis": RADROOTS_PHASE1_PUBLICATION_CLAIM_LEASE_MAX_MILLIS,
            })
        );
        assert_eq!(
            descriptor["stable_errors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap())
                .collect::<Vec<_>>(),
            RADROOTS_PHASE1_PUBLICATION_ERROR_CODES
        );
        let transitions = descriptor["transitions"].as_array().unwrap();
        assert_eq!(
            transitions.len(),
            RADROOTS_PHASE1_PUBLICATION_TRANSITIONS.len()
        );
        for (wire, runtime) in transitions
            .iter()
            .zip(RADROOTS_PHASE1_PUBLICATION_TRANSITIONS)
        {
            assert_eq!(wire["id"], runtime.id);
            assert_eq!(wire["scope"], runtime.scope.as_str());
            assert_eq!(wire["from"], runtime.from);
            assert_eq!(wire["to"], runtime.to);
            assert_eq!(wire["revision_cas"], runtime.revision_cas);
            assert_eq!(wire["lease_predicate"], runtime.lease_predicate);
            assert_eq!(wire["durable_side_effect"], runtime.durable_side_effect);
            assert_eq!(wire["retry_class"], runtime.retry_class.as_str());
            assert_eq!(wire["repair_edge"], runtime.repair_edge);
            assert_eq!(wire["terminal_destination"], runtime.terminal_destination);
        }
    }

    #[test]
    fn phase1_publication_target_policy_is_canonical_bounded_and_stable() {
        let policy = RadrootsPhase1PublicationTargetPolicy::new(
            ["wss://B.example:443", "wss://a.example/relay"],
            1,
        )
        .unwrap();
        assert_eq!(
            policy.targets(),
            ["wss://a.example/relay", "wss://b.example/"]
        );
        assert_eq!(
            policy,
            RadrootsPhase1PublicationTargetPolicy::new(
                ["wss://a.example/relay", "wss://b.example/"],
                1,
            )
            .unwrap()
        );
        assert!(matches!(
            RadrootsPhase1PublicationTargetPolicy::new(Vec::<String>::new(), 0),
            Err(RadrootsPhase1PublicationError::RequiredTargetCount { .. })
        ));
        assert!(matches!(
            RadrootsPhase1PublicationTargetPolicy::new(
                ["wss://same.example", "wss://same.example/"],
                1
            ),
            Err(RadrootsPhase1PublicationError::DuplicateTarget)
        ));
        let one_over = format!("wss://example.com/{}", "a".repeat(2_048));
        assert!(matches!(
            RadrootsPhase1PublicationTargetPolicy::new([one_over], 1),
            Err(RadrootsPhase1PublicationError::TargetUriTooLarge { .. })
        ));
        let targets = (0..=RADROOTS_PHASE1_PUBLICATION_TARGET_MAX_COUNT)
            .map(|index| format!("wss://relay-{index}.example"))
            .collect::<Vec<_>>();
        assert!(matches!(
            RadrootsPhase1PublicationTargetPolicy::new(targets, 1),
            Err(RadrootsPhase1PublicationError::TargetCount { .. })
        ));

        let prefix = "wss://example.com/";
        let exact = format!(
            "{prefix}{}",
            "a".repeat(RADROOTS_PHASE1_PUBLICATION_TARGET_URI_MAX_BYTES - prefix.len())
        );
        assert_eq!(
            exact.len(),
            RADROOTS_PHASE1_PUBLICATION_TARGET_URI_MAX_BYTES
        );
        assert!(RadrootsPhase1PublicationTargetPolicy::new([exact.clone()], 1).is_ok());
        assert!(matches!(
            RadrootsPhase1PublicationTargetPolicy::new([format!("{exact}a")], 1),
            Err(RadrootsPhase1PublicationError::TargetUriTooLarge { .. })
        ));
        let exact_targets = (0..RADROOTS_PHASE1_PUBLICATION_TARGET_MAX_COUNT)
            .map(|index| format!("wss://relay-{index}.example"))
            .collect::<Vec<_>>();
        assert!(RadrootsPhase1PublicationTargetPolicy::new(exact_targets, 16).is_ok());
    }

    #[tokio::test]
    async fn phase1_publication_enqueue_is_typed_idempotent_and_revalidated() {
        let outbox = RadrootsOutbox::open_memory().await.unwrap();
        let ready = ready_update();
        let policy = RadrootsPhase1PublicationTargetPolicy::new(
            ["wss://relay-b.example", "wss://relay-a.example"],
            1,
        )
        .unwrap();
        let inserted = outbox
            .enqueue_phase1_publication(&ready, &policy, 10)
            .await
            .unwrap();
        assert_eq!(
            inserted.status(),
            RadrootsPhase1PublicationEnqueueStatus::Inserted
        );
        assert_eq!(
            inserted.record().state(),
            RadrootsPhase1PublicationEventState::Ready
        );
        assert_eq!(inserted.record().targets().len(), 2);
        assert_eq!(
            inserted.record().targets()[0].endpoint_uri(),
            "wss://relay-a.example/"
        );
        let duplicate = outbox
            .enqueue_phase1_publication(&ready, &policy, 99)
            .await
            .unwrap();
        assert_eq!(
            duplicate.status(),
            RadrootsPhase1PublicationEnqueueStatus::Existing
        );
        assert_eq!(
            duplicate.record().operation_digest(),
            inserted.record().operation_digest()
        );
        let conflicting_policy =
            RadrootsPhase1PublicationTargetPolicy::new(["wss://other.example"], 1).unwrap();
        assert_eq!(
            outbox
                .enqueue_phase1_publication(&ready, &conflicting_policy, 100)
                .await
                .unwrap_err()
                .code(),
            "phase1_publication_idempotency_conflict"
        );

        sqlx::query(
            "UPDATE outbox_phase1_publication SET readiness_digest = zeroblob(32) WHERE publication_id = ?",
        )
        .bind(inserted.record().publication_id())
        .execute(outbox.pool())
        .await
        .unwrap();
        assert_eq!(
            outbox
                .load_phase1_publication(inserted.record().publication_id())
                .await
                .unwrap_err()
                .code(),
            "phase1_publication_stored_authority_invalid"
        );
    }

    #[tokio::test]
    async fn phase1_publication_enqueue_failure_rolls_back_every_typed_row() {
        let outbox = RadrootsOutbox::open_memory().await.unwrap();
        sqlx::query(
            "CREATE TEMP TRIGGER phase1_publication_fail_target BEFORE INSERT ON outbox_phase1_delivery_target BEGIN SELECT RAISE(ABORT, 'injected target failure'); END",
        )
        .execute(outbox.pool())
        .await
        .unwrap();
        let result = outbox
            .enqueue_phase1_publication(
                &ready_update(),
                &RadrootsPhase1PublicationTargetPolicy::new(["wss://relay.example"], 1).unwrap(),
                1,
            )
            .await;
        assert!(matches!(
            result,
            Err(RadrootsPhase1PublicationError::Sqlite(_))
        ));
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox_phase1_publication")
                .fetch_one(outbox.pool())
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox_phase1_delivery_target")
                .fetch_one(outbox.pool())
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn phase1_publication_reload_rejects_malformed_swapped_and_cross_row_authority() {
        for mutation in [
            "UPDATE outbox_phase1_publication SET artifact_json = x'7b7d' WHERE publication_id = ?",
            "UPDATE outbox_phase1_publication SET artifact_json = substr(artifact_json, 1, length(artifact_json) - 1) WHERE publication_id = ?",
            "UPDATE outbox_phase1_publication SET artifact_json = CAST(artifact_json || x'20' AS BLOB) WHERE publication_id = ?",
            "UPDATE outbox_phase1_publication SET artifact_json = readiness_json WHERE publication_id = ?",
            "UPDATE outbox_phase1_delivery_target SET endpoint_uri = 'wss://retarget.example/' WHERE publication_id = ?",
        ] {
            let outbox = RadrootsOutbox::open_memory().await.unwrap();
            let receipt = outbox
                .enqueue_phase1_publication(
                    &ready_update(),
                    &RadrootsPhase1PublicationTargetPolicy::new(["wss://relay.example"], 1)
                        .unwrap(),
                    1,
                )
                .await
                .unwrap();
            sqlx::query(mutation)
                .bind(receipt.record().publication_id())
                .execute(outbox.pool())
                .await
                .unwrap();
            assert!(
                outbox
                    .load_phase1_publication(receipt.record().publication_id())
                    .await
                    .is_err(),
                "mutation must fail closed: {mutation}"
            );
        }

        let outbox = RadrootsOutbox::open_memory().await.unwrap();
        let policy =
            RadrootsPhase1PublicationTargetPolicy::new(["wss://relay.example"], 1).unwrap();
        let first = outbox
            .enqueue_phase1_publication(&ready_update(), &policy, 1)
            .await
            .unwrap();
        let second = outbox
            .enqueue_phase1_publication(
                &ready_update_with_content("A second Victoria harvest update"),
                &policy,
                2,
            )
            .await
            .unwrap();
        sqlx::query(
            "UPDATE outbox_phase1_publication SET artifact_json = (SELECT artifact_json FROM outbox_phase1_publication WHERE publication_id = ?) WHERE publication_id = ?",
        )
        .bind(second.record().publication_id())
        .bind(first.record().publication_id())
        .execute(outbox.pool())
        .await
        .unwrap();
        assert_eq!(
            outbox
                .load_phase1_publication(first.record().publication_id())
                .await
                .unwrap_err()
                .code(),
            "phase1_publication_readiness_invalid"
        );
    }

    #[tokio::test]
    async fn phase1_publication_claims_are_lease_fenced_and_race_safe() {
        let outbox = RadrootsOutbox::open_memory().await.unwrap();
        let receipt = outbox
            .enqueue_phase1_publication(
                &ready_update(),
                &RadrootsPhase1PublicationTargetPolicy::new(["wss://relay.example"], 1).unwrap(),
                10,
            )
            .await
            .unwrap();
        let publication_id = receipt.record().publication_id();
        let first = outbox
            .claim_phase1_publication_for_signing(publication_id, 0, 20, 10)
            .await
            .unwrap();
        assert_eq!(
            outbox
                .claim_phase1_publication_for_signing(publication_id, 0, 20, 10)
                .await
                .unwrap_err()
                .code(),
            "phase1_publication_revision_conflict"
        );
        let reclaimed = outbox
            .claim_phase1_publication_for_signing(publication_id, 1, 30, 10)
            .await
            .unwrap();
        assert_eq!(
            outbox
                .renew_phase1_publication_claim(&first, 31, 10)
                .await
                .unwrap_err()
                .code(),
            "phase1_publication_claim_invalid"
        );
        let retryable = outbox
            .fail_phase1_publication_signing_retryable(&reclaimed, 31, 40, "signer unavailable")
            .await
            .unwrap();
        assert_eq!(
            retryable.state(),
            RadrootsPhase1PublicationEventState::FailedRetryable
        );
        let final_claim = outbox
            .claim_phase1_publication_for_signing(publication_id, retryable.revision(), 40, 10)
            .await
            .unwrap();
        let cancelled = outbox
            .cancel_phase1_publication(&final_claim, 41)
            .await
            .unwrap();
        assert_eq!(
            cancelled.state(),
            RadrootsPhase1PublicationEventState::Cancelled
        );
    }

    #[tokio::test]
    async fn phase1_publication_signed_dispatch_and_observation_repair_are_durable() {
        let outbox = RadrootsOutbox::open_memory().await.unwrap();
        let ready = ready_update();
        let receipt = outbox
            .enqueue_phase1_publication(
                &ready,
                &RadrootsPhase1PublicationTargetPolicy::new(
                    ["wss://relay-a.example", "wss://relay-b.example"],
                    1,
                )
                .unwrap(),
                100,
            )
            .await
            .unwrap();
        let claim = outbox
            .claim_phase1_publication_for_signing(
                receipt.record().publication_id(),
                receipt.record().revision(),
                101,
                100,
            )
            .await
            .unwrap();
        let signed = signed_update(&ready);
        let signed_record = outbox
            .complete_phase1_publication_signing(&claim, &signed, 102)
            .await
            .unwrap();
        assert_eq!(
            signed_record.state(),
            RadrootsPhase1PublicationEventState::SignedReady
        );
        assert_eq!(
            signed_record
                .signed_event()
                .unwrap()
                .signed_event()
                .raw_json(),
            signed.signed_event().raw_json()
        );
        let target = &signed_record.targets()[0];
        let target_claim = outbox
            .claim_phase1_publication_target(
                signed_record.publication_id(),
                signed_record.revision(),
                target.target_id(),
                target.revision(),
                103,
                100,
            )
            .await
            .unwrap();
        let pending = outbox
            .complete_phase1_target_accepted_pending(&target_claim, 104)
            .await
            .unwrap();
        assert_eq!(
            pending.state(),
            RadrootsPhase1PublicationEventState::Published
        );
        let repaired_target = pending
            .targets()
            .iter()
            .find(|candidate| candidate.target_id() == target_claim.target_id())
            .unwrap();
        assert_eq!(
            repaired_target.state(),
            RadrootsPhase1PublicationTargetState::AcceptedObservationPending
        );
        let repaired = outbox
            .complete_phase1_observation_repair(
                pending.publication_id(),
                repaired_target.target_id(),
                repaired_target.revision(),
                105,
            )
            .await
            .unwrap();
        assert_eq!(
            repaired.state(),
            RadrootsPhase1PublicationEventState::Published
        );
        assert_eq!(
            repaired.targets()[0].state(),
            RadrootsPhase1PublicationTargetState::AcceptedObserved
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox_phase1_dispatch_intent")
                .fetch_one(outbox.pool())
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox_phase1_target_receipt")
                .fetch_one(outbox.pool())
                .await
                .unwrap(),
            2
        );
    }

    #[tokio::test]
    async fn phase1_publication_reopen_upgrade_and_exact_rollback_are_transactional() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("phase1.sqlite");
        let ready = ready_update();
        let policy =
            RadrootsPhase1PublicationTargetPolicy::new(["wss://relay.example"], 1).unwrap();
        let outbox = RadrootsOutbox::open_file(&path).await.unwrap();
        let receipt = outbox
            .enqueue_phase1_publication(&ready, &policy, 1)
            .await
            .unwrap();
        let publication_id = receipt.record().publication_id();
        outbox.close().await;

        let reopened = RadrootsOutbox::open_file(&path).await.unwrap();
        assert_eq!(
            reopened
                .load_phase1_publication(publication_id)
                .await
                .unwrap()
                .operation_digest(),
            receipt.record().operation_digest()
        );
        reopened.close().await;
        RadrootsOutbox::rollback_file_schema_offline(
            &path,
            1,
            RadrootsOutboxRollbackConfirmation::acknowledge_data_loss(),
        )
        .await
        .unwrap();
        let remigrated = RadrootsOutbox::open_file(&path).await.unwrap();
        assert!(matches!(
            remigrated.load_phase1_publication(publication_id).await,
            Err(RadrootsPhase1PublicationError::PublicationNotFound { .. })
        ));
    }

    #[test]
    fn phase1_publication_diagnostics_enforce_exact_and_one_over() {
        assert!(
            validate_diagnostic(&"a".repeat(RADROOTS_PHASE1_PUBLICATION_DIAGNOSTIC_MAX_BYTES))
                .is_ok()
        );
        assert!(matches!(
            validate_diagnostic(&"a".repeat(RADROOTS_PHASE1_PUBLICATION_DIAGNOSTIC_MAX_BYTES + 1)),
            Err(RadrootsPhase1PublicationError::DiagnosticTooLarge { .. })
        ));
    }
}

#![forbid(unsafe_code)]

use crate::RadrootsOutboxError;
use radroots_event::draft::{RadrootsEventDraft, RadrootsSignedEvent};
use radroots_transport::{
    RadrootsTransportKind, RadrootsTransportMeshScopeId, RadrootsTransportOutcomeKind,
    RadrootsTransportSatisfactionClass, RadrootsTransportSatisfactionPolicy,
    RadrootsTransportTarget, RadrootsTransportTargetFingerprint, RadrootsTransportTargetLabel,
    RadrootsTransportTargetUri,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOutboxOperationStatus {
    Queued,
    Complete,
    DeferredUntilImplemented,
    PreviewUnavailable,
    FailedTerminal,
    Cancelled,
}

impl RadrootsOutboxOperationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Complete => "complete",
            Self::DeferredUntilImplemented => "deferred_until_implemented",
            Self::PreviewUnavailable => "preview_unavailable",
            Self::FailedTerminal => "failed_terminal",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsOutboxError> {
        match value {
            "queued" => Ok(Self::Queued),
            "complete" => Ok(Self::Complete),
            "deferred_until_implemented" => Ok(Self::DeferredUntilImplemented),
            "preview_unavailable" => Ok(Self::PreviewUnavailable),
            "failed_terminal" => Ok(Self::FailedTerminal),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(RadrootsOutboxError::InvalidStoredEnum {
                field: "outbox_operations.status",
                value: value.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOutboxEventState {
    DraftQueued,
    Signing,
    Signed,
    Publishing,
    Published,
    SignRetryable,
    PublishRetryable,
    DeferredUntilImplemented,
    PreviewUnavailable,
    FailedTerminal,
    Cancelled,
}

impl RadrootsOutboxEventState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DraftQueued => "draft_queued",
            Self::Signing => "signing",
            Self::Signed => "signed",
            Self::Publishing => "publishing",
            Self::Published => "published",
            Self::SignRetryable => "sign_retryable",
            Self::PublishRetryable => "publish_retryable",
            Self::DeferredUntilImplemented => "deferred_until_implemented",
            Self::PreviewUnavailable => "preview_unavailable",
            Self::FailedTerminal => "failed_terminal",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsOutboxError> {
        match value {
            "draft_queued" => Ok(Self::DraftQueued),
            "signing" => Ok(Self::Signing),
            "signed" => Ok(Self::Signed),
            "publishing" => Ok(Self::Publishing),
            "published" => Ok(Self::Published),
            "sign_retryable" => Ok(Self::SignRetryable),
            "publish_retryable" => Ok(Self::PublishRetryable),
            "deferred_until_implemented" => Ok(Self::DeferredUntilImplemented),
            "preview_unavailable" => Ok(Self::PreviewUnavailable),
            "failed_terminal" => Ok(Self::FailedTerminal),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(RadrootsOutboxError::InvalidStoredEnum {
                field: "outbox_event.state",
                value: value.to_owned(),
            }),
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Published | Self::FailedTerminal | Self::Cancelled
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOutboxDeliveryPlanStatus {
    Queued,
    Complete,
    DeferredUntilImplemented,
    PreviewUnavailable,
    FailedTerminal,
    Cancelled,
}

impl RadrootsOutboxDeliveryPlanStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Complete => "complete",
            Self::DeferredUntilImplemented => "deferred_until_implemented",
            Self::PreviewUnavailable => "preview_unavailable",
            Self::FailedTerminal => "failed_terminal",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsOutboxError> {
        match value {
            "queued" => Ok(Self::Queued),
            "complete" => Ok(Self::Complete),
            "deferred_until_implemented" => Ok(Self::DeferredUntilImplemented),
            "preview_unavailable" => Ok(Self::PreviewUnavailable),
            "failed_terminal" => Ok(Self::FailedTerminal),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(RadrootsOutboxError::InvalidStoredEnum {
                field: "outbox_delivery_plan.status",
                value: value.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOutboxDeliveryTargetStatus {
    Pending,
    Accepted,
    Delivered,
    Forwarded,
    StoredByGateway,
    Seen,
    DeferredUntilImplemented,
    PreviewUnavailable,
    SkippedPolicyDenied,
    FailedRetryable,
    FailedTerminal,
}

impl RadrootsOutboxDeliveryTargetStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Delivered => "delivered",
            Self::Forwarded => "forwarded",
            Self::StoredByGateway => "stored_by_gateway",
            Self::Seen => "seen",
            Self::DeferredUntilImplemented => "deferred_until_implemented",
            Self::PreviewUnavailable => "preview_unavailable",
            Self::SkippedPolicyDenied => "skipped_policy_denied",
            Self::FailedRetryable => "failed_retryable",
            Self::FailedTerminal => "failed_terminal",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsOutboxError> {
        match value {
            "pending" => Ok(Self::Pending),
            "accepted" => Ok(Self::Accepted),
            "delivered" => Ok(Self::Delivered),
            "forwarded" => Ok(Self::Forwarded),
            "stored_by_gateway" => Ok(Self::StoredByGateway),
            "seen" => Ok(Self::Seen),
            "deferred_until_implemented" => Ok(Self::DeferredUntilImplemented),
            "preview_unavailable" => Ok(Self::PreviewUnavailable),
            "skipped_policy_denied" => Ok(Self::SkippedPolicyDenied),
            "failed_retryable" => Ok(Self::FailedRetryable),
            "failed_terminal" => Ok(Self::FailedTerminal),
            _ => Err(RadrootsOutboxError::InvalidStoredEnum {
                field: "outbox_delivery_target.status",
                value: value.to_owned(),
            }),
        }
    }

    pub fn is_ready_for_attempt(self) -> bool {
        matches!(self, Self::Pending | Self::FailedRetryable)
    }

    pub fn counts_as_transport_satisfaction(
        self,
        satisfaction_class: RadrootsTransportSatisfactionClass,
    ) -> bool {
        match satisfaction_class {
            RadrootsTransportSatisfactionClass::Accepted => matches!(
                self,
                Self::Accepted
                    | Self::Delivered
                    | Self::Forwarded
                    | Self::StoredByGateway
                    | Self::Seen
            ),
            RadrootsTransportSatisfactionClass::Forwarded => {
                matches!(self, Self::Forwarded | Self::Delivered)
            }
            RadrootsTransportSatisfactionClass::Stored => matches!(self, Self::StoredByGateway),
            RadrootsTransportSatisfactionClass::Seen => {
                matches!(self, Self::Seen | Self::Delivered)
            }
            RadrootsTransportSatisfactionClass::Delivered => matches!(self, Self::Delivered),
            RadrootsTransportSatisfactionClass::DurableOrObserved => {
                matches!(self, Self::StoredByGateway | Self::Seen | Self::Delivered)
            }
        }
    }

    pub fn is_deferred_preview(self) -> bool {
        matches!(
            self,
            Self::DeferredUntilImplemented | Self::PreviewUnavailable
        )
    }

    pub fn is_retryable_failure(self) -> bool {
        matches!(self, Self::FailedRetryable)
    }

    pub fn is_terminal_failure(self) -> bool {
        matches!(self, Self::SkippedPolicyDenied | Self::FailedTerminal)
    }

    pub fn is_completed(self) -> bool {
        self.counts_as_transport_satisfaction(RadrootsTransportSatisfactionClass::Accepted)
            || self.is_deferred_preview()
            || self.is_terminal_failure()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RadrootsOutboxReticulumPreviewBehavior {
    #[default]
    RejectDeliveryAttempts,
    DeferDeliveryPlans,
}

impl RadrootsOutboxReticulumPreviewBehavior {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RejectDeliveryAttempts => "reject_delivery_attempts",
            Self::DeferDeliveryPlans => "defer_delivery_plans",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxDeliveryPlanInput {
    pub transport_profile_id: String,
    pub target_policy_version: u32,
    pub satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    pub targets: Vec<RadrootsTransportTarget>,
    pub reticulum_preview_behavior: RadrootsOutboxReticulumPreviewBehavior,
}

impl RadrootsOutboxDeliveryPlanInput {
    pub fn new(
        transport_profile_id: impl Into<String>,
        target_policy_version: u32,
        satisfaction_policy: RadrootsTransportSatisfactionPolicy,
        targets: Vec<RadrootsTransportTarget>,
    ) -> Self {
        Self {
            transport_profile_id: transport_profile_id.into(),
            target_policy_version,
            satisfaction_policy,
            targets,
            reticulum_preview_behavior: RadrootsOutboxReticulumPreviewBehavior::default(),
        }
    }

    pub fn with_reticulum_preview_behavior(
        mut self,
        behavior: RadrootsOutboxReticulumPreviewBehavior,
    ) -> Self {
        self.reticulum_preview_behavior = behavior;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxOperationInput {
    pub operation_kind: String,
    pub draft: RadrootsEventDraft,
    pub delivery_plan: RadrootsOutboxDeliveryPlanInput,
    pub idempotency_key: Option<String>,
    pub created_at_ms: i64,
}

impl RadrootsOutboxOperationInput {
    pub fn new(
        operation_kind: impl Into<String>,
        draft: RadrootsEventDraft,
        delivery_plan: RadrootsOutboxDeliveryPlanInput,
        created_at_ms: i64,
    ) -> Self {
        Self {
            operation_kind: operation_kind.into(),
            draft,
            delivery_plan,
            idempotency_key: None,
            created_at_ms,
        }
    }

    pub fn with_idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxSignedOperationInput {
    pub operation_kind: String,
    pub draft: RadrootsEventDraft,
    pub signed_event: RadrootsSignedEvent,
    pub delivery_plan: RadrootsOutboxDeliveryPlanInput,
    pub idempotency_key: Option<String>,
    pub event_store_inserted: bool,
    pub event_store_ingested_at_ms: i64,
    pub created_at_ms: i64,
}

impl RadrootsOutboxSignedOperationInput {
    pub fn new(
        operation_kind: impl Into<String>,
        draft: RadrootsEventDraft,
        signed_event: RadrootsSignedEvent,
        delivery_plan: RadrootsOutboxDeliveryPlanInput,
        event_store_inserted: bool,
        event_store_ingested_at_ms: i64,
        created_at_ms: i64,
    ) -> Self {
        Self {
            operation_kind: operation_kind.into(),
            draft,
            signed_event,
            delivery_plan,
            idempotency_key: None,
            event_store_inserted,
            event_store_ingested_at_ms,
            created_at_ms,
        }
    }

    pub fn with_idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOutboxEnqueueStatus {
    Inserted,
    Existing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxEnqueueReceipt {
    pub status: RadrootsOutboxEnqueueStatus,
    pub operation_id: i64,
    pub outbox_event_id: i64,
    pub delivery_plan_id: i64,
    pub expected_event_id: String,
    pub operation_idempotency_digest: String,
    pub delivery_plan_idempotency_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxIdempotencyPreflight {
    pub operation_idempotency_digest: String,
    pub delivery_plan_idempotency_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxOperationRecord {
    pub operation_id: i64,
    pub operation_kind: String,
    pub expected_pubkey: String,
    pub idempotency_key: Option<String>,
    pub operation_idempotency_digest: String,
    pub status: RadrootsOutboxOperationStatus,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxEventRecord {
    pub outbox_event_id: i64,
    pub operation_id: i64,
    pub event_id: String,
    pub expected_pubkey: String,
    pub draft: RadrootsEventDraft,
    pub signed_event: Option<RadrootsSignedEvent>,
    pub raw_event_json: Option<String>,
    pub state: RadrootsOutboxEventState,
    pub attempt_count: i64,
    pub claim_token: Option<String>,
    pub claim_owner: Option<String>,
    pub claim_expires_at_ms: Option<i64>,
    pub active_delivery_plan_id: Option<i64>,
    pub next_attempt_after_ms: i64,
    pub last_error: Option<String>,
    pub event_store_ingested: bool,
    pub event_store_inserted: bool,
    pub event_store_ingested_at_ms: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxDeliveryPlanRecord {
    pub delivery_plan_id: i64,
    pub outbox_event_id: i64,
    pub transport_profile_id: String,
    pub target_policy_fingerprint: String,
    pub target_policy_version: u32,
    pub satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    pub required_success_count: i64,
    pub delivery_plan_idempotency_digest: String,
    pub status: RadrootsOutboxDeliveryPlanStatus,
    pub satisfied_at_ms: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxDeliveryTargetRecord {
    pub delivery_target_id: i64,
    pub delivery_plan_id: i64,
    pub transport_kind: RadrootsTransportKind,
    pub endpoint_uri: RadrootsTransportTargetUri,
    pub target_scope: Option<RadrootsTransportMeshScopeId>,
    pub target_label: Option<RadrootsTransportTargetLabel>,
    pub endpoint_fingerprint: RadrootsTransportTargetFingerprint,
    pub status: RadrootsOutboxDeliveryTargetStatus,
    pub last_outcome_kind: Option<RadrootsTransportOutcomeKind>,
    pub attempt_count: i64,
    pub last_attempt_at_ms: Option<i64>,
    pub completed_at_ms: Option<i64>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxReticulumPreviewEventRecord {
    pub event: RadrootsOutboxEventRecord,
    pub targets: Vec<RadrootsOutboxDeliveryTargetRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxDeliveryAttemptRecord {
    pub delivery_attempt_id: i64,
    pub delivery_plan_id: i64,
    pub delivery_target_id: i64,
    pub status: RadrootsOutboxDeliveryTargetStatus,
    pub outcome_kind: RadrootsTransportOutcomeKind,
    pub attempted_at_ms: i64,
    pub message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxClaimedEvent {
    pub outbox_event_id: i64,
    pub operation_id: i64,
    pub expected_event_id: String,
    pub attempt_count: i64,
    pub state: RadrootsOutboxEventState,
    pub claim_token: String,
    pub active_delivery_plan_id: Option<i64>,
    pub draft: RadrootsEventDraft,
    pub signed_event: Option<RadrootsSignedEvent>,
    pub delivery_targets: Vec<RadrootsOutboxDeliveryTargetRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxEventStoreIngestReceipt {
    pub outbox_event_id: i64,
    pub event_id: String,
    pub already_ingested: bool,
    pub event_store_inserted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxStatusSummary {
    pub total_events: i64,
    pub pending_events: i64,
    pub retryable_events: i64,
    pub terminal_events: i64,
    pub failed_terminal_events: i64,
    pub preview_unavailable_events: i64,
    pub deferred_until_implemented_events: i64,
    pub ready_signed_events: i64,
    pub publishing_events: i64,
    pub last_attempt_at_ms: Option<i64>,
    pub last_error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_enums_round_trip_target_state_values() {
        for (status, expected) in [
            (RadrootsOutboxOperationStatus::Queued, "queued"),
            (RadrootsOutboxOperationStatus::Complete, "complete"),
            (
                RadrootsOutboxOperationStatus::DeferredUntilImplemented,
                "deferred_until_implemented",
            ),
            (
                RadrootsOutboxOperationStatus::PreviewUnavailable,
                "preview_unavailable",
            ),
            (
                RadrootsOutboxOperationStatus::FailedTerminal,
                "failed_terminal",
            ),
            (RadrootsOutboxOperationStatus::Cancelled, "cancelled"),
        ] {
            assert_eq!(status.as_str(), expected);
            assert_eq!(
                RadrootsOutboxOperationStatus::parse(expected).expect("status"),
                status
            );
        }

        for (state, expected) in [
            (RadrootsOutboxEventState::DraftQueued, "draft_queued"),
            (RadrootsOutboxEventState::Signing, "signing"),
            (RadrootsOutboxEventState::Signed, "signed"),
            (RadrootsOutboxEventState::Publishing, "publishing"),
            (RadrootsOutboxEventState::Published, "published"),
            (RadrootsOutboxEventState::SignRetryable, "sign_retryable"),
            (
                RadrootsOutboxEventState::PublishRetryable,
                "publish_retryable",
            ),
            (
                RadrootsOutboxEventState::DeferredUntilImplemented,
                "deferred_until_implemented",
            ),
            (
                RadrootsOutboxEventState::PreviewUnavailable,
                "preview_unavailable",
            ),
            (RadrootsOutboxEventState::FailedTerminal, "failed_terminal"),
            (RadrootsOutboxEventState::Cancelled, "cancelled"),
        ] {
            assert_eq!(state.as_str(), expected);
            assert_eq!(
                RadrootsOutboxEventState::parse(expected).expect("event state"),
                state
            );
        }

        for (status, expected) in [
            (RadrootsOutboxDeliveryPlanStatus::Queued, "queued"),
            (RadrootsOutboxDeliveryPlanStatus::Complete, "complete"),
            (
                RadrootsOutboxDeliveryPlanStatus::DeferredUntilImplemented,
                "deferred_until_implemented",
            ),
            (
                RadrootsOutboxDeliveryPlanStatus::PreviewUnavailable,
                "preview_unavailable",
            ),
            (
                RadrootsOutboxDeliveryPlanStatus::FailedTerminal,
                "failed_terminal",
            ),
            (RadrootsOutboxDeliveryPlanStatus::Cancelled, "cancelled"),
        ] {
            assert_eq!(status.as_str(), expected);
            assert_eq!(
                RadrootsOutboxDeliveryPlanStatus::parse(expected).expect("plan status"),
                status
            );
        }

        for (behavior, expected) in [
            (
                RadrootsOutboxReticulumPreviewBehavior::RejectDeliveryAttempts,
                "reject_delivery_attempts",
            ),
            (
                RadrootsOutboxReticulumPreviewBehavior::DeferDeliveryPlans,
                "defer_delivery_plans",
            ),
        ] {
            assert_eq!(behavior.as_str(), expected);
        }
        assert_eq!(
            RadrootsOutboxReticulumPreviewBehavior::default(),
            RadrootsOutboxReticulumPreviewBehavior::RejectDeliveryAttempts
        );

        for (status, expected, ready, satisfied, delivered, deferred_preview, terminal_failure) in [
            (
                RadrootsOutboxDeliveryTargetStatus::Pending,
                "pending",
                true,
                false,
                false,
                false,
                false,
            ),
            (
                RadrootsOutboxDeliveryTargetStatus::Accepted,
                "accepted",
                false,
                true,
                false,
                false,
                false,
            ),
            (
                RadrootsOutboxDeliveryTargetStatus::Delivered,
                "delivered",
                false,
                true,
                true,
                false,
                false,
            ),
            (
                RadrootsOutboxDeliveryTargetStatus::Forwarded,
                "forwarded",
                false,
                true,
                false,
                false,
                false,
            ),
            (
                RadrootsOutboxDeliveryTargetStatus::StoredByGateway,
                "stored_by_gateway",
                false,
                true,
                false,
                false,
                false,
            ),
            (
                RadrootsOutboxDeliveryTargetStatus::Seen,
                "seen",
                false,
                true,
                false,
                false,
                false,
            ),
            (
                RadrootsOutboxDeliveryTargetStatus::DeferredUntilImplemented,
                "deferred_until_implemented",
                false,
                false,
                false,
                true,
                false,
            ),
            (
                RadrootsOutboxDeliveryTargetStatus::PreviewUnavailable,
                "preview_unavailable",
                false,
                false,
                false,
                true,
                false,
            ),
            (
                RadrootsOutboxDeliveryTargetStatus::SkippedPolicyDenied,
                "skipped_policy_denied",
                false,
                false,
                false,
                false,
                true,
            ),
            (
                RadrootsOutboxDeliveryTargetStatus::FailedRetryable,
                "failed_retryable",
                true,
                false,
                false,
                false,
                false,
            ),
            (
                RadrootsOutboxDeliveryTargetStatus::FailedTerminal,
                "failed_terminal",
                false,
                false,
                false,
                false,
                true,
            ),
        ] {
            assert_eq!(status.as_str(), expected);
            assert_eq!(
                RadrootsOutboxDeliveryTargetStatus::parse(expected).expect("target status"),
                status
            );
            assert_eq!(status.is_ready_for_attempt(), ready);
            assert_eq!(
                status.counts_as_transport_satisfaction(
                    radroots_transport::RadrootsTransportSatisfactionClass::Accepted
                ),
                satisfied
            );
            assert_eq!(
                status.counts_as_transport_satisfaction(
                    radroots_transport::RadrootsTransportSatisfactionClass::Delivered
                ),
                delivered
            );
            assert_eq!(status.is_deferred_preview(), deferred_preview);
            assert_eq!(status.is_terminal_failure(), terminal_failure);
        }
    }
}

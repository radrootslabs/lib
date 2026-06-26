#![forbid(unsafe_code)]

use crate::RadrootsOutboxError;
use radroots_events::draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOutboxOperationStatus {
    Queued,
    Complete,
    FailedTerminal,
    Cancelled,
}

impl RadrootsOutboxOperationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Complete => "complete",
            Self::FailedTerminal => "failed_terminal",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsOutboxError> {
        match value {
            "queued" => Ok(Self::Queued),
            "complete" => Ok(Self::Complete),
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
pub enum RadrootsOutboxRelayStatus {
    Pending,
    Accepted,
    FailedRetryable,
    FailedTerminal,
}

impl RadrootsOutboxRelayStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::FailedRetryable => "failed_retryable",
            Self::FailedTerminal => "failed_terminal",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsOutboxError> {
        match value {
            "pending" => Ok(Self::Pending),
            "accepted" => Ok(Self::Accepted),
            "failed_retryable" => Ok(Self::FailedRetryable),
            "failed_terminal" => Ok(Self::FailedTerminal),
            _ => Err(RadrootsOutboxError::InvalidStoredEnum {
                field: "outbox_event_relay_status.status",
                value: value.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxOperationInput {
    pub operation_kind: String,
    pub draft: RadrootsFrozenEventDraft,
    pub target_relays: Vec<String>,
    pub idempotency_key: Option<String>,
    pub allow_empty_target_relays: bool,
    pub created_at_ms: i64,
}

impl RadrootsOutboxOperationInput {
    pub fn new(
        operation_kind: impl Into<String>,
        draft: RadrootsFrozenEventDraft,
        target_relays: Vec<String>,
        created_at_ms: i64,
    ) -> Self {
        Self {
            operation_kind: operation_kind.into(),
            draft,
            target_relays,
            idempotency_key: None,
            allow_empty_target_relays: false,
            created_at_ms,
        }
    }

    pub fn with_idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
        self
    }

    pub fn allow_empty_target_relays(mut self) -> Self {
        self.allow_empty_target_relays = true;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxSignedOperationInput {
    pub operation_kind: String,
    pub draft: RadrootsFrozenEventDraft,
    pub signed_event: RadrootsSignedNostrEvent,
    pub target_relays: Vec<String>,
    pub idempotency_key: Option<String>,
    pub allow_empty_target_relays: bool,
    pub event_store_inserted: bool,
    pub event_store_ingested_at_ms: i64,
    pub created_at_ms: i64,
}

impl RadrootsOutboxSignedOperationInput {
    pub fn new(
        operation_kind: impl Into<String>,
        draft: RadrootsFrozenEventDraft,
        signed_event: RadrootsSignedNostrEvent,
        target_relays: Vec<String>,
        event_store_inserted: bool,
        event_store_ingested_at_ms: i64,
        created_at_ms: i64,
    ) -> Self {
        Self {
            operation_kind: operation_kind.into(),
            draft,
            signed_event,
            target_relays,
            idempotency_key: None,
            allow_empty_target_relays: false,
            event_store_inserted,
            event_store_ingested_at_ms,
            created_at_ms,
        }
    }

    pub fn with_idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
        self
    }

    pub fn allow_empty_target_relays(mut self) -> Self {
        self.allow_empty_target_relays = true;
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
    pub expected_event_id: String,
    pub idempotency_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxOperationRecord {
    pub operation_id: i64,
    pub operation_kind: String,
    pub expected_pubkey: String,
    pub idempotency_key: Option<String>,
    pub idempotency_digest: String,
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
    pub draft: RadrootsFrozenEventDraft,
    pub signed_event: Option<RadrootsSignedNostrEvent>,
    pub raw_event_json: Option<String>,
    pub state: RadrootsOutboxEventState,
    pub accepted_quorum: i64,
    pub attempt_count: i64,
    pub claim_token: Option<String>,
    pub claim_owner: Option<String>,
    pub claim_expires_at_ms: Option<i64>,
    pub next_attempt_after_ms: i64,
    pub last_error: Option<String>,
    pub event_store_ingested: bool,
    pub event_store_inserted: bool,
    pub event_store_ingested_at_ms: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxRelayStatusRecord {
    pub outbox_event_id: i64,
    pub relay_url: String,
    pub status: RadrootsOutboxRelayStatus,
    pub attempt_count: i64,
    pub last_attempt_at_ms: Option<i64>,
    pub acknowledged_at_ms: Option<i64>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxClaimedEvent {
    pub outbox_event_id: i64,
    pub operation_id: i64,
    pub expected_event_id: String,
    pub attempt_count: i64,
    pub state: RadrootsOutboxEventState,
    pub claim_token: String,
    pub draft: RadrootsFrozenEventDraft,
    pub signed_event: Option<RadrootsSignedNostrEvent>,
    pub target_relays: Vec<String>,
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
    pub ready_signed_events: i64,
    pub publishing_events: i64,
    pub last_attempt_at_ms: Option<i64>,
    pub last_error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_event_and_relay_status_values_round_trip() {
        for (status, expected) in [
            (RadrootsOutboxOperationStatus::Queued, "queued"),
            (RadrootsOutboxOperationStatus::Complete, "complete"),
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
        assert!(RadrootsOutboxOperationStatus::parse("bad").is_err());

        for (state, expected, terminal) in [
            (RadrootsOutboxEventState::DraftQueued, "draft_queued", false),
            (RadrootsOutboxEventState::Signing, "signing", false),
            (RadrootsOutboxEventState::Signed, "signed", false),
            (RadrootsOutboxEventState::Publishing, "publishing", false),
            (RadrootsOutboxEventState::Published, "published", true),
            (
                RadrootsOutboxEventState::SignRetryable,
                "sign_retryable",
                false,
            ),
            (
                RadrootsOutboxEventState::PublishRetryable,
                "publish_retryable",
                false,
            ),
            (
                RadrootsOutboxEventState::FailedTerminal,
                "failed_terminal",
                true,
            ),
            (RadrootsOutboxEventState::Cancelled, "cancelled", true),
        ] {
            assert_eq!(state.as_str(), expected);
            assert_eq!(
                RadrootsOutboxEventState::parse(expected).expect("state"),
                state
            );
            assert_eq!(state.is_terminal(), terminal);
        }
        assert!(RadrootsOutboxEventState::parse("bad").is_err());

        for (status, expected) in [
            (RadrootsOutboxRelayStatus::Pending, "pending"),
            (RadrootsOutboxRelayStatus::Accepted, "accepted"),
            (
                RadrootsOutboxRelayStatus::FailedRetryable,
                "failed_retryable",
            ),
            (RadrootsOutboxRelayStatus::FailedTerminal, "failed_terminal"),
        ] {
            assert_eq!(status.as_str(), expected);
            assert_eq!(
                RadrootsOutboxRelayStatus::parse(expected).expect("relay status"),
                status
            );
        }
        assert!(RadrootsOutboxRelayStatus::parse("bad").is_err());
    }
}

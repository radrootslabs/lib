#![forbid(unsafe_code)]

use crate::RadrootsOutboxError;
use radroots_events::draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOutboxOperationStatus {
    Queued,
    Complete,
}

impl RadrootsOutboxOperationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Complete => "complete",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsOutboxError> {
        match value {
            "queued" => Ok(Self::Queued),
            "complete" => Ok(Self::Complete),
            _ => Err(RadrootsOutboxError::InvalidStoredEnum {
                field: "outbox_operation.status",
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
            _ => Err(RadrootsOutboxError::InvalidStoredEnum {
                field: "outbox_event.state",
                value: value.to_owned(),
            }),
        }
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

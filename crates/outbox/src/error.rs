#![forbid(unsafe_code)]

use radroots_transport::RadrootsTransportError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadrootsOutboxError {
    #[error("SQLx error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Event store error: {0}")]
    EventStore(#[from] radroots_event_store::RadrootsEventStoreError),

    #[error("Signed event does not match frozen draft: {0}")]
    SignedEventDraftMismatch(#[from] radroots_events::draft::RadrootsDraftError),

    #[error("delivery targets cannot be empty")]
    EmptyDeliveryTargets,

    #[error("transport profile id cannot be empty")]
    EmptyTransportProfileId,

    #[error("transport contract error: {0}")]
    Transport(RadrootsTransportError),

    #[error("Invalid stored enum for {field}: {value}")]
    InvalidStoredEnum { field: &'static str, value: String },

    #[error("stored integer for {field} is outside the supported range: {value}")]
    IntegerRange { field: &'static str, value: i64 },

    #[error("Idempotency conflict for {operation_kind}/{expected_pubkey}/{idempotency_key}")]
    IdempotencyConflict {
        operation_kind: String,
        expected_pubkey: String,
        idempotency_key: String,
        existing_digest: String,
        new_digest: String,
    },

    #[error("Outbox event not found: {0}")]
    EventNotFound(i64),

    #[error("Outbox delivery target not found: {0}")]
    DeliveryTargetNotFound(i64),

    #[error(
        "Outbox delivery target {delivery_target_id} already completed as {current_status}; requested {requested_status}"
    )]
    DeliveryTargetStatusConflict {
        delivery_target_id: i64,
        current_status: &'static str,
        requested_status: &'static str,
    },

    #[error("Claim token mismatch for outbox event {outbox_event_id}")]
    ClaimTokenMismatch { outbox_event_id: i64 },

    #[error("Outbox event {outbox_event_id} has no active delivery plan")]
    MissingActiveDeliveryPlan { outbox_event_id: i64 },

    #[error("Signed event missing for outbox event {0}")]
    MissingSignedEvent(i64),

    #[error("Signed event ID mismatch: expected {expected_event_id}, got {actual_event_id}")]
    SignedEventIdMismatch {
        expected_event_id: String,
        actual_event_id: String,
    },
}

impl From<RadrootsTransportError> for RadrootsOutboxError {
    fn from(value: RadrootsTransportError) -> Self {
        Self::Transport(value)
    }
}

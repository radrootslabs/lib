#![forbid(unsafe_code)]

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadrootsOutboxError {
    #[error("SQLx error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Nostr error: {0}")]
    Nostr(#[from] radroots_nostr::prelude::RadrootsNostrError),

    #[error("Event store error: {0}")]
    EventStore(#[from] radroots_event_store::RadrootsEventStoreError),

    #[error("Invalid stored enum for {field}: {value}")]
    InvalidStoredEnum { field: &'static str, value: String },

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

    #[error("Claim token mismatch for outbox event {outbox_event_id}")]
    ClaimTokenMismatch { outbox_event_id: i64 },

    #[error("Signed event missing for outbox event {0}")]
    MissingSignedEvent(i64),

    #[error("Signed event ID mismatch: expected {expected_event_id}, got {actual_event_id}")]
    SignedEventIdMismatch {
        expected_event_id: String,
        actual_event_id: String,
    },
}

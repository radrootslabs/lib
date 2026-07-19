use radroots_event::draft::RadrootsSignedEventError;
use radroots_event::ids::RadrootsIdParseError;
use radroots_event::wire::RadrootsEventWireError;
use radroots_event_codec::verification::RadrootsNip01VerificationError;
use radroots_transport::RadrootsTransportError;

#[derive(Debug, thiserror::Error)]
pub enum RadrootsEventStoreError {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("identifier parse error: {0}")]
    IdParse(#[from] RadrootsIdParseError),
    #[error("event wire error: {0}")]
    EventWire(#[from] RadrootsEventWireError),
    #[error("signed event error: {0}")]
    SignedEvent(#[from] RadrootsSignedEventError),
    #[error("NIP-01 verification error: {0}")]
    Nip01Verification(#[from] RadrootsNip01VerificationError),
    #[error("transport contract error: {0}")]
    Transport(RadrootsTransportError),
    #[error("stored event `{0}` was not found")]
    MissingEvent(String),
    #[error("event-store tag query tag name cannot be empty")]
    EmptyTagName,
    #[error("event-store contract tag query contract list cannot be empty")]
    EmptyContractList,
    #[error("event-store contract list length {actual} exceeds {max}")]
    ContractListTooLarge { max: usize, actual: usize },
    #[error("event-store query limit {actual} is outside {min}..={max}")]
    QueryLimitOutOfRange { min: u32, max: u32, actual: u32 },
    #[error(
        "an in-memory event-store pool must have exactly one connection, configured maximum was {actual}"
    )]
    UnsafeInMemoryPoolConnectionCount { actual: u32 },
    #[error(
        "event-store pool backing mismatch: file_backed={file_backed}, configured filename `{filename}`"
    )]
    SqlitePoolBackingMismatch { file_backed: bool, filename: String },
    #[error("invalid stored enum value `{value}` for {field}")]
    InvalidStoredEnum { field: &'static str, value: String },
    #[error("invalid stored boolean value `{value}` for {field}; expected 0 or 1")]
    InvalidStoredBoolean { field: &'static str, value: i64 },
    #[error("stored raw event `{event_id}` is not signature verified: `{status}`")]
    StoredRawEventNotVerified { event_id: String, status: String },
    #[error(
        "stored raw event `{event_id}` uses pre-admission status `{contract_status}` and must be reconciled"
    )]
    StoredRawEventRequiresReconciliation {
        event_id: String,
        contract_status: String,
    },
    #[error("stored raw event `{event_id}` is missing its numeric NIP-01 event class")]
    StoredRawEventMissingClass { event_id: String },
    #[error("stored raw event `{event_id}` has an inconsistent admission classification")]
    StoredRawEventClassificationInconsistent { event_id: String },
    #[error("stored event `{event_id}` does not have a raw event-head coordinate")]
    StoredHeadCoordinateUnavailable { event_id: String },
    #[error("stored raw event head referencing `{event_id}` is inconsistent with its event")]
    StoredHeadInconsistent { event_id: String },
    #[error("projection `{projection_id}` version mismatch: expected {expected}, stored {actual}")]
    ProjectionVersionMismatch {
        projection_id: String,
        expected: u32,
        actual: u32,
    },
    #[error(
        "projection `{projection_id}` cursor compare-and-swap conflict: expected prior sequence {expected:?}, stored {actual:?}"
    )]
    ProjectionCursorConflict {
        projection_id: String,
        expected: Option<i64>,
        actual: Option<i64>,
    },
    #[error(
        "projection `{projection_id}` cursor cannot move backward from {current} to {proposed}"
    )]
    ProjectionCursorRegression {
        projection_id: String,
        current: i64,
        proposed: i64,
    },
    #[error("projection `{projection_id}` cursor sequence cannot be negative: {value}")]
    InvalidProjectionCursor { projection_id: String, value: i64 },
    #[error(
        "stored transport observation fingerprint `{endpoint_fingerprint}` does not match `{transport_kind}` endpoint `{endpoint_uri}` for event `{event_id}`"
    )]
    InvalidStoredTransportEndpointFingerprint {
        event_id: String,
        transport_kind: String,
        endpoint_uri: String,
        endpoint_fingerprint: String,
    },
    #[error(
        "stored transport observation for event `{event_id}` has invalid times/count: first={first_observed_at_ms}, last={last_observed_at_ms}, count={observation_count}"
    )]
    InvalidStoredTransportObservation {
        event_id: String,
        first_observed_at_ms: i64,
        last_observed_at_ms: i64,
        observation_count: i64,
    },
    #[error("integer value `{value}` is outside {field} range")]
    IntegerRange { field: &'static str, value: i64 },
    #[error("unsigned integer value `{value}` is outside {field} range")]
    UnsignedIntegerRange { field: &'static str, value: u64 },
}

impl From<RadrootsTransportError> for RadrootsEventStoreError {
    fn from(value: RadrootsTransportError) -> Self {
        Self::Transport(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn transport_errors_preserve_their_typed_source() {
        let error = RadrootsEventStoreError::from(RadrootsTransportError::InvalidTargetUri);

        assert!(matches!(
            error,
            RadrootsEventStoreError::Transport(RadrootsTransportError::InvalidTargetUri)
        ));
    }
}

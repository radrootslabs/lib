use radroots_event::contract::RadrootsContractMatchError;
use radroots_event::draft::RadrootsSignedEventError;
use radroots_event::event_head::RadrootsEventHeadMalformed;
use radroots_event::ids::RadrootsIdParseError;
use radroots_event::wire::RadrootsEventWireError;
use radroots_transport::RadrootsTransportError;

#[derive(Debug, thiserror::Error)]
pub enum RadrootsEventStoreError {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("contract match error: {0:?}")]
    ContractMatch(RadrootsContractMatchError),
    #[error("event-head malformed: {0:?}")]
    EventHeadMalformed(RadrootsEventHeadMalformed),
    #[error("identifier parse error: {0}")]
    IdParse(#[from] RadrootsIdParseError),
    #[error("event wire error: {0}")]
    EventWire(#[from] RadrootsEventWireError),
    #[error("signed event error: {0}")]
    SignedEvent(#[from] RadrootsSignedEventError),
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
    #[error("invalid stored enum value `{value}` for {field}")]
    InvalidStoredEnum { field: &'static str, value: String },
    #[error(
        "stored transport observation fingerprint `{endpoint_fingerprint}` does not match `{transport_kind}` endpoint `{endpoint_uri}` for event `{event_id}`"
    )]
    InvalidStoredTransportEndpointFingerprint {
        event_id: String,
        transport_kind: String,
        endpoint_uri: String,
        endpoint_fingerprint: String,
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

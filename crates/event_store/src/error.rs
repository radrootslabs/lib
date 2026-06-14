use radroots_events::contract::RadrootsContractMatchError;
use radroots_events::event_head::RadrootsEventHeadMalformed;
use radroots_events::ids::RadrootsIdParseError;

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
    #[error("integer value `{value}` is outside {field} range")]
    IntegerRange { field: &'static str, value: i64 },
}

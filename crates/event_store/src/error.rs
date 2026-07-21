use radroots_event::draft::RadrootsSignedEventError;
use radroots_event::ids::RadrootsIdParseError;
use radroots_event::wire::RadrootsEventWireError;
use radroots_event_codec::verification::RadrootsNip01VerificationError;
use radroots_transport::RadrootsTransportError;

/// Maximum retained raw event rows in one event store.
pub const RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1: u64 = 25_000;
/// Maximum retained raw tag rows in one event store.
pub const RADROOTS_EVENT_STORE_RAW_TAG_COUNT_LIMIT_V1: u64 = 250_000;
/// Maximum governed UTF-8 bytes in retained raw event text columns.
pub const RADROOTS_EVENT_STORE_RAW_EVENT_TEXT_BYTES_LIMIT_V1: u64 = 64 * 1024 * 1024;
/// Maximum governed UTF-8 bytes in retained raw tag text columns.
pub const RADROOTS_EVENT_STORE_RAW_TAG_TEXT_BYTES_LIMIT_V1: u64 = 32 * 1024 * 1024;
/// Maximum append-only source generations retained before fresh-store resync.
pub const RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1: u32 = 8;

/// Governed retained raw-source resource dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RadrootsEventStoreSourceCapacityResourceV1 {
    /// Number of retained raw-source event rows.
    RawEvents,
    /// Number of retained raw-source tag rows.
    RawTags,
    /// Total UTF-8 bytes across retained text fields in raw-source event rows.
    RawEventBytes,
    /// Total UTF-8 bytes across retained text fields in raw-source tag rows.
    RawTagBytes,
}

impl RadrootsEventStoreSourceCapacityResourceV1 {
    /// Stable diagnostic label used by the typed capacity error.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RawEvents => "raw event count",
            Self::RawTags => "raw tag count",
            Self::RawEventBytes => "total retained raw-source event row text bytes",
            Self::RawTagBytes => "total retained raw-source tag row text bytes",
        }
    }
}

impl core::fmt::Display for RadrootsEventStoreSourceCapacityResourceV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

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
    #[error("addressable transition scope cannot be empty")]
    AddressableTransitionScopeEmpty,
    #[error("addressable transition scope length {actual} exceeds {max}")]
    AddressableTransitionScopeTooLarge { max: usize, actual: usize },
    #[error("addressable transition scope kind {kind} is outside 30000..=39999")]
    AddressableTransitionScopeKindInvalid { kind: u32 },
    #[error("addressable transition cursor sequence cannot be negative: {value}")]
    AddressableTransitionCursorNegative { value: i64 },
    #[error("addressable transition cursor JSON is {actual} bytes; maximum is {max}")]
    AddressableTransitionCursorTooLarge { max: usize, actual: usize },
    #[error("addressable transition cursor field `{field}` is not canonical lowercase 32-byte hex")]
    AddressableTransitionCursorEncoding { field: &'static str },
    #[error(
        "addressable transition cursor feed version mismatch: expected {expected}, found {actual}"
    )]
    AddressableTransitionFeedVersionMismatch { expected: u32, actual: u32 },
    #[error("addressable transition cursor scope fingerprint does not match the requested scope")]
    AddressableTransitionScopeMismatch,
    #[error("addressable transition cursor source generation is no longer active")]
    AddressableTransitionSourceGenerationMismatch,
    #[error("addressable transition cursor {cursor} precedes the active generation floor {floor}")]
    AddressableTransitionCursorExpired { cursor: i64, floor: i64 },
    #[error("addressable transition cursor {cursor} is ahead of source high-water {high_water}")]
    AddressableTransitionCursorAhead { cursor: i64, high_water: i64 },
    #[error("addressable transition feed sequence interval has a gap: {reason}")]
    AddressableTransitionSequenceGap { reason: String },
    #[error("addressable transition feed contains corrupt authority: {reason}")]
    AddressableTransitionCorruption { reason: String },
    #[error("addressable transition canonical payload is {actual} bytes; page maximum is {max}")]
    AddressableTransitionPagePayloadTooLarge { max: usize, actual: usize },
    #[error("event-store current-visibility authority is inconsistent: {reason}")]
    CurrentVisibilityDrift { reason: String },
    #[error("FoodAvailability projection authority is inconsistent: {reason}")]
    FoodAvailabilityProjectionDrift { reason: String },
    #[error("FoodAvailability search query must not be empty")]
    FoodAvailabilitySearchEmpty,
    #[error("FoodAvailability search query is {actual} bytes; maximum is {max}")]
    FoodAvailabilitySearchTooLarge { max: usize, actual: usize },
    #[error("FoodAvailability search query has {actual} terms; maximum is {max}")]
    FoodAvailabilitySearchTooManyTerms { max: usize, actual: usize },
    #[error("event-store query limit {actual} is outside {min}..={max}")]
    QueryLimitOutOfRange { min: u32, max: u32, actual: u32 },
    #[error("event visibility batch contains more than {max} event ids")]
    EventVisibilityBatchTooLarge { max: usize },
    #[error(
        "an in-memory event-store pool must have exactly one connection, configured maximum was {actual}"
    )]
    UnsafeInMemoryPoolConnectionCount { actual: u32 },
    #[error(
        "event-store pool backing mismatch: file_backed={file_backed}, configured filename `{filename}`"
    )]
    SqlitePoolBackingMismatch { file_backed: bool, filename: String },
    #[error("event-store SQLite connection has no main database")]
    SqliteMainDatabaseUnavailable,
    #[error("event-store SQLite main database must use UTF-8 encoding; reported `{actual}`")]
    SqliteMainDatabaseEncodingNotUtf8 { actual: String },
    #[error(
        "event-store SQLite file connection did not enter WAL journal mode; reported `{actual}`"
    )]
    SqliteFileJournalModeNotWal { actual: String },
    #[error(
        "temporary schema object `{name}` ({object_type}, table `{table_name}`) collides with event-store authority"
    )]
    TemporarySchemaCollision {
        object_type: String,
        name: String,
        table_name: String,
    },
    #[error("event-store migration registry defect: {reason}")]
    MigrationRegistryDefect { reason: String },
    #[error(
        "embedded event-store migration {version} {direction} length mismatch: expected {expected}, found {actual}"
    )]
    EmbeddedMigrationLengthMismatch {
        version: u32,
        direction: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error(
        "embedded event-store migration {version} {direction} checksum mismatch: expected {expected}, found {actual}"
    )]
    EmbeddedMigrationChecksumMismatch {
        version: u32,
        direction: &'static str,
        expected: &'static str,
        actual: String,
    },
    #[error("event-store migration {version} {direction} catalog delta mismatch: {reason}")]
    MigrationCatalogDeltaMismatch {
        version: u32,
        direction: &'static str,
        reason: String,
    },
    #[error("unmanaged event-store schema has fingerprint {actual_schema_sha256}")]
    UnmanagedSchema { actual_schema_sha256: String },
    #[error("event-store migration ledger catalog is invalid: {reason}")]
    MigrationLedgerDrift { reason: String },
    #[error("event-store migration history gap: expected version {expected}, found {actual:?}")]
    MigrationHistoryGap { expected: u32, actual: Option<u32> },
    #[error("event-store migration history references unknown version {version}")]
    UnknownMigration { version: u32 },
    #[error("event-store schema version {database} is newer than supported version {current}")]
    SchemaTooNew { current: u32, database: i64 },
    #[error("event-store migration {version} name drift: expected `{expected}`, found `{actual}`")]
    MigrationHistoryNameDrift {
        version: u32,
        expected: &'static str,
        actual: String,
    },
    #[error(
        "event-store migration {version} {field} checksum drift: expected {expected}, found {actual}"
    )]
    MigrationHistoryChecksumDrift {
        version: u32,
        field: &'static str,
        expected: &'static str,
        actual: String,
    },
    #[error(
        "event-store schema fingerprint mismatch at version {version}: expected {expected}, found {actual}"
    )]
    SchemaFingerprintMismatch {
        version: u32,
        expected: &'static str,
        actual: String,
    },
    #[error("event-store rollback target {target} is below the supported version floor {floor}")]
    RollbackBelowVersionFloor { floor: u32, target: u32 },
    #[error("event-store rollback target {target} is ahead of managed version {current}")]
    RollbackAhead { current: u32, target: u32 },
    #[error(
        "event-store rollback from version {current} to {target} would discard retained source-generation history; minimum retained-history schema version is {floor}"
    )]
    RollbackWouldDiscardSourceGenerationHistory {
        current: u32,
        target: u32,
        floor: u32,
    },
    #[error("event-store rollback requires a managed schema")]
    RollbackUnmanaged,
    #[error(
        "event-store schema operation failed: {primary}; transaction rollback also failed: {rollback}"
    )]
    MigrationTransactionRollbackFailed {
        #[source]
        primary: Box<RadrootsEventStoreError>,
        rollback: sqlx::Error,
    },
    #[error(
        "event-store ingest failed: {primary}; ingest transaction rollback also failed: {rollback}"
    )]
    IngestTransactionRollbackFailed {
        #[source]
        primary: Box<RadrootsEventStoreError>,
        rollback: sqlx::Error,
    },
    #[error("event-store source generation entropy is unavailable")]
    SourceGenerationEntropyUnavailable,
    #[error(
        "event-store retained source {resource} capacity exceeded: current {current}, requested additional {requested}, limit {limit}; durable append refused, retain a bounded source set in a new disposable cache"
    )]
    /// Refuses migration or prospective durable ingest before the retained raw
    /// source can become unrebuildable. Immutable raw rows are never pruned.
    SourceCapacityExceeded {
        resource: RadrootsEventStoreSourceCapacityResourceV1,
        current: u64,
        requested: u64,
        limit: u64,
    },
    #[error(
        "event-store retained source generation limit reached: current {current}, limit {limit}; replace and resync into a fresh store"
    )]
    SourceGenerationHistoryLimitReached { current: u32, limit: u32 },
    #[error(
        "event-store retained source contains ephemeral event `{event_id}` of kind {kind}; ephemeral events must be discarded"
    )]
    PersistedEphemeralRawEvent { event_id: String, kind: i64 },
    #[error("event-store retained source capacity authority is inconsistent: {reason}")]
    SourceCapacityStateDrift { reason: String },
    #[error("event-store migration hook `{hook_id}` state is invalid: {reason}")]
    MigrationHookStateDrift {
        hook_id: &'static str,
        reason: String,
    },
    #[error(
        "event-store raw event `{event_id}` does not match its signed raw JSON field `{field}`"
    )]
    RawEventReconciliationMismatch {
        event_id: String,
        field: &'static str,
    },
    #[error(
        "event-store raw authority drift: expected events={expected_count}, tags={expected_tag_count}, high-water={expected_high_water}; found events={actual_count}, tags={actual_tag_count}, high-water={actual_high_water}"
    )]
    RawEventSourceDrift {
        expected_count: i64,
        expected_tag_count: i64,
        expected_high_water: i64,
        actual_count: i64,
        actual_tag_count: i64,
        actual_high_water: i64,
    },
    #[error("SQLite integrity check failed: {detail}")]
    IntegrityCheckFailed { detail: String },
    #[error("event-store FTS5 integrity check failed for `{table}`: {source}")]
    Fts5IntegrityCheckFailed {
        table: &'static str,
        #[source]
        source: sqlx::Error,
    },
    #[error(
        "event-store foreign-key violation in `{table}` row {rowid:?}, parent `{parent}`, constraint {foreign_key_index}"
    )]
    ForeignKeyViolation {
        table: String,
        rowid: Option<i64>,
        parent: String,
        foreign_key_index: i64,
    },
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
    #[error("projection `{projection_id}` has no source generation and must be rebuilt")]
    ProjectionCursorRebuildRequired { projection_id: String },
    #[error(
        "projection `{projection_id}` source generation does not match the active event-store generation"
    )]
    ProjectionSourceGenerationMismatch { projection_id: String },
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
    #[error(
        "projection `{projection_id}` cursor sequence {proposed} is ahead of the active raw source high-water {high_water}"
    )]
    ProjectionCursorAheadOfSource {
        projection_id: String,
        proposed: i64,
        high_water: i64,
    },
    #[error(
        "projection `{projection_id}` version {projection_version} is already current for the active source generation"
    )]
    ProjectionRebuildNotRequired {
        projection_id: String,
        projection_version: u32,
    },
    #[error("projection `{projection_id}` rebuild ticket no longer matches stored state")]
    ProjectionRebuildTicketConflict { projection_id: String },
    #[error("projection id cannot be empty")]
    InvalidProjectionId,
    #[error("projection `{projection_id}` version is invalid: {value}")]
    InvalidProjectionVersion { projection_id: String, value: i64 },
    #[error("projection `{projection_id}` source revision is invalid: {value:?}")]
    InvalidProjectionSourceRevision {
        projection_id: String,
        value: Option<i64>,
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
    #[error("transport observation timestamp cannot be negative: {value}")]
    InvalidTransportObservationTimestamp { value: i64 },
    #[error("event ingest timestamp cannot be negative: {value}")]
    InvalidEventIngestTimestamp { value: i64 },
    #[error(
        "transport observation caller-redacted message is invalid: {reason}; bytes={actual_bytes}, max={max_bytes}"
    )]
    InvalidTransportObservationMessage {
        reason: &'static str,
        actual_bytes: usize,
        max_bytes: usize,
    },
    #[error(
        "stored transport observation caller-redacted message for event `{event_id}` is invalid: {reason}; bytes={actual_bytes}, max={max_bytes}"
    )]
    InvalidStoredTransportObservationMessage {
        event_id: String,
        reason: &'static str,
        actual_bytes: usize,
        max_bytes: usize,
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

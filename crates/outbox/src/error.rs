#![forbid(unsafe_code)]

use radroots_transport::RadrootsTransportError;
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RadrootsOutboxError {
    #[cfg(feature = "sqlite")]
    #[error("SQLite operation failed")]
    Sqlx(
        #[source]
        #[from]
        sqlx::Error,
    ),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[cfg(feature = "event-store-adapter")]
    #[error("Event store error: {0}")]
    EventStore(#[from] radroots_event_store::RadrootsEventStoreError),

    #[error("Signed event does not match frozen draft: {0}")]
    SignedEventDraftMismatch(#[from] radroots_event::draft::RadrootsDraftError),

    #[error("Event wire error: {0}")]
    EventWire(#[from] radroots_event::wire::RadrootsEventWireError),

    #[error("Signed event error: {0}")]
    SignedEvent(#[from] radroots_event::draft::RadrootsSignedEventError),

    #[error("delivery targets cannot be empty")]
    EmptyDeliveryTargets,

    #[error("transport profile id cannot be empty")]
    EmptyTransportProfileId,

    #[error("trade mutation drafts require the semantic trade mutation outbox API")]
    TradeMutationRequiresSemanticOutbox,

    #[error("ephemeral event kind {kind} cannot enter the durable generic outbox")]
    EphemeralEventNotQueueable { kind: u32 },

    #[error("SQLite outbox file connection did not enter WAL journal mode; reported `{actual}`")]
    SqliteFileJournalModeNotWal { actual: String },

    #[cfg(feature = "sqlite")]
    #[error("SQLite outbox main database must use UTF-8 encoding; reported `{actual}`")]
    SqliteMainDatabaseEncodingNotUtf8 { actual: String },

    #[cfg(feature = "sqlite")]
    #[error("SQLite outbox connection must enforce foreign keys; reported {actual}")]
    SqliteForeignKeysNotEnabled { actual: i64 },

    #[cfg(feature = "sqlite")]
    #[error("SQLite outbox open deadline of {limit_ms} ms was exhausted during {stage}")]
    SqliteOpenDeadlineExceeded { stage: &'static str, limit_ms: u64 },

    #[cfg(feature = "sqlite")]
    #[error("SQLite outbox file path is {actual} bytes; maximum is {max}")]
    SqliteFilePathTooLong { max: usize, actual: usize },

    #[cfg(feature = "sqlite")]
    #[error("SQLite outbox file path cannot identify one database file")]
    SqliteFilePathInvalid,

    #[cfg(feature = "sqlite")]
    #[error("SQLite outbox file path could not be resolved")]
    SqliteFilePathResolutionFailed {
        #[source]
        source: std::io::Error,
    },

    #[cfg(feature = "sqlite")]
    #[error("SQLite outbox offline rollback is already active for this database")]
    SqliteOfflineRollbackInProgress,

    #[cfg(feature = "sqlite")]
    #[error("SQLite outbox offline rollback requires all owned live handles to be closed")]
    SqliteOfflineRollbackHasLiveHandles,

    #[cfg(feature = "sqlite")]
    #[error("SQLite outbox lifecycle failed during {stage}")]
    SqliteLifecycleFailure { stage: &'static str },

    #[cfg(feature = "sqlite")]
    #[error("SQLite {field} is {actual} bytes; maximum is {max}")]
    SqliteTextLimitExceeded {
        field: &'static str,
        max: usize,
        actual: usize,
    },

    #[cfg(feature = "sqlite")]
    #[error(
        "temporary schema object `{name}` ({object_type}, table `{table_name}`) collides with outbox authority"
    )]
    TemporarySchemaCollision {
        object_type: String,
        name: String,
        table_name: String,
    },

    #[cfg(feature = "sqlite")]
    #[error("outbox migration registry defect: {reason}")]
    MigrationRegistryDefect { reason: String },

    #[cfg(feature = "sqlite")]
    #[error(
        "embedded outbox migration {version} {direction} length mismatch: expected {expected}, found {actual}"
    )]
    EmbeddedMigrationLengthMismatch {
        version: u32,
        direction: &'static str,
        expected: usize,
        actual: usize,
    },

    #[cfg(feature = "sqlite")]
    #[error(
        "embedded outbox migration {version} {direction} checksum mismatch: expected {expected}, found {actual}"
    )]
    EmbeddedMigrationChecksumMismatch {
        version: u32,
        direction: &'static str,
        expected: &'static str,
        actual: String,
    },

    #[cfg(feature = "sqlite")]
    #[error("outbox migration {version} {direction} catalog delta mismatch: {reason}")]
    MigrationCatalogDeltaMismatch {
        version: u32,
        direction: &'static str,
        reason: String,
    },

    #[cfg(feature = "sqlite")]
    #[error("outbox governed catalog exceeds the supported {max} rows")]
    GovernedCatalogCapacityExceeded { max: usize },

    #[cfg(feature = "sqlite")]
    #[error("unmanaged outbox schema has fingerprint {actual_schema_sha256}")]
    UnmanagedSchema { actual_schema_sha256: String },

    #[cfg(feature = "sqlite")]
    #[error("outbox migration ledger catalog is invalid: {reason}")]
    MigrationLedgerDrift { reason: String },

    #[cfg(feature = "sqlite")]
    #[error("outbox migration history gap: expected version {expected}, found {actual:?}")]
    MigrationHistoryGap { expected: u32, actual: Option<u32> },

    #[cfg(feature = "sqlite")]
    #[error("outbox migration history references unknown version {version}")]
    UnknownMigration { version: u32 },

    #[cfg(feature = "sqlite")]
    #[error("outbox schema version {database} is newer than supported version {current}")]
    SchemaTooNew { current: u32, database: i64 },

    #[cfg(feature = "sqlite")]
    #[error("outbox migration {version} name drift: expected `{expected}`, found `{actual}`")]
    MigrationHistoryNameDrift {
        version: u32,
        expected: &'static str,
        actual: String,
    },

    #[cfg(feature = "sqlite")]
    #[error(
        "outbox migration {version} {field} checksum drift: expected {expected}, found {actual}"
    )]
    MigrationHistoryChecksumDrift {
        version: u32,
        field: &'static str,
        expected: &'static str,
        actual: String,
    },

    #[cfg(feature = "sqlite")]
    #[error(
        "outbox schema fingerprint mismatch at version {version}: expected {expected}, found {actual}"
    )]
    SchemaFingerprintMismatch {
        version: u32,
        expected: &'static str,
        actual: String,
    },

    #[cfg(feature = "sqlite")]
    #[error("outbox rollback target {target} is below the supported version floor {floor}")]
    RollbackBelowVersionFloor { floor: u32, target: u32 },

    #[cfg(feature = "sqlite")]
    #[error("outbox rollback target {target} is ahead of managed version {current}")]
    RollbackAhead { current: u32, target: u32 },

    #[cfg(feature = "sqlite")]
    #[error("outbox rollback requires a managed schema")]
    RollbackUnmanaged,

    #[cfg(feature = "sqlite")]
    #[error(
        "outbox schema operation failed: {primary}; transaction rollback also failed: {rollback}"
    )]
    MigrationTransactionRollbackFailed {
        #[source]
        primary: Box<RadrootsOutboxError>,
        rollback: sqlx::Error,
    },

    #[cfg(feature = "sqlite")]
    #[error("outbox SQLite integrity check failed: {detail}")]
    IntegrityCheckFailed { detail: String },

    #[cfg(feature = "sqlite")]
    #[error(
        "outbox foreign-key violation in `{table}` row {rowid:?} against `{parent}` constraint {foreign_key_id}"
    )]
    ForeignKeyViolation {
        table: String,
        rowid: Option<i64>,
        parent: String,
        foreign_key_id: i64,
    },

    #[error(
        "trade mutation outbox metadata does not match the canonical mutation content: {field}"
    )]
    TradeMutationMetadataMismatch { field: &'static str },

    #[error("transport contract error: {0}")]
    Transport(RadrootsTransportError),

    #[error("Invalid stored enum for {field}: {value}")]
    InvalidStoredEnum { field: &'static str, value: String },

    #[error("invalid stored boolean value {value} for {field}; expected 0 or 1")]
    InvalidStoredBoolean { field: &'static str, value: i64 },

    #[error("stored event-store ingest state is inconsistent for outbox event {outbox_event_id}")]
    StoredEventStoreIngestStateInconsistent { outbox_event_id: i64 },

    #[error(
        "stored delivery target {delivery_target_id} has inconsistent canonical identity in {field}"
    )]
    InvalidStoredDeliveryTargetIdentity {
        delivery_target_id: i64,
        field: &'static str,
    },

    #[error("Invalid stored identifier for {field}: {value}")]
    InvalidStoredIdentifier { field: &'static str, value: String },

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

    #[error("Stored signed event JSON missing raw event JSON for outbox event {0}")]
    StoredSignedEventMissingRawJson(i64),

    #[error("Stored raw event JSON missing signed event JSON for outbox event {0}")]
    StoredRawEventMissingSignedEvent(i64),

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

impl RadrootsOutboxError {
    /// Returns a stable public diagnostic capped at the governed byte ceiling.
    pub fn public_diagnostic(&self) -> String {
        #[cfg(feature = "sqlite")]
        const LIMIT: usize = crate::RADROOTS_OUTBOX_DIAGNOSTIC_BYTES_MAX;
        #[cfg(not(feature = "sqlite"))]
        const LIMIT: usize = 4_096;

        let diagnostic = self.to_string();
        if diagnostic.len() <= LIMIT {
            return diagnostic;
        }
        let suffix = "…";
        let mut end = LIMIT.saturating_sub(suffix.len());
        while !diagnostic.is_char_boundary(end) {
            end = end.saturating_sub(1);
        }
        let mut bounded = diagnostic[..end].to_owned();
        bounded.push_str(suffix);
        bounded
    }
}

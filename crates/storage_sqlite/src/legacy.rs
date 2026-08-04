//! Explicit one-shot legacy import planning and immutable source backup.

use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
};

use radroots_event_codec::Codec;
use radroots_storage::{backup::MemberDigest, event::SourceGeneration, status::EventStoreMode};
use sha2::{Digest, Sha256};
use sqlx::{Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};

use crate::{Error, SqliteStorage};

const LEGACY_SOURCE_MAX: usize = 4;
/// Maximum predecessor event rows converted by one staging transaction.
pub const LEGACY_STAGE_PAGE_LIMIT_MAX: u16 = 256;
const LEGACY_MANIFEST: &str = "manifest.v1";
const EVENT_STORE_LEDGER: &str = "radroots_event_store_schema_migrations";
const EVENT_STORE_LEDGER_DDL: &str = "CREATE TABLE radroots_event_store_schema_migrations (
  version INTEGER PRIMARY KEY NOT NULL CHECK (version > 0),
  name TEXT NOT NULL UNIQUE CHECK (length(name) > 0),
  up_sha256 TEXT NOT NULL CHECK (length(up_sha256) = 64 AND up_sha256 NOT GLOB '*[^0-9a-f]*'),
  down_sha256 TEXT NOT NULL CHECK (length(down_sha256) = 64 AND down_sha256 NOT GLOB '*[^0-9a-f]*'),
  schema_sha256 TEXT NOT NULL CHECK (length(schema_sha256) = 64 AND schema_sha256 NOT GLOB '*[^0-9a-f]*')
) STRICT, WITHOUT ROWID";

const EVENT_STORE_MIGRATIONS: [LegacyEventMigration; 4] = [
    LegacyEventMigration {
        version: 1,
        name: "event_store",
        up_sha256: "4c03906a1cffd418a48d40907aa9a1ca51bb41766cff7250c4dfc7c2fd6eddde",
        down_sha256: "fa84d587f657f601947eaeb9cd239c962a48f6fcdce723588476e8d22f3c1f53",
        schema_sha256: "5b1f92779640f1a2dbd75e37a96996bda6c8be58883190f69eb3eced22a48f03",
    },
    LegacyEventMigration {
        version: 2,
        name: "nip09",
        up_sha256: "0c1730ff36eaebd285f9c0c94b9b7346af60266afa55c24a18e30446d369581a",
        down_sha256: "c51a099d9501f1e692c13d2226296a68ed9e6bfa5e8e46b2f12c6574dbe59e31",
        schema_sha256: "1fee6b2bb8cdc4602d9c89fecd97c3f51312b9a4339dbf5049b04c692ba50b12",
    },
    LegacyEventMigration {
        version: 3,
        name: "food_availability_projection",
        up_sha256: "4e7edfb981b25f76055efc7802ec30b4034eeae9b9c0809ea4ea7c574678748a",
        down_sha256: "29d663320109d9dd0df6a00b6a53d8d988438d01f7a66960a9d4ba3482ffffb8",
        schema_sha256: "dd12467e04addcbddb5ea0f386c12a8ac05ef5ebaaf949f24dd2c62745f5aaac",
    },
    LegacyEventMigration {
        version: 4,
        name: "source_maintenance",
        up_sha256: "ab2724188f8d08c897eebea2533a635e7c74282a25e84e4c0c37e78b08837a43",
        down_sha256: "fe44fd53c51545c08ea479b385e6781079dab70fc63da2a3c205d727a00ce860",
        schema_sha256: "074f85b663444ac150239ecd8441ea4a96ad83a798a55e22d2e5e2f7ee943a8c",
    },
];
const OUTBOX_CATALOG_SHA256: &str =
    "e7eeba00de78ec6d990c620e7c056018166e8a00bb703e472ef6f67a00870293";
const PRIVATE_CATALOG_SHA256: &str =
    "5aa3664e3ecb4461bde0589c3e8f73be041b715a83a67add366af975f827614e";
const STUDIO_CATALOG_SHA256: &str =
    "3e13518dba056db82090a336833618ca1bc3a44ba49067967ad9bf4c22768193";

#[derive(Clone, Copy)]
struct LegacyEventMigration {
    version: u32,
    name: &'static str,
    up_sha256: &'static str,
    down_sha256: &'static str,
    schema_sha256: &'static str,
}

/// Stable identity for one forward-only legacy import attempt.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LegacyImportId([u8; 16]);

impl LegacyImportId {
    /// Creates a non-zero caller-supplied import identity.
    pub const fn new(bytes: [u8; 16]) -> Result<Self, Error> {
        if bytes_are_zero(&bytes) {
            Err(Error::InvalidLegacyImportPlan)
        } else {
            Ok(Self(bytes))
        }
    }

    /// Returns the stable identity bytes.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// Supported predecessor database families accepted by the one-shot planner.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum LegacySourceKind {
    EventStore,
    Outbox,
    Private,
    Studio,
}

impl LegacySourceKind {
    /// Returns the stable policy identifier for this source family.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EventStore => "event_store",
            Self::Outbox => "outbox",
            Self::Private => "private",
            Self::Studio => "studio",
        }
    }

    const fn backup_file_name(self) -> &'static str {
        match self {
            Self::EventStore => "event_store.sqlite",
            Self::Outbox => "outbox.sqlite",
            Self::Private => "private.sqlite",
            Self::Studio => "studio.sqlite",
        }
    }
}

/// Exact supported predecessor schema selected by fail-closed classification.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum LegacySchema {
    EventStoreV1,
    EventStoreV2,
    EventStoreV3,
    EventStoreV4,
    OutboxV1,
    PrivateV1,
    StudioV1HostHandoff,
}

impl LegacySchema {
    /// Returns the stable schema identifier recorded by the importer.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EventStoreV1 => "event_store_v1",
            Self::EventStoreV2 => "event_store_v2",
            Self::EventStoreV3 => "event_store_v3",
            Self::EventStoreV4 => "event_store_v4",
            Self::OutboxV1 => "outbox_v1",
            Self::PrivateV1 => "private_v1",
            Self::StudioV1HostHandoff => "studio_v1_host_handoff",
        }
    }

    /// Returns whether this source is converted into owned storage or handed to its host.
    pub const fn disposition(self) -> LegacyImportDisposition {
        match self {
            Self::StudioV1HostHandoff => LegacyImportDisposition::HostHandoff,
            Self::EventStoreV1
            | Self::EventStoreV2
            | Self::EventStoreV3
            | Self::EventStoreV4
            | Self::OutboxV1
            | Self::PrivateV1 => LegacyImportDisposition::Import,
        }
    }
}

/// Required destination behavior for one classified predecessor source.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum LegacyImportDisposition {
    Import,
    HostHandoff,
}

impl LegacyImportDisposition {
    /// Returns the stable durable-journal value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Import => "import",
            Self::HostHandoff => "host_handoff",
        }
    }
}

/// Exact schema evidence for one classified predecessor snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacySourceClassification {
    kind: LegacySourceKind,
    schema: LegacySchema,
    user_version: u32,
    catalog_sha256: MemberDigest,
}

impl LegacySourceClassification {
    /// Returns the predecessor source family.
    pub const fn kind(&self) -> LegacySourceKind {
        self.kind
    }

    /// Returns the exact supported predecessor schema.
    pub const fn schema(&self) -> LegacySchema {
        self.schema
    }

    /// Returns the observed SQLite application user version.
    pub const fn user_version(&self) -> u32 {
        self.user_version
    }

    /// Returns the exact governed SQLite schema-catalog fingerprint.
    pub const fn catalog_sha256(&self) -> MemberDigest {
        self.catalog_sha256
    }
}

/// Fully reverified classification of a prepared import evidence bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassifiedLegacyImport {
    prepared: PreparedLegacyImport,
    sources: Vec<LegacySourceClassification>,
}

impl ClassifiedLegacyImport {
    /// Returns the stable import-attempt identity.
    pub const fn import_id(&self) -> LegacyImportId {
        self.prepared.import_id()
    }

    /// Returns the exact destination storage generation.
    pub const fn target_generation(&self) -> SourceGeneration {
        self.prepared.target_generation()
    }

    /// Returns the reverified finalized evidence bundle.
    pub fn bundle_path(&self) -> &Path {
        self.prepared.bundle_path()
    }

    /// Returns exact classifications in stable source-family order.
    pub fn sources(&self) -> &[LegacySourceClassification] {
        &self.sources
    }
}

/// Durable whole-import lifecycle state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum LegacyImportState {
    Classified,
    Staging,
    Ready,
    Committing,
    Complete,
}

impl LegacyImportState {
    /// Returns the stable SQLite journal value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Classified => "classified",
            Self::Staging => "staging",
            Self::Ready => "ready",
            Self::Committing => "committing",
            Self::Complete => "complete",
        }
    }
}

/// Durable per-source staging lifecycle state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum LegacyImportMemberState {
    Pending,
    Staging,
    Ready,
    Complete,
}

impl LegacyImportMemberState {
    /// Returns the stable SQLite journal value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Staging => "staging",
            Self::Ready => "ready",
            Self::Complete => "complete",
        }
    }
}

/// Durable recovery state for one classified predecessor source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyImportMemberJournal {
    classification: LegacySourceClassification,
    state: LegacyImportMemberState,
    resume_cursor: Option<Vec<u8>>,
    staged_row_count: u64,
    updated_at_unix_ms: u64,
}

impl LegacyImportMemberJournal {
    /// Returns the exact source classification bound to this member.
    pub const fn classification(&self) -> &LegacySourceClassification {
        &self.classification
    }

    /// Returns the durable staging state.
    pub const fn state(&self) -> LegacyImportMemberState {
        self.state
    }

    /// Returns the opaque source-specific resume cursor.
    pub fn resume_cursor(&self) -> Option<&[u8]> {
        self.resume_cursor.as_deref()
    }

    /// Returns the number of rows durably staged so far.
    pub const fn staged_row_count(&self) -> u64 {
        self.staged_row_count
    }

    /// Returns the positive last-update timestamp supplied by the host.
    pub const fn updated_at_unix_ms(&self) -> u64 {
        self.updated_at_unix_ms
    }
}

/// Exact durable recovery journal for one target-bound import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyImportJournal {
    import_id: LegacyImportId,
    target_generation: SourceGeneration,
    manifest_sha256: MemberDigest,
    classification_sha256: MemberDigest,
    state: LegacyImportState,
    started_at_unix_ms: u64,
    updated_at_unix_ms: u64,
    completed_at_unix_ms: Option<u64>,
    members: Vec<LegacyImportMemberJournal>,
}

/// Result of one bounded, durable legacy event-store staging transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyEventStagePage {
    staged_rows: u16,
    staged_row_count: u64,
    resume_cursor: Option<[u8; 8]>,
    complete: bool,
}

/// Stable predecessor table order for bounded legacy outbox graph staging.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum LegacyOutboxTable {
    Operations,
    Events,
    DeliveryPlans,
    DeliveryTargets,
    DeliveryAttempts,
}

impl LegacyOutboxTable {
    /// Returns the stable staging table-kind value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Operations => "operations",
            Self::Events => "events",
            Self::DeliveryPlans => "delivery_plans",
            Self::DeliveryTargets => "delivery_targets",
            Self::DeliveryAttempts => "delivery_attempts",
        }
    }

    const fn code(self) -> u8 {
        match self {
            Self::Operations => 1,
            Self::Events => 2,
            Self::DeliveryPlans => 3,
            Self::DeliveryTargets => 4,
            Self::DeliveryAttempts => 5,
        }
    }

    const fn next(self) -> Option<Self> {
        match self {
            Self::Operations => Some(Self::Events),
            Self::Events => Some(Self::DeliveryPlans),
            Self::DeliveryPlans => Some(Self::DeliveryTargets),
            Self::DeliveryTargets => Some(Self::DeliveryAttempts),
            Self::DeliveryAttempts => None,
        }
    }
}

/// Result of one bounded legacy outbox table staging transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyOutboxStagePage {
    table: LegacyOutboxTable,
    staged_rows: u16,
    staged_row_count: u64,
    resume_cursor: [u8; 9],
    complete: bool,
}

/// Stable predecessor table order for protected legacy private-store staging.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum LegacyPrivateTable {
    Metadata,
    WrappedProfileKeys,
    SigningSecrets,
    FarmLocations,
    TradeArtifacts,
    CursorKeys,
    Nip46Sessions,
    RotationProgress,
}

impl LegacyPrivateTable {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::WrappedProfileKeys => "wrapped_profile_keys",
            Self::SigningSecrets => "signing_secrets",
            Self::FarmLocations => "farm_locations",
            Self::TradeArtifacts => "trade_artifacts",
            Self::CursorKeys => "cursor_keys",
            Self::Nip46Sessions => "nip46_sessions",
            Self::RotationProgress => "rotation_progress",
        }
    }

    const fn code(self) -> u8 {
        match self {
            Self::Metadata => 1,
            Self::WrappedProfileKeys => 2,
            Self::SigningSecrets => 3,
            Self::FarmLocations => 4,
            Self::TradeArtifacts => 5,
            Self::CursorKeys => 6,
            Self::Nip46Sessions => 7,
            Self::RotationProgress => 8,
        }
    }

    const fn next(self) -> Option<Self> {
        match self {
            Self::Metadata => Some(Self::WrappedProfileKeys),
            Self::WrappedProfileKeys => Some(Self::SigningSecrets),
            Self::SigningSecrets => Some(Self::FarmLocations),
            Self::FarmLocations => Some(Self::TradeArtifacts),
            Self::TradeArtifacts => Some(Self::CursorKeys),
            Self::CursorKeys => Some(Self::Nip46Sessions),
            Self::Nip46Sessions => Some(Self::RotationProgress),
            Self::RotationProgress => None,
        }
    }
}

/// Result of one recoverable protected legacy private-store page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyPrivateStagePage {
    table: LegacyPrivateTable,
    staged_rows: u16,
    staged_row_count: u64,
    resume_cursor: Vec<u8>,
    complete: bool,
}

/// Immutable host-owned handoff descriptor for one classified Studio snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyStudioHandoff {
    import_id: LegacyImportId,
    evidence_path: PathBuf,
    byte_length: u64,
    source_sha256: MemberDigest,
    catalog_sha256: MemberDigest,
    handoff_sha256: MemberDigest,
}

impl LegacyStudioHandoff {
    /// Returns the import attempt bound to this handoff.
    pub const fn import_id(&self) -> LegacyImportId {
        self.import_id
    }

    /// Returns the immutable backed-up Studio database offered to the host.
    pub fn evidence_path(&self) -> &Path {
        &self.evidence_path
    }

    /// Returns the exact backed-up Studio database length.
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    /// Returns the exact backed-up Studio database digest.
    pub const fn source_sha256(&self) -> MemberDigest {
        self.source_sha256
    }

    /// Returns the exact classified Studio schema-catalog digest.
    pub const fn catalog_sha256(&self) -> MemberDigest {
        self.catalog_sha256
    }

    /// Returns the deterministic identity the host must acknowledge.
    pub const fn handoff_sha256(&self) -> MemberDigest {
        self.handoff_sha256
    }
}

/// Host-supplied proof that a specific Studio handoff was durably accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyStudioHandoffReceipt {
    handoff_sha256: MemberDigest,
    host_commitment_sha256: MemberDigest,
}

/// Snapshot-consistent proof that every legacy member is ready to commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyImportValidation {
    imported_row_count: u64,
    validation_sha256: MemberDigest,
}

/// Durable receipt for one fully sealed, forward-only legacy import.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyImportCommitReceipt {
    validation_sha256: MemberDigest,
    imported_row_count: u64,
    completed_at_unix_ms: u64,
}

impl LegacyImportCommitReceipt {
    /// Returns the exact validation identity sealed by both databases.
    pub const fn validation_sha256(&self) -> MemberDigest {
        self.validation_sha256
    }

    /// Returns the exact retained SDK-owned predecessor row count.
    pub const fn imported_row_count(&self) -> u64 {
        self.imported_row_count
    }

    /// Returns the positive host-supplied completion timestamp.
    pub const fn completed_at_unix_ms(&self) -> u64 {
        self.completed_at_unix_ms
    }
}

impl LegacyImportValidation {
    /// Returns the exact number of predecessor rows staged for SDK-owned storage.
    pub const fn imported_row_count(&self) -> u64 {
        self.imported_row_count
    }

    /// Returns the deterministic commit identity for all staged rows and receipts.
    pub const fn validation_sha256(&self) -> MemberDigest {
        self.validation_sha256
    }
}

impl LegacyStudioHandoffReceipt {
    /// Binds an exact handoff to a non-zero host-owned durable commitment.
    pub const fn new(
        handoff_sha256: MemberDigest,
        host_commitment_sha256: MemberDigest,
    ) -> Result<Self, Error> {
        if bytes_are_zero(host_commitment_sha256.as_bytes()) {
            Err(Error::InvalidLegacyImportStageRequest)
        } else {
            Ok(Self {
                handoff_sha256,
                host_commitment_sha256,
            })
        }
    }

    /// Returns the acknowledged handoff identity.
    pub const fn handoff_sha256(&self) -> MemberDigest {
        self.handoff_sha256
    }

    /// Returns the opaque host-owned durable commitment.
    pub const fn host_commitment_sha256(&self) -> MemberDigest {
        self.host_commitment_sha256
    }
}

impl LegacyPrivateStagePage {
    pub const fn table(&self) -> LegacyPrivateTable {
        self.table
    }
    pub const fn staged_rows(&self) -> u16 {
        self.staged_rows
    }
    pub const fn staged_row_count(&self) -> u64 {
        self.staged_row_count
    }
    pub fn resume_cursor(&self) -> &[u8] {
        &self.resume_cursor
    }
    pub const fn is_complete(&self) -> bool {
        self.complete
    }
}

impl LegacyOutboxStagePage {
    /// Returns the predecessor table processed by this page.
    pub const fn table(&self) -> LegacyOutboxTable {
        self.table
    }

    /// Returns rows newly staged by this transaction.
    pub const fn staged_rows(&self) -> u16 {
        self.staged_rows
    }

    /// Returns the cumulative durable outbox graph row count.
    pub const fn staged_row_count(&self) -> u64 {
        self.staged_row_count
    }

    /// Returns the exact table-discriminated predecessor cursor.
    pub const fn resume_cursor(&self) -> &[u8; 9] {
        &self.resume_cursor
    }

    /// Reports whether all five predecessor tables reached their exact end.
    pub const fn is_complete(&self) -> bool {
        self.complete
    }
}

impl LegacyEventStagePage {
    /// Returns rows newly converted by this transaction.
    pub const fn staged_rows(&self) -> u16 {
        self.staged_rows
    }

    /// Returns the total durable event staging row count for this import.
    pub const fn staged_row_count(&self) -> u64 {
        self.staged_row_count
    }

    /// Returns the exact big-endian predecessor `event_envelopes.seq` cursor.
    pub const fn resume_cursor(&self) -> Option<&[u8; 8]> {
        self.resume_cursor.as_ref()
    }

    /// Reports whether the source member has reached its exact end.
    pub const fn is_complete(&self) -> bool {
        self.complete
    }
}

impl LegacyImportJournal {
    /// Returns the stable import identity.
    pub const fn import_id(&self) -> LegacyImportId {
        self.import_id
    }

    /// Returns the exact destination storage generation.
    pub const fn target_generation(&self) -> SourceGeneration {
        self.target_generation
    }

    /// Returns the exact finalized evidence-manifest digest.
    pub const fn manifest_sha256(&self) -> MemberDigest {
        self.manifest_sha256
    }

    /// Returns the exact ordered-classification digest.
    pub const fn classification_sha256(&self) -> MemberDigest {
        self.classification_sha256
    }

    /// Returns the durable whole-import state.
    pub const fn state(&self) -> LegacyImportState {
        self.state
    }

    /// Returns the positive host-supplied start timestamp.
    pub const fn started_at_unix_ms(&self) -> u64 {
        self.started_at_unix_ms
    }

    /// Returns the last positive host-supplied update timestamp.
    pub const fn updated_at_unix_ms(&self) -> u64 {
        self.updated_at_unix_ms
    }

    /// Returns the host-supplied completion timestamp once terminal.
    pub const fn completed_at_unix_ms(&self) -> Option<u64> {
        self.completed_at_unix_ms
    }

    /// Returns one exact durable row per classified source.
    pub fn members(&self) -> &[LegacyImportMemberJournal] {
        &self.members
    }
}

/// One explicitly typed existing predecessor database.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacySource {
    kind: LegacySourceKind,
    path: PathBuf,
}

impl LegacySource {
    /// Binds a source family to one absolute existing regular SQLite file.
    pub fn new(kind: LegacySourceKind, path: impl Into<PathBuf>) -> Result<Self, Error> {
        let path = path.into();
        validate_source_path(&path)?;
        Ok(Self { kind, path })
    }

    /// Returns the declared predecessor database family.
    pub const fn kind(&self) -> LegacySourceKind {
        self.kind
    }

    /// Returns the exact caller-supplied predecessor database path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Immutable authority for one pre-backed-up, forward-only import attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyImportPlan {
    import_id: LegacyImportId,
    sources: Vec<LegacySource>,
    backup_root: PathBuf,
    requested_at_unix_ms: u64,
}

impl LegacyImportPlan {
    /// Creates a deterministic import plan with one source per family.
    pub fn new(
        import_id: LegacyImportId,
        mut sources: Vec<LegacySource>,
        backup_root: impl Into<PathBuf>,
        requested_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        let backup_root = backup_root.into();
        crate::backup::validate_backup_root(&backup_root)?;
        if requested_at_unix_ms == 0 || sources.is_empty() || sources.len() > LEGACY_SOURCE_MAX {
            return Err(Error::InvalidLegacyImportPlan);
        }
        let mut kinds = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for source in &sources {
            validate_source_path(source.path())?;
            if !kinds.insert(source.kind()) || !paths.insert(source.path().to_path_buf()) {
                return Err(Error::InvalidLegacyImportPlan);
            }
        }
        sources.sort_by_key(LegacySource::kind);
        Ok(Self {
            import_id,
            sources,
            backup_root,
            requested_at_unix_ms,
        })
    }

    /// Returns the stable import-attempt identity.
    pub const fn import_id(&self) -> LegacyImportId {
        self.import_id
    }

    /// Returns sources in stable source-family order.
    pub fn sources(&self) -> &[LegacySource] {
        &self.sources
    }

    /// Returns the existing host-owned directory for immutable import evidence.
    pub fn backup_root(&self) -> &Path {
        &self.backup_root
    }

    /// Returns the positive host-supplied import request timestamp.
    pub const fn requested_at_unix_ms(&self) -> u64 {
        self.requested_at_unix_ms
    }
}

/// Exact immutable evidence for one backed-up predecessor member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacySourceSnapshot {
    kind: LegacySourceKind,
    relative_path: String,
    byte_length: u64,
    sha256: MemberDigest,
}

impl LegacySourceSnapshot {
    /// Returns the predecessor database family.
    pub const fn kind(&self) -> LegacySourceKind {
        self.kind
    }

    /// Returns the stable bundle-relative snapshot path.
    pub fn relative_path(&self) -> &str {
        self.relative_path.as_str()
    }

    /// Returns the exact snapshot length.
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    /// Returns the exact snapshot SHA-256 digest.
    pub const fn sha256(&self) -> MemberDigest {
        self.sha256
    }
}

/// Durable result of the mandatory pre-import source backup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedLegacyImport {
    import_id: LegacyImportId,
    target_generation: SourceGeneration,
    bundle_path: PathBuf,
    manifest_byte_length: u64,
    manifest_sha256: MemberDigest,
    snapshots: Vec<LegacySourceSnapshot>,
}

impl PreparedLegacyImport {
    /// Returns the stable import-attempt identity.
    pub const fn import_id(&self) -> LegacyImportId {
        self.import_id
    }

    /// Returns the exact destination storage generation.
    pub const fn target_generation(&self) -> SourceGeneration {
        self.target_generation
    }

    /// Returns the finalized immutable evidence bundle.
    pub fn bundle_path(&self) -> &Path {
        &self.bundle_path
    }

    /// Returns the exact manifest length.
    pub const fn manifest_byte_length(&self) -> u64 {
        self.manifest_byte_length
    }

    /// Returns the exact manifest SHA-256 digest.
    pub const fn manifest_sha256(&self) -> MemberDigest {
        self.manifest_sha256
    }

    /// Returns the exact evidence inventory in stable source-family order.
    pub fn snapshots(&self) -> &[LegacySourceSnapshot] {
        &self.snapshots
    }
}

impl SqliteStorage {
    /// Captures and verifies every legacy source before any import mutation.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn prepare_legacy_import(
        &self,
        plan: &LegacyImportPlan,
    ) -> Result<PreparedLegacyImport, Error> {
        self.lifecycle
            .require_open()
            .map_err(|_| Error::BackupBackendUnavailable)?;
        if self.mode != EventStoreMode::ReadWrite {
            return Err(Error::RestoreRequiresWritableStorage);
        }
        for source in plan.sources() {
            validate_source_path(source.path())?;
            if let Some(paths) = self.paths.as_deref() {
                for owned in [paths.runtime(), paths.private()] {
                    if paths_refer_to_same_file(source.path(), owned)? {
                        return Err(Error::InvalidLegacySource(source.path().to_path_buf()));
                    }
                }
            }
        }

        let layout = LegacyBackupLayout::new(plan);
        layout.create()?;
        let mut snapshots = Vec::with_capacity(plan.sources().len());
        for source in plan.sources() {
            let destination = layout.staging.join(source.kind().backup_file_name());
            capture_legacy_source(source, &destination).await?;
            snapshots.push(snapshot(source.kind(), &destination)?);
        }
        let manifest_path = layout.staging.join(LEGACY_MANIFEST);
        write_manifest(plan, self.generation, &snapshots, &manifest_path)?;
        let (manifest_byte_length, manifest_sha256) = file_digest(&manifest_path)?;
        sync_directory(&layout.staging, "sync legacy import staging bundle")?;
        fs::rename(&layout.staging, &layout.finalized).map_err(|source| {
            Error::LegacyImportFilesystem {
                operation: "finalize legacy import backup bundle",
                source,
            }
        })?;
        sync_directory(plan.backup_root(), "sync legacy import backup root")?;
        Ok(PreparedLegacyImport {
            import_id: plan.import_id(),
            target_generation: self.generation,
            bundle_path: layout.finalized,
            manifest_byte_length,
            manifest_sha256,
            snapshots,
        })
    }

    /// Revalidates a prepared bundle and classifies every exact predecessor schema.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn classify_legacy_import(
        &self,
        prepared: &PreparedLegacyImport,
    ) -> Result<ClassifiedLegacyImport, Error> {
        self.lifecycle
            .require_open()
            .map_err(|_| Error::BackupBackendUnavailable)?;
        if self.mode != EventStoreMode::ReadWrite {
            return Err(Error::RestoreRequiresWritableStorage);
        }
        if prepared.target_generation() != self.generation {
            return Err(Error::LegacyImportTargetMismatch);
        }
        verify_prepared_evidence(prepared).await?;
        let mut sources = Vec::with_capacity(prepared.snapshots().len());
        for snapshot in prepared.snapshots() {
            sources.push(
                classify_snapshot(
                    snapshot.kind(),
                    &prepared.bundle_path().join(snapshot.relative_path()),
                )
                .await?,
            );
        }
        Ok(ClassifiedLegacyImport {
            prepared: prepared.clone(),
            sources,
        })
    }

    /// Atomically creates or resumes the exact durable journal for a classification.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn begin_legacy_import(
        &self,
        classified: &ClassifiedLegacyImport,
        started_at_unix_ms: u64,
    ) -> Result<LegacyImportJournal, Error> {
        self.require_legacy_import_writer(classified.target_generation())?;
        if started_at_unix_ms == 0 || classified.sources().is_empty() {
            return Err(Error::InvalidLegacyImportJournal);
        }
        verify_prepared_evidence(&classified.prepared).await?;
        let started_at =
            i64::try_from(started_at_unix_ms).map_err(|_| Error::InvalidLegacyImportJournal)?;
        let classification_sha256 = classification_digest(classified);
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| Error::LegacyImportJournalFailed)?;
        let existing = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM radroots_runtime_legacy_imports WHERE import_id = ?",
        )
        .bind(classified.import_id().as_bytes().as_slice())
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| Error::LegacyImportJournalFailed)?;
        if existing == 0 {
            let active = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM radroots_runtime_legacy_imports WHERE target_generation = ?",
            )
            .bind(classified.target_generation().as_bytes().as_slice())
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| Error::LegacyImportJournalFailed)?;
            if active != 0 {
                transaction
                    .rollback()
                    .await
                    .map_err(|_| Error::LegacyImportJournalFailed)?;
                return Err(Error::LegacyImportConflict);
            }
            sqlx::query(
                "INSERT INTO radroots_runtime_legacy_imports(
                    import_id, target_generation, manifest_sha256,
                    classification_sha256, state, started_at_ms,
                    updated_at_ms, completed_at_ms
                 ) VALUES (?, ?, ?, ?, 'classified', ?, ?, NULL)",
            )
            .bind(classified.import_id().as_bytes().as_slice())
            .bind(classified.target_generation().as_bytes().as_slice())
            .bind(classified.prepared.manifest_sha256().as_bytes().as_slice())
            .bind(classification_sha256.as_bytes().as_slice())
            .bind(started_at)
            .bind(started_at)
            .execute(&mut *transaction)
            .await
            .map_err(|_| Error::LegacyImportJournalFailed)?;
            for source in classified.sources() {
                sqlx::query(
                    "INSERT INTO radroots_runtime_legacy_import_members(
                        import_id, source_kind, legacy_schema, disposition,
                        catalog_sha256, state, resume_cursor, staged_row_count,
                        updated_at_ms
                     ) VALUES (?, ?, ?, ?, ?, 'pending', NULL, 0, ?)",
                )
                .bind(classified.import_id().as_bytes().as_slice())
                .bind(source.kind().as_str())
                .bind(source.schema().as_str())
                .bind(source.schema().disposition().as_str())
                .bind(source.catalog_sha256().as_bytes().as_slice())
                .bind(started_at)
                .execute(&mut *transaction)
                .await
                .map_err(|_| Error::LegacyImportJournalFailed)?;
            }
        }
        transaction
            .commit()
            .await
            .map_err(|_| Error::LegacyImportJournalFailed)?;
        let journal = self
            .legacy_import_journal(classified.import_id())
            .await?
            .ok_or(Error::InvalidLegacyImportJournal)?;
        if journal_matches_classified(&journal, classified, classification_sha256) {
            Ok(journal)
        } else {
            Err(Error::LegacyImportConflict)
        }
    }

    /// Reads exact durable recovery state without advancing the importer.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn legacy_import_journal(
        &self,
        import_id: LegacyImportId,
    ) -> Result<Option<LegacyImportJournal>, Error> {
        self.lifecycle
            .require_open()
            .map_err(|_| Error::BackupBackendUnavailable)?;
        let mut transaction = self
            .pool
            .begin_with("BEGIN")
            .await
            .map_err(|_| Error::LegacyImportJournalFailed)?;
        let row = sqlx::query(
            "SELECT import_id, target_generation, manifest_sha256,
                    classification_sha256, state, started_at_ms, updated_at_ms,
                    completed_at_ms
             FROM radroots_runtime_legacy_imports WHERE import_id = ?",
        )
        .bind(import_id.as_bytes().as_slice())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| Error::LegacyImportJournalFailed)?;
        let Some(row) = row else {
            transaction
                .commit()
                .await
                .map_err(|_| Error::LegacyImportJournalFailed)?;
            return Ok(None);
        };
        let durable_import_id = decode_import_id(
            row.try_get("import_id")
                .map_err(|_| Error::InvalidLegacyImportJournal)?,
        )?;
        if durable_import_id != import_id {
            return Err(Error::InvalidLegacyImportJournal);
        }
        let target_generation = decode_generation(
            row.try_get("target_generation")
                .map_err(|_| Error::InvalidLegacyImportJournal)?,
        )?;
        if target_generation != self.generation {
            return Err(Error::InvalidLegacyImportJournal);
        }
        let manifest_sha256 = decode_digest(
            row.try_get("manifest_sha256")
                .map_err(|_| Error::InvalidLegacyImportJournal)?,
        )?;
        let classification_sha256 = decode_digest(
            row.try_get("classification_sha256")
                .map_err(|_| Error::InvalidLegacyImportJournal)?,
        )?;
        let state = parse_import_state(
            row.try_get::<String, _>("state")
                .map_err(|_| Error::InvalidLegacyImportJournal)?
                .as_str(),
        )?;
        let started_at_unix_ms = decode_positive_time(
            row.try_get("started_at_ms")
                .map_err(|_| Error::InvalidLegacyImportJournal)?,
        )?;
        let updated_at_unix_ms = decode_positive_time(
            row.try_get("updated_at_ms")
                .map_err(|_| Error::InvalidLegacyImportJournal)?,
        )?;
        let completed_at_unix_ms = row
            .try_get::<Option<i64>, _>("completed_at_ms")
            .map_err(|_| Error::InvalidLegacyImportJournal)?
            .map(decode_positive_time)
            .transpose()?;
        let member_rows = sqlx::query(
            "SELECT source_kind, legacy_schema, disposition, catalog_sha256,
                    state, resume_cursor, staged_row_count, updated_at_ms
             FROM radroots_runtime_legacy_import_members
             WHERE import_id = ? ORDER BY source_kind",
        )
        .bind(import_id.as_bytes().as_slice())
        .fetch_all(&mut *transaction)
        .await
        .map_err(|_| Error::LegacyImportJournalFailed)?;
        if member_rows.is_empty() || member_rows.len() > LEGACY_SOURCE_MAX {
            return Err(Error::InvalidLegacyImportJournal);
        }
        let mut members = Vec::with_capacity(member_rows.len());
        for row in member_rows {
            let kind = parse_source_kind(
                row.try_get::<String, _>("source_kind")
                    .map_err(|_| Error::InvalidLegacyImportJournal)?
                    .as_str(),
            )?;
            let schema = parse_legacy_schema(
                row.try_get::<String, _>("legacy_schema")
                    .map_err(|_| Error::InvalidLegacyImportJournal)?
                    .as_str(),
            )?;
            let disposition = row
                .try_get::<String, _>("disposition")
                .map_err(|_| Error::InvalidLegacyImportJournal)?;
            if disposition != schema.disposition().as_str() || schema_source_kind(schema) != kind {
                return Err(Error::InvalidLegacyImportJournal);
            }
            members.push(LegacyImportMemberJournal {
                classification: LegacySourceClassification {
                    kind,
                    schema,
                    user_version: expected_user_version(schema),
                    catalog_sha256: decode_digest(
                        row.try_get("catalog_sha256")
                            .map_err(|_| Error::InvalidLegacyImportJournal)?,
                    )?,
                },
                state: parse_member_state(
                    row.try_get::<String, _>("state")
                        .map_err(|_| Error::InvalidLegacyImportJournal)?
                        .as_str(),
                )?,
                resume_cursor: row
                    .try_get("resume_cursor")
                    .map_err(|_| Error::InvalidLegacyImportJournal)?,
                staged_row_count: u64::try_from(
                    row.try_get::<i64, _>("staged_row_count")
                        .map_err(|_| Error::InvalidLegacyImportJournal)?,
                )
                .map_err(|_| Error::InvalidLegacyImportJournal)?,
                updated_at_unix_ms: decode_positive_time(
                    row.try_get("updated_at_ms")
                        .map_err(|_| Error::InvalidLegacyImportJournal)?,
                )?,
            });
        }
        if updated_at_unix_ms < started_at_unix_ms
            || members
                .iter()
                .any(|member| member.updated_at_unix_ms() < started_at_unix_ms)
            || !journal_member_states_are_consistent(state, &members)
        {
            return Err(Error::InvalidLegacyImportJournal);
        }
        transaction
            .commit()
            .await
            .map_err(|_| Error::LegacyImportJournalFailed)?;
        Ok(Some(LegacyImportJournal {
            import_id,
            target_generation,
            manifest_sha256,
            classification_sha256,
            state,
            started_at_unix_ms,
            updated_at_unix_ms,
            completed_at_unix_ms,
            members,
        }))
    }

    /// Converts one bounded page of an exact legacy event store into isolated staging.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn stage_legacy_events(
        &self,
        classified: &ClassifiedLegacyImport,
        limit: u16,
        updated_at_unix_ms: u64,
    ) -> Result<LegacyEventStagePage, Error> {
        self.require_legacy_import_writer(classified.target_generation())?;
        if limit == 0 || limit > LEGACY_STAGE_PAGE_LIMIT_MAX || updated_at_unix_ms == 0 {
            return Err(Error::InvalidLegacyImportStageRequest);
        }
        verify_prepared_evidence(&classified.prepared).await?;
        let classification_sha256 = classification_digest(classified);
        let journal = self
            .legacy_import_journal(classified.import_id())
            .await?
            .ok_or(Error::InvalidLegacyImportJournal)?;
        if !journal_matches_classified(&journal, classified, classification_sha256) {
            return Err(Error::LegacyImportConflict);
        }
        let classification = classified
            .sources()
            .iter()
            .find(|source| source.kind() == LegacySourceKind::EventStore)
            .ok_or(Error::LegacyImportConflict)?;
        if !matches!(
            classification.schema(),
            LegacySchema::EventStoreV1
                | LegacySchema::EventStoreV2
                | LegacySchema::EventStoreV3
                | LegacySchema::EventStoreV4
        ) {
            return Err(Error::LegacyImportConflict);
        }
        let snapshot = classified
            .prepared
            .snapshots()
            .iter()
            .find(|snapshot| snapshot.kind() == LegacySourceKind::EventStore)
            .ok_or(Error::LegacyImportConflict)?;
        let source_path = classified.bundle_path().join(snapshot.relative_path());
        let updated_at = i64::try_from(updated_at_unix_ms)
            .map_err(|_| Error::InvalidLegacyImportStageRequest)?;

        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        let import_row = sqlx::query(
            "SELECT state, updated_at_ms FROM radroots_runtime_legacy_imports
             WHERE import_id = ? AND target_generation = ?
               AND manifest_sha256 = ? AND classification_sha256 = ?",
        )
        .bind(classified.import_id().as_bytes().as_slice())
        .bind(classified.target_generation().as_bytes().as_slice())
        .bind(classified.prepared.manifest_sha256().as_bytes().as_slice())
        .bind(classification_sha256.as_bytes().as_slice())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| Error::LegacyImportStagingFailed)?
        .ok_or(Error::LegacyImportConflict)?;
        let import_state = parse_import_state(
            import_row
                .try_get::<String, _>("state")
                .map_err(|_| Error::InvalidLegacyImportJournal)?
                .as_str(),
        )?;
        let import_updated_at = import_row
            .try_get::<i64, _>("updated_at_ms")
            .map_err(|_| Error::InvalidLegacyImportJournal)?;
        if updated_at < import_updated_at
            || !matches!(
                import_state,
                LegacyImportState::Classified
                    | LegacyImportState::Staging
                    | LegacyImportState::Ready
            )
        {
            return Err(Error::LegacyImportConflict);
        }
        let member_row = sqlx::query(
            "SELECT state, resume_cursor, staged_row_count, updated_at_ms
             FROM radroots_runtime_legacy_import_members
             WHERE import_id = ? AND source_kind = 'event_store'",
        )
        .bind(classified.import_id().as_bytes().as_slice())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| Error::LegacyImportStagingFailed)?
        .ok_or(Error::LegacyImportConflict)?;
        let member_state = parse_member_state(
            member_row
                .try_get::<String, _>("state")
                .map_err(|_| Error::InvalidLegacyImportJournal)?
                .as_str(),
        )?;
        let durable_cursor = member_row
            .try_get::<Option<Vec<u8>>, _>("resume_cursor")
            .map_err(|_| Error::InvalidLegacyImportJournal)?;
        let staged_row_count = u64::try_from(
            member_row
                .try_get::<i64, _>("staged_row_count")
                .map_err(|_| Error::InvalidLegacyImportJournal)?,
        )
        .map_err(|_| Error::InvalidLegacyImportJournal)?;
        let member_updated_at = member_row
            .try_get::<i64, _>("updated_at_ms")
            .map_err(|_| Error::InvalidLegacyImportJournal)?;
        let resume_sequence = decode_event_stage_cursor(durable_cursor.as_deref())?;
        if updated_at < member_updated_at {
            return Err(Error::LegacyImportConflict);
        }
        if member_state == LegacyImportMemberState::Ready {
            transaction
                .commit()
                .await
                .map_err(|_| Error::LegacyImportStagingFailed)?;
            return Ok(LegacyEventStagePage {
                staged_rows: 0,
                staged_row_count,
                resume_cursor: durable_cursor
                    .as_deref()
                    .map(decode_exact_event_stage_cursor)
                    .transpose()?,
                complete: true,
            });
        }
        if !matches!(
            member_state,
            LegacyImportMemberState::Pending | LegacyImportMemberState::Staging
        ) {
            return Err(Error::LegacyImportConflict);
        }

        if import_state == LegacyImportState::Classified {
            sqlx::query(
                "UPDATE radroots_runtime_legacy_imports
                 SET state = 'staging', updated_at_ms = ? WHERE import_id = ?",
            )
            .bind(updated_at)
            .bind(classified.import_id().as_bytes().as_slice())
            .execute(&mut *transaction)
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        }
        if member_state == LegacyImportMemberState::Pending {
            sqlx::query(
                "UPDATE radroots_runtime_legacy_import_members
                 SET state = 'staging', updated_at_ms = ?
                 WHERE import_id = ? AND source_kind = 'event_store'",
            )
            .bind(updated_at)
            .bind(classified.import_id().as_bytes().as_slice())
            .execute(&mut *transaction)
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        }

        let mut source = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&source_path)
                .read_only(true),
        )
        .await
        .map_err(|_| Error::LegacyImportStagingFailed)?;
        let rows = sqlx::query(
            "SELECT seq, event_id, raw_json, verification_status, contract_status,
                    projection_eligible, inserted_at_ms, updated_at_ms
             FROM event_envelopes WHERE seq > ? ORDER BY seq LIMIT ?",
        )
        .bind(resume_sequence)
        .bind(i64::from(limit) + 1)
        .fetch_all(&mut source)
        .await
        .map_err(|_| Error::LegacyImportStagingFailed)?;
        source
            .close()
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        let complete = rows.len() <= usize::from(limit);
        let rows = rows.into_iter().take(usize::from(limit));
        let mut last_sequence = resume_sequence;
        let mut newly_staged = 0_u16;
        for row in rows {
            let converted = convert_legacy_event_row(&row)?;
            sqlx::query(
                "INSERT INTO radroots_runtime_legacy_event_staging(
                    import_id, source_kind, legacy_sequence, event_id, signed_event,
                    legacy_verification_status, legacy_contract_status,
                    legacy_projection_eligible, legacy_inserted_at_ms,
                    legacy_updated_at_ms
                 ) VALUES (?, 'event_store', ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(classified.import_id().as_bytes().as_slice())
            .bind(converted.sequence)
            .bind(converted.event_id.as_slice())
            .bind(converted.signed_event.as_slice())
            .bind(converted.verification_status)
            .bind(converted.contract_status)
            .bind(converted.projection_eligible)
            .bind(converted.inserted_at_ms)
            .bind(converted.updated_at_ms)
            .execute(&mut *transaction)
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
            last_sequence = converted.sequence;
            newly_staged = newly_staged
                .checked_add(1)
                .ok_or(Error::LegacyImportStagingFailed)?;
        }
        let total = staged_row_count
            .checked_add(u64::from(newly_staged))
            .ok_or(Error::LegacyImportStagingFailed)?;
        let cursor = (last_sequence > 0).then(|| encode_event_stage_cursor(last_sequence));
        sqlx::query(
            "UPDATE radroots_runtime_legacy_import_members
             SET state = ?, resume_cursor = ?, staged_row_count = ?, updated_at_ms = ?
             WHERE import_id = ? AND source_kind = 'event_store'",
        )
        .bind(if complete { "ready" } else { "staging" })
        .bind(cursor.as_ref().map(<[u8; 8]>::as_slice))
        .bind(i64::try_from(total).map_err(|_| Error::LegacyImportStagingFailed)?)
        .bind(updated_at)
        .bind(classified.import_id().as_bytes().as_slice())
        .execute(&mut *transaction)
        .await
        .map_err(|_| Error::LegacyImportStagingFailed)?;
        let pending_members = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM radroots_runtime_legacy_import_members
             WHERE import_id = ? AND state != 'ready'",
        )
        .bind(classified.import_id().as_bytes().as_slice())
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| Error::LegacyImportStagingFailed)?;
        sqlx::query(
            "UPDATE radroots_runtime_legacy_imports SET state = ?, updated_at_ms = ?
             WHERE import_id = ?",
        )
        .bind(if pending_members == 0 {
            "ready"
        } else {
            "staging"
        })
        .bind(updated_at)
        .bind(classified.import_id().as_bytes().as_slice())
        .execute(&mut *transaction)
        .await
        .map_err(|_| Error::LegacyImportStagingFailed)?;
        transaction
            .commit()
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        Ok(LegacyEventStagePage {
            staged_rows: newly_staged,
            staged_row_count: total,
            resume_cursor: cursor,
            complete,
        })
    }

    /// Converts one bounded table page from an exact legacy outbox graph.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn stage_legacy_outbox(
        &self,
        classified: &ClassifiedLegacyImport,
        limit: u16,
        updated_at_unix_ms: u64,
    ) -> Result<LegacyOutboxStagePage, Error> {
        self.require_legacy_import_writer(classified.target_generation())?;
        if limit == 0 || limit > LEGACY_STAGE_PAGE_LIMIT_MAX || updated_at_unix_ms == 0 {
            return Err(Error::InvalidLegacyImportStageRequest);
        }
        verify_prepared_evidence(&classified.prepared).await?;
        let classification_sha256 = classification_digest(classified);
        let journal = self
            .legacy_import_journal(classified.import_id())
            .await?
            .ok_or(Error::InvalidLegacyImportJournal)?;
        if !journal_matches_classified(&journal, classified, classification_sha256)
            || !classified.sources().iter().any(|source| {
                source.kind() == LegacySourceKind::Outbox
                    && source.schema() == LegacySchema::OutboxV1
            })
        {
            return Err(Error::LegacyImportConflict);
        }
        let snapshot = classified
            .prepared
            .snapshots()
            .iter()
            .find(|snapshot| snapshot.kind() == LegacySourceKind::Outbox)
            .ok_or(Error::LegacyImportConflict)?;
        let source_path = classified.bundle_path().join(snapshot.relative_path());
        let updated_at = i64::try_from(updated_at_unix_ms)
            .map_err(|_| Error::InvalidLegacyImportStageRequest)?;
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        let import_row = sqlx::query(
            "SELECT state, updated_at_ms FROM radroots_runtime_legacy_imports
             WHERE import_id = ? AND target_generation = ?
               AND manifest_sha256 = ? AND classification_sha256 = ?",
        )
        .bind(classified.import_id().as_bytes().as_slice())
        .bind(classified.target_generation().as_bytes().as_slice())
        .bind(classified.prepared.manifest_sha256().as_bytes().as_slice())
        .bind(classification_sha256.as_bytes().as_slice())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| Error::LegacyImportStagingFailed)?
        .ok_or(Error::LegacyImportConflict)?;
        let import_state = parse_import_state(
            import_row
                .try_get::<String, _>("state")
                .map_err(|_| Error::InvalidLegacyImportJournal)?
                .as_str(),
        )?;
        let import_updated_at = import_row
            .try_get::<i64, _>("updated_at_ms")
            .map_err(|_| Error::InvalidLegacyImportJournal)?;
        if updated_at < import_updated_at
            || !matches!(
                import_state,
                LegacyImportState::Classified
                    | LegacyImportState::Staging
                    | LegacyImportState::Ready
            )
        {
            return Err(Error::LegacyImportConflict);
        }
        let member_row = sqlx::query(
            "SELECT state, resume_cursor, staged_row_count, updated_at_ms
             FROM radroots_runtime_legacy_import_members
             WHERE import_id = ? AND source_kind = 'outbox'",
        )
        .bind(classified.import_id().as_bytes().as_slice())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| Error::LegacyImportStagingFailed)?
        .ok_or(Error::LegacyImportConflict)?;
        let member_state = parse_member_state(
            member_row
                .try_get::<String, _>("state")
                .map_err(|_| Error::InvalidLegacyImportJournal)?
                .as_str(),
        )?;
        let durable_cursor = member_row
            .try_get::<Option<Vec<u8>>, _>("resume_cursor")
            .map_err(|_| Error::InvalidLegacyImportJournal)?;
        let (table, after) = decode_outbox_stage_cursor(durable_cursor.as_deref())?;
        let staged_row_count = u64::try_from(
            member_row
                .try_get::<i64, _>("staged_row_count")
                .map_err(|_| Error::InvalidLegacyImportJournal)?,
        )
        .map_err(|_| Error::InvalidLegacyImportJournal)?;
        let member_updated_at = member_row
            .try_get::<i64, _>("updated_at_ms")
            .map_err(|_| Error::InvalidLegacyImportJournal)?;
        if updated_at < member_updated_at {
            return Err(Error::LegacyImportConflict);
        }
        if member_state == LegacyImportMemberState::Ready {
            let cursor = durable_cursor
                .as_deref()
                .map(decode_exact_outbox_stage_cursor)
                .transpose()?
                .ok_or(Error::InvalidLegacyImportJournal)?;
            transaction
                .commit()
                .await
                .map_err(|_| Error::LegacyImportStagingFailed)?;
            return Ok(LegacyOutboxStagePage {
                table: LegacyOutboxTable::DeliveryAttempts,
                staged_rows: 0,
                staged_row_count,
                resume_cursor: cursor,
                complete: true,
            });
        }
        if !matches!(
            member_state,
            LegacyImportMemberState::Pending | LegacyImportMemberState::Staging
        ) {
            return Err(Error::LegacyImportConflict);
        }
        if import_state == LegacyImportState::Classified {
            sqlx::query(
                "UPDATE radroots_runtime_legacy_imports
                 SET state = 'staging', updated_at_ms = ? WHERE import_id = ?",
            )
            .bind(updated_at)
            .bind(classified.import_id().as_bytes().as_slice())
            .execute(&mut *transaction)
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        }
        if member_state == LegacyImportMemberState::Pending {
            sqlx::query(
                "UPDATE radroots_runtime_legacy_import_members
                 SET state = 'staging', updated_at_ms = ?
                 WHERE import_id = ? AND source_kind = 'outbox'",
            )
            .bind(updated_at)
            .bind(classified.import_id().as_bytes().as_slice())
            .execute(&mut *transaction)
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        }
        let mut source = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&source_path)
                .read_only(true),
        )
        .await
        .map_err(|_| Error::LegacyImportStagingFailed)?;
        let rows = sqlx::query(outbox_stage_query(table))
            .bind(after)
            .bind(i64::from(limit) + 1)
            .fetch_all(&mut source)
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        source
            .close()
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        let table_complete = rows.len() <= usize::from(limit);
        let mut last_id = after;
        let mut newly_staged = 0_u16;
        for row in rows.into_iter().take(usize::from(limit)) {
            let legacy_id = row
                .try_get::<i64, _>("legacy_id")
                .map_err(|_| Error::LegacyImportStagingFailed)?;
            let parent_legacy_id = row
                .try_get::<Option<i64>, _>("parent_legacy_id")
                .map_err(|_| Error::LegacyImportStagingFailed)?;
            let related_legacy_id = row
                .try_get::<Option<i64>, _>("related_legacy_id")
                .map_err(|_| Error::LegacyImportStagingFailed)?;
            let record_json = row
                .try_get::<Vec<u8>, _>("record_json")
                .map_err(|_| Error::LegacyImportStagingFailed)?;
            if legacy_id <= last_id || record_json.is_empty() {
                return Err(Error::LegacyImportRowInvalid {
                    source_kind: "outbox",
                    legacy_sequence: legacy_id,
                });
            }
            sqlx::query(
                "INSERT INTO radroots_runtime_legacy_outbox_staging(
                    import_id, source_kind, table_kind, legacy_id,
                    parent_legacy_id, related_legacy_id, record_json
                 ) VALUES (?, 'outbox', ?, ?, ?, ?, ?)",
            )
            .bind(classified.import_id().as_bytes().as_slice())
            .bind(table.as_str())
            .bind(legacy_id)
            .bind(parent_legacy_id)
            .bind(related_legacy_id)
            .bind(record_json)
            .execute(&mut *transaction)
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
            last_id = legacy_id;
            newly_staged += 1;
        }
        let complete = table_complete && table.next().is_none();
        let next_cursor = if table_complete {
            encode_outbox_stage_cursor(
                table.next().unwrap_or(table),
                if complete { last_id } else { 0 },
            )
        } else {
            encode_outbox_stage_cursor(table, last_id)
        };
        let total = staged_row_count
            .checked_add(u64::from(newly_staged))
            .ok_or(Error::LegacyImportStagingFailed)?;
        sqlx::query(
            "UPDATE radroots_runtime_legacy_import_members
             SET state = ?, resume_cursor = ?, staged_row_count = ?, updated_at_ms = ?
             WHERE import_id = ? AND source_kind = 'outbox'",
        )
        .bind(if complete { "ready" } else { "staging" })
        .bind(next_cursor.as_slice())
        .bind(i64::try_from(total).map_err(|_| Error::LegacyImportStagingFailed)?)
        .bind(updated_at)
        .bind(classified.import_id().as_bytes().as_slice())
        .execute(&mut *transaction)
        .await
        .map_err(|_| Error::LegacyImportStagingFailed)?;
        let pending_members = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM radroots_runtime_legacy_import_members
             WHERE import_id = ? AND state != 'ready'",
        )
        .bind(classified.import_id().as_bytes().as_slice())
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| Error::LegacyImportStagingFailed)?;
        sqlx::query(
            "UPDATE radroots_runtime_legacy_imports SET state = ?, updated_at_ms = ?
             WHERE import_id = ?",
        )
        .bind(if pending_members == 0 {
            "ready"
        } else {
            "staging"
        })
        .bind(updated_at)
        .bind(classified.import_id().as_bytes().as_slice())
        .execute(&mut *transaction)
        .await
        .map_err(|_| Error::LegacyImportStagingFailed)?;
        transaction
            .commit()
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        Ok(LegacyOutboxStagePage {
            table,
            staged_rows: newly_staged,
            staged_row_count: total,
            resume_cursor: next_cursor,
            complete,
        })
    }

    /// Stages one recoverable page of an exact predecessor private store.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn stage_legacy_private(
        &self,
        classified: &ClassifiedLegacyImport,
        limit: u16,
        updated_at_unix_ms: u64,
    ) -> Result<LegacyPrivateStagePage, Error> {
        self.require_legacy_import_writer(classified.target_generation())?;
        if limit == 0 || limit > LEGACY_STAGE_PAGE_LIMIT_MAX || updated_at_unix_ms == 0 {
            return Err(Error::InvalidLegacyImportStageRequest);
        }
        verify_prepared_evidence(&classified.prepared).await?;
        let classification_sha256 = classification_digest(classified);
        let journal = self
            .legacy_import_journal(classified.import_id())
            .await?
            .ok_or(Error::InvalidLegacyImportJournal)?;
        if !journal_matches_classified(&journal, classified, classification_sha256)
            || !classified.sources().iter().any(|source| {
                source.kind() == LegacySourceKind::Private
                    && source.schema() == LegacySchema::PrivateV1
            })
        {
            return Err(Error::LegacyImportConflict);
        }
        let member = journal
            .members()
            .iter()
            .find(|member| member.classification().kind() == LegacySourceKind::Private)
            .ok_or(Error::LegacyImportConflict)?;
        let (table, after) = decode_private_stage_cursor(member.resume_cursor())?;
        if member.state() == LegacyImportMemberState::Ready {
            return Ok(LegacyPrivateStagePage {
                table: LegacyPrivateTable::RotationProgress,
                staged_rows: 0,
                staged_row_count: member.staged_row_count(),
                resume_cursor: member
                    .resume_cursor()
                    .ok_or(Error::InvalidLegacyImportJournal)?
                    .to_vec(),
                complete: true,
            });
        }
        if !matches!(
            member.state(),
            LegacyImportMemberState::Pending | LegacyImportMemberState::Staging
        ) || updated_at_unix_ms < member.updated_at_unix_ms()
            || updated_at_unix_ms < journal.updated_at_unix_ms()
        {
            return Err(Error::LegacyImportConflict);
        }
        let updated_at = i64::try_from(updated_at_unix_ms)
            .map_err(|_| Error::InvalidLegacyImportStageRequest)?;
        if member.state() == LegacyImportMemberState::Pending {
            let mut tx = self
                .pool
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(|_| Error::LegacyImportStagingFailed)?;
            if journal.state() == LegacyImportState::Classified {
                sqlx::query("UPDATE radroots_runtime_legacy_imports SET state = 'staging', updated_at_ms = ? WHERE import_id = ? AND state = 'classified'")
                    .bind(updated_at).bind(classified.import_id().as_bytes().as_slice()).execute(&mut *tx).await.map_err(|_| Error::LegacyImportStagingFailed)?;
            }
            let changed = sqlx::query("UPDATE radroots_runtime_legacy_import_members SET state = 'staging', updated_at_ms = ? WHERE import_id = ? AND source_kind = 'private' AND state = 'pending'")
                .bind(updated_at).bind(classified.import_id().as_bytes().as_slice()).execute(&mut *tx).await.map_err(|_| Error::LegacyImportStagingFailed)?;
            if changed.rows_affected() != 1 {
                return Err(Error::LegacyImportConflict);
            }
            tx.commit()
                .await
                .map_err(|_| Error::LegacyImportStagingFailed)?;
        }
        let snapshot = classified
            .prepared
            .snapshots()
            .iter()
            .find(|snapshot| snapshot.kind() == LegacySourceKind::Private)
            .ok_or(Error::LegacyImportConflict)?;
        let mut source = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(classified.bundle_path().join(snapshot.relative_path()))
                .read_only(true),
        )
        .await
        .map_err(|_| Error::LegacyImportStagingFailed)?;
        let rows = sqlx::query(private_stage_query(table))
            .bind(after.as_str())
            .bind(i64::from(limit) + 1)
            .fetch_all(&mut source)
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        source
            .close()
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        let table_complete = rows.len() <= usize::from(limit);
        let page_rows = rows
            .into_iter()
            .take(usize::from(limit))
            .collect::<Vec<_>>();
        let mut last_key = after.clone();
        let mut private_tx = self
            .private_pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        for row in &page_rows {
            let key = row
                .try_get::<String, _>("key_cursor")
                .map_err(|_| Error::LegacyImportStagingFailed)?;
            let parent = row
                .try_get::<Option<i64>, _>("parent_key_version")
                .map_err(|_| Error::LegacyImportStagingFailed)?;
            let record = row
                .try_get::<Vec<u8>, _>("record_json")
                .map_err(|_| Error::LegacyImportStagingFailed)?;
            if key <= last_key || key.len() > 1024 || record.is_empty() {
                return Err(Error::LegacyImportRowInvalid {
                    source_kind: "private",
                    legacy_sequence: 0,
                });
            }
            sqlx::query("INSERT OR IGNORE INTO radroots_private_legacy_import_staging(import_id, table_kind, key_cursor, parent_key_version, record_json) VALUES (?, ?, ?, ?, ?)")
                .bind(classified.import_id().as_bytes().as_slice()).bind(table.as_str()).bind(&key).bind(parent).bind(&record).execute(&mut *private_tx).await.map_err(|_| Error::LegacyImportStagingFailed)?;
            let existing = sqlx::query("SELECT parent_key_version, record_json FROM radroots_private_legacy_import_staging WHERE import_id = ? AND table_kind = ? AND key_cursor = ?")
                .bind(classified.import_id().as_bytes().as_slice()).bind(table.as_str()).bind(&key).fetch_one(&mut *private_tx).await.map_err(|_| Error::LegacyImportStagingFailed)?;
            if existing
                .try_get::<Option<i64>, _>("parent_key_version")
                .map_err(|_| Error::LegacyImportStagingFailed)?
                != parent
                || existing
                    .try_get::<Vec<u8>, _>("record_json")
                    .map_err(|_| Error::LegacyImportStagingFailed)?
                    != record
            {
                return Err(Error::LegacyImportConflict);
            }
            last_key = key;
        }
        private_tx
            .commit()
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        let complete = table_complete && table.next().is_none();
        let next_cursor = if table_complete {
            encode_private_stage_cursor(
                table.next().unwrap_or(table),
                if complete { &last_key } else { "" },
            )
        } else {
            encode_private_stage_cursor(table, &last_key)
        };
        let total = member
            .staged_row_count()
            .checked_add(
                u64::try_from(page_rows.len()).map_err(|_| Error::LegacyImportStagingFailed)?,
            )
            .ok_or(Error::LegacyImportStagingFailed)?;
        let mut runtime_tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        let changed = sqlx::query("UPDATE radroots_runtime_legacy_import_members SET state = ?, resume_cursor = ?, staged_row_count = ?, updated_at_ms = ? WHERE import_id = ? AND source_kind = 'private' AND staged_row_count = ? AND resume_cursor IS ?")
            .bind(if complete { "ready" } else { "staging" }).bind(&next_cursor).bind(i64::try_from(total).map_err(|_| Error::LegacyImportStagingFailed)?).bind(updated_at).bind(classified.import_id().as_bytes().as_slice()).bind(i64::try_from(member.staged_row_count()).map_err(|_| Error::LegacyImportStagingFailed)?).bind(member.resume_cursor()).execute(&mut *runtime_tx).await.map_err(|_| Error::LegacyImportStagingFailed)?;
        if changed.rows_affected() != 1 {
            return Err(Error::LegacyImportConflict);
        }
        let pending = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM radroots_runtime_legacy_import_members WHERE import_id = ? AND state != 'ready'").bind(classified.import_id().as_bytes().as_slice()).fetch_one(&mut *runtime_tx).await.map_err(|_| Error::LegacyImportStagingFailed)?;
        sqlx::query("UPDATE radroots_runtime_legacy_imports SET state = ?, updated_at_ms = ? WHERE import_id = ?")
            .bind(if pending == 0 { "ready" } else { "staging" }).bind(updated_at).bind(classified.import_id().as_bytes().as_slice()).execute(&mut *runtime_tx).await.map_err(|_| Error::LegacyImportStagingFailed)?;
        runtime_tx
            .commit()
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        Ok(LegacyPrivateStagePage {
            table,
            staged_rows: u16::try_from(page_rows.len())
                .map_err(|_| Error::LegacyImportStagingFailed)?,
            staged_row_count: total,
            resume_cursor: next_cursor,
            complete,
        })
    }

    /// Revalidates and describes a Studio predecessor snapshot for its host.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn prepare_legacy_studio_handoff(
        &self,
        classified: &ClassifiedLegacyImport,
    ) -> Result<LegacyStudioHandoff, Error> {
        self.require_legacy_import_writer(classified.target_generation())?;
        verify_prepared_evidence(&classified.prepared).await?;
        let classification_sha256 = classification_digest(classified);
        let journal = self
            .legacy_import_journal(classified.import_id())
            .await?
            .ok_or(Error::InvalidLegacyImportJournal)?;
        if !journal_matches_classified(&journal, classified, classification_sha256) {
            return Err(Error::LegacyImportConflict);
        }
        let classification = classified
            .sources()
            .iter()
            .find(|source| source.kind() == LegacySourceKind::Studio)
            .filter(|source| {
                source.schema() == LegacySchema::StudioV1HostHandoff
                    && source.schema().disposition() == LegacyImportDisposition::HostHandoff
            })
            .ok_or(Error::LegacyImportConflict)?;
        let member = journal
            .members()
            .iter()
            .find(|member| member.classification().kind() == LegacySourceKind::Studio)
            .ok_or(Error::LegacyImportConflict)?;
        if !matches!(
            member.state(),
            LegacyImportMemberState::Pending
                | LegacyImportMemberState::Staging
                | LegacyImportMemberState::Ready
        ) || member.staged_row_count() != 0
        {
            return Err(Error::LegacyImportConflict);
        }
        let snapshot = classified
            .prepared
            .snapshots()
            .iter()
            .find(|snapshot| snapshot.kind() == LegacySourceKind::Studio)
            .ok_or(Error::LegacyImportConflict)?;
        let evidence_path = classified.bundle_path().join(snapshot.relative_path());
        Ok(LegacyStudioHandoff {
            import_id: classified.import_id(),
            evidence_path,
            byte_length: snapshot.byte_length(),
            source_sha256: snapshot.sha256(),
            catalog_sha256: classification.catalog_sha256(),
            handoff_sha256: studio_handoff_digest(classified, snapshot, classification),
        })
    }

    /// Records an exact host-owned Studio handoff acknowledgement without importing it.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn acknowledge_legacy_studio_handoff(
        &self,
        classified: &ClassifiedLegacyImport,
        receipt: LegacyStudioHandoffReceipt,
        updated_at_unix_ms: u64,
    ) -> Result<LegacyImportJournal, Error> {
        if updated_at_unix_ms == 0 {
            return Err(Error::InvalidLegacyImportStageRequest);
        }
        let handoff = self.prepare_legacy_studio_handoff(classified).await?;
        if receipt.handoff_sha256() != handoff.handoff_sha256() {
            return Err(Error::LegacyImportConflict);
        }
        let receipt_cursor = studio_handoff_receipt_cursor(receipt);
        let journal = self
            .legacy_import_journal(classified.import_id())
            .await?
            .ok_or(Error::InvalidLegacyImportJournal)?;
        let member = journal
            .members()
            .iter()
            .find(|member| member.classification().kind() == LegacySourceKind::Studio)
            .ok_or(Error::LegacyImportConflict)?;
        if member.state() == LegacyImportMemberState::Ready {
            return if member.resume_cursor() == Some(receipt_cursor.as_slice()) {
                Ok(journal)
            } else {
                Err(Error::LegacyImportConflict)
            };
        }
        if !matches!(
            member.state(),
            LegacyImportMemberState::Pending | LegacyImportMemberState::Staging
        ) || member.resume_cursor().is_some()
            || member.staged_row_count() != 0
            || updated_at_unix_ms < member.updated_at_unix_ms()
            || updated_at_unix_ms < journal.updated_at_unix_ms()
        {
            return Err(Error::LegacyImportConflict);
        }
        let updated_at = i64::try_from(updated_at_unix_ms)
            .map_err(|_| Error::InvalidLegacyImportStageRequest)?;
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        if journal.state() == LegacyImportState::Classified {
            let changed = sqlx::query("UPDATE radroots_runtime_legacy_imports SET state = 'staging', updated_at_ms = ? WHERE import_id = ? AND state = 'classified'")
                .bind(updated_at).bind(classified.import_id().as_bytes().as_slice()).execute(&mut *transaction).await.map_err(|_| Error::LegacyImportStagingFailed)?;
            if changed.rows_affected() != 1 {
                return Err(Error::LegacyImportConflict);
            }
        }
        if member.state() == LegacyImportMemberState::Pending {
            let changed = sqlx::query("UPDATE radroots_runtime_legacy_import_members SET state = 'staging', updated_at_ms = ? WHERE import_id = ? AND source_kind = 'studio' AND state = 'pending' AND resume_cursor IS NULL AND staged_row_count = 0")
                .bind(updated_at).bind(classified.import_id().as_bytes().as_slice()).execute(&mut *transaction).await.map_err(|_| Error::LegacyImportStagingFailed)?;
            if changed.rows_affected() != 1 {
                return Err(Error::LegacyImportConflict);
            }
        }
        let changed = sqlx::query("UPDATE radroots_runtime_legacy_import_members SET state = 'ready', resume_cursor = ?, updated_at_ms = ? WHERE import_id = ? AND source_kind = 'studio' AND state = 'staging' AND resume_cursor IS NULL AND staged_row_count = 0")
            .bind(receipt_cursor.as_slice()).bind(updated_at).bind(classified.import_id().as_bytes().as_slice()).execute(&mut *transaction).await.map_err(|_| Error::LegacyImportStagingFailed)?;
        if changed.rows_affected() != 1 {
            return Err(Error::LegacyImportConflict);
        }
        let pending = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM radroots_runtime_legacy_import_members WHERE import_id = ? AND state != 'ready'")
            .bind(classified.import_id().as_bytes().as_slice()).fetch_one(&mut *transaction).await.map_err(|_| Error::LegacyImportStagingFailed)?;
        if pending == 0 {
            sqlx::query("UPDATE radroots_runtime_legacy_imports SET state = 'ready', updated_at_ms = ? WHERE import_id = ? AND state = 'staging'")
                .bind(updated_at).bind(classified.import_id().as_bytes().as_slice()).execute(&mut *transaction).await.map_err(|_| Error::LegacyImportStagingFailed)?;
        }
        transaction
            .commit()
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        self.legacy_import_journal(classified.import_id())
            .await?
            .ok_or(Error::InvalidLegacyImportJournal)
    }

    /// Proves every classified source is completely staged or acknowledged.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn validate_legacy_import(
        &self,
        classified: &ClassifiedLegacyImport,
    ) -> Result<LegacyImportValidation, Error> {
        self.require_legacy_import_writer(classified.target_generation())?;
        verify_prepared_evidence(&classified.prepared).await?;
        let classification_sha256 = classification_digest(classified);
        let journal = self
            .legacy_import_journal(classified.import_id())
            .await?
            .ok_or(Error::InvalidLegacyImportJournal)?;
        if !journal_matches_classified(&journal, classified, classification_sha256)
            || journal.state() != LegacyImportState::Ready
            || journal.members().iter().any(|member| {
                member.state() != LegacyImportMemberState::Ready
                    || (member.classification().kind() == LegacySourceKind::Studio
                        && member.staged_row_count() != 0)
            })
        {
            return Err(Error::LegacyImportConflict);
        }

        let mut source_counts = Vec::with_capacity(classified.sources().len());
        for source in classified.sources() {
            source_counts.push((
                source.kind(),
                source_import_row_count(classified, source.kind()).await?,
            ));
        }

        let mut runtime_tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        let mut private_tx = self
            .private_pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        let current_state = sqlx::query_scalar::<_, String>(
            "SELECT state FROM radroots_runtime_legacy_imports WHERE import_id = ?",
        )
        .bind(classified.import_id().as_bytes().as_slice())
        .fetch_one(&mut *runtime_tx)
        .await
        .map_err(|_| Error::LegacyImportStagingFailed)?;
        let member_rows = sqlx::query(
            "SELECT source_kind, state, resume_cursor, staged_row_count
             FROM radroots_runtime_legacy_import_members
             WHERE import_id = ? ORDER BY source_kind",
        )
        .bind(classified.import_id().as_bytes().as_slice())
        .fetch_all(&mut *runtime_tx)
        .await
        .map_err(|_| Error::LegacyImportStagingFailed)?;
        if current_state != "ready" || member_rows.len() != classified.sources().len() {
            return Err(Error::LegacyImportConflict);
        }

        let mut digest = Sha256::new();
        for field in [
            b"radroots.legacy.import.validation.v1".as_slice(),
            classified.import_id().as_bytes().as_slice(),
            classified.target_generation().as_bytes().as_slice(),
            classified.prepared.manifest_sha256().as_bytes().as_slice(),
            classification_sha256.as_bytes().as_slice(),
        ] {
            update_framed_digest(&mut digest, field)?;
        }
        let mut imported_row_count = 0_u64;
        for row in member_rows {
            let kind_value = row
                .try_get::<String, _>("source_kind")
                .map_err(|_| Error::InvalidLegacyImportJournal)?;
            let kind = parse_source_kind(kind_value.as_str())?;
            let state = row
                .try_get::<String, _>("state")
                .map_err(|_| Error::InvalidLegacyImportJournal)?;
            let cursor = row
                .try_get::<Option<Vec<u8>>, _>("resume_cursor")
                .map_err(|_| Error::InvalidLegacyImportJournal)?;
            let staged = u64::try_from(
                row.try_get::<i64, _>("staged_row_count")
                    .map_err(|_| Error::InvalidLegacyImportJournal)?,
            )
            .map_err(|_| Error::InvalidLegacyImportJournal)?;
            let source_count = source_counts
                .iter()
                .find_map(|(source_kind, count)| (*source_kind == kind).then_some(*count))
                .ok_or(Error::LegacyImportConflict)?;
            if state != "ready"
                || staged != source_count
                || cursor.is_none()
                || (kind == LegacySourceKind::Studio && staged != 0)
            {
                return Err(Error::LegacyImportConflict);
            }
            update_framed_digest(&mut digest, kind_value.as_bytes())?;
            update_framed_digest(&mut digest, cursor.as_deref().unwrap_or_default())?;
            update_framed_digest(&mut digest, &staged.to_be_bytes())?;
            imported_row_count = imported_row_count
                .checked_add(staged)
                .ok_or(Error::LegacyImportStagingFailed)?;
        }
        hash_runtime_legacy_staging(&mut runtime_tx, classified.import_id(), &mut digest).await?;
        hash_private_legacy_staging(&mut private_tx, classified.import_id(), &mut digest).await?;
        runtime_tx
            .commit()
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        private_tx
            .commit()
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        Ok(LegacyImportValidation {
            imported_row_count,
            validation_sha256: MemberDigest::new(digest.finalize().into()),
        })
    }

    /// Seals validated legacy staging through a private-first recovery protocol.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn finalize_legacy_import(
        &self,
        classified: &ClassifiedLegacyImport,
        expected: LegacyImportValidation,
        completed_at_unix_ms: u64,
    ) -> Result<LegacyImportCommitReceipt, Error> {
        self.require_legacy_import_writer(classified.target_generation())?;
        if completed_at_unix_ms == 0 {
            return Err(Error::InvalidLegacyImportStageRequest);
        }
        let classification_sha256 = classification_digest(classified);
        let journal = self
            .legacy_import_journal(classified.import_id())
            .await?
            .ok_or(Error::InvalidLegacyImportJournal)?;
        if !journal_matches_classified(&journal, classified, classification_sha256) {
            return Err(Error::LegacyImportConflict);
        }
        if journal.state() == LegacyImportState::Complete {
            return self
                .completed_legacy_import_receipt(classified.import_id(), expected)
                .await;
        }
        if journal.state() != LegacyImportState::Ready
            || completed_at_unix_ms < journal.updated_at_unix_ms()
        {
            return Err(Error::LegacyImportConflict);
        }
        let actual = self.validate_legacy_import(classified).await?;
        if actual != expected {
            return Err(Error::LegacyImportConflict);
        }
        let completed_at = i64::try_from(completed_at_unix_ms)
            .map_err(|_| Error::InvalidLegacyImportStageRequest)?;
        let imported_row_count = i64::try_from(expected.imported_row_count())
            .map_err(|_| Error::LegacyImportStagingFailed)?;

        let mut private_tx = self
            .private_pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        sqlx::query("INSERT OR IGNORE INTO radroots_private_legacy_import_commits(import_id, validation_sha256, imported_row_count, committed_at_ms) VALUES (?, ?, ?, ?)")
            .bind(classified.import_id().as_bytes().as_slice()).bind(expected.validation_sha256().as_bytes().as_slice()).bind(imported_row_count).bind(completed_at).execute(&mut *private_tx).await.map_err(|_| Error::LegacyImportStagingFailed)?;
        let private_record = sqlx::query("SELECT validation_sha256, imported_row_count, committed_at_ms FROM radroots_private_legacy_import_commits WHERE import_id = ?")
            .bind(classified.import_id().as_bytes().as_slice()).fetch_one(&mut *private_tx).await.map_err(|_| Error::LegacyImportStagingFailed)?;
        let private_committed_at = private_record
            .try_get::<i64, _>("committed_at_ms")
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        if decode_digest(
            private_record
                .try_get("validation_sha256")
                .map_err(|_| Error::LegacyImportStagingFailed)?,
        )? != expected.validation_sha256()
            || private_record
                .try_get::<i64, _>("imported_row_count")
                .map_err(|_| Error::LegacyImportStagingFailed)?
                != imported_row_count
            || private_committed_at
                < i64::try_from(journal.updated_at_unix_ms())
                    .map_err(|_| Error::LegacyImportStagingFailed)?
        {
            return Err(Error::LegacyImportConflict);
        }
        private_tx
            .commit()
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        let completed_at = private_committed_at;
        let completed_at_unix_ms =
            u64::try_from(completed_at).map_err(|_| Error::LegacyImportStagingFailed)?;

        let mut runtime_tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        sqlx::query("INSERT INTO radroots_runtime_legacy_import_commits(import_id, validation_sha256, imported_row_count, completed_at_ms) VALUES (?, ?, ?, ?)")
            .bind(classified.import_id().as_bytes().as_slice()).bind(expected.validation_sha256().as_bytes().as_slice()).bind(imported_row_count).bind(completed_at).execute(&mut *runtime_tx).await.map_err(|_| Error::LegacyImportStagingFailed)?;
        let changed = sqlx::query("UPDATE radroots_runtime_legacy_imports SET state = 'committing', updated_at_ms = ? WHERE import_id = ? AND state = 'ready'")
            .bind(completed_at).bind(classified.import_id().as_bytes().as_slice()).execute(&mut *runtime_tx).await.map_err(|_| Error::LegacyImportStagingFailed)?;
        if changed.rows_affected() != 1 {
            return Err(Error::LegacyImportConflict);
        }
        let changed = sqlx::query("UPDATE radroots_runtime_legacy_import_members SET state = 'complete', updated_at_ms = ? WHERE import_id = ? AND state = 'ready'")
            .bind(completed_at).bind(classified.import_id().as_bytes().as_slice()).execute(&mut *runtime_tx).await.map_err(|_| Error::LegacyImportStagingFailed)?;
        if usize::try_from(changed.rows_affected()).map_err(|_| Error::LegacyImportStagingFailed)?
            != classified.sources().len()
        {
            return Err(Error::LegacyImportConflict);
        }
        let changed = sqlx::query("UPDATE radroots_runtime_legacy_imports SET state = 'complete', updated_at_ms = ?, completed_at_ms = ? WHERE import_id = ? AND state = 'committing'")
            .bind(completed_at).bind(completed_at).bind(classified.import_id().as_bytes().as_slice()).execute(&mut *runtime_tx).await.map_err(|_| Error::LegacyImportStagingFailed)?;
        if changed.rows_affected() != 1 {
            return Err(Error::LegacyImportConflict);
        }
        runtime_tx
            .commit()
            .await
            .map_err(|_| Error::LegacyImportStagingFailed)?;
        Ok(LegacyImportCommitReceipt {
            validation_sha256: expected.validation_sha256(),
            imported_row_count: expected.imported_row_count(),
            completed_at_unix_ms,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn completed_legacy_import_receipt(
        &self,
        import_id: LegacyImportId,
        expected: LegacyImportValidation,
    ) -> Result<LegacyImportCommitReceipt, Error> {
        let row = sqlx::query("SELECT validation_sha256, imported_row_count, completed_at_ms FROM radroots_runtime_legacy_import_commits WHERE import_id = ?")
            .bind(import_id.as_bytes().as_slice()).fetch_one(&self.pool).await.map_err(|_| Error::LegacyImportStagingFailed)?;
        let validation_sha256 = decode_digest(
            row.try_get("validation_sha256")
                .map_err(|_| Error::LegacyImportStagingFailed)?,
        )?;
        let imported_row_count = u64::try_from(
            row.try_get::<i64, _>("imported_row_count")
                .map_err(|_| Error::LegacyImportStagingFailed)?,
        )
        .map_err(|_| Error::LegacyImportStagingFailed)?;
        let completed_at_unix_ms = decode_positive_time(
            row.try_get("completed_at_ms")
                .map_err(|_| Error::LegacyImportStagingFailed)?,
        )?;
        if validation_sha256 != expected.validation_sha256()
            || imported_row_count != expected.imported_row_count()
        {
            return Err(Error::LegacyImportConflict);
        }
        let private_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM radroots_private_legacy_import_commits WHERE import_id = ? AND validation_sha256 = ? AND imported_row_count = ? AND committed_at_ms = ?")
            .bind(import_id.as_bytes().as_slice()).bind(validation_sha256.as_bytes().as_slice()).bind(i64::try_from(imported_row_count).map_err(|_| Error::LegacyImportStagingFailed)?).bind(i64::try_from(completed_at_unix_ms).map_err(|_| Error::LegacyImportStagingFailed)?).fetch_one(&self.private_pool).await.map_err(|_| Error::LegacyImportStagingFailed)?;
        if private_count != 1 {
            return Err(Error::LegacyImportConflict);
        }
        Ok(LegacyImportCommitReceipt {
            validation_sha256,
            imported_row_count,
            completed_at_unix_ms,
        })
    }

    fn require_legacy_import_writer(
        &self,
        target_generation: SourceGeneration,
    ) -> Result<(), Error> {
        self.lifecycle
            .require_open()
            .map_err(|_| Error::BackupBackendUnavailable)?;
        if self.mode != EventStoreMode::ReadWrite {
            return Err(Error::RestoreRequiresWritableStorage);
        }
        if target_generation != self.generation {
            return Err(Error::LegacyImportTargetMismatch);
        }
        Ok(())
    }
}

struct LegacyBackupLayout {
    staging: PathBuf,
    finalized: PathBuf,
}

impl LegacyBackupLayout {
    fn new(plan: &LegacyImportPlan) -> Self {
        let id = encode_id(plan.import_id().as_bytes());
        Self {
            staging: plan
                .backup_root()
                .join(format!(".radroots-legacy-import-{id}.staging")),
            finalized: plan
                .backup_root()
                .join(format!("radroots-legacy-import-{id}")),
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn create(&self) -> Result<(), Error> {
        for path in [&self.staging, &self.finalized] {
            if path
                .try_exists()
                .map_err(|source| Error::LegacyImportFilesystem {
                    operation: "inspect legacy import backup path",
                    source,
                })?
            {
                return Err(Error::LegacyImportBackupAlreadyExists(path.clone()));
            }
        }
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(&self.staging)
            .map_err(|source| Error::LegacyImportFilesystem {
                operation: "create legacy import staging bundle",
                source,
            })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn capture_legacy_source(source: &LegacySource, destination: &Path) -> Result<(), Error> {
    let destination_text = destination
        .to_str()
        .ok_or_else(|| Error::InvalidLegacySource(destination.to_path_buf()))?;
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(source.path())
            .read_only(true)
            .foreign_keys(true),
    )
    .await
    .map_err(|_| Error::LegacyImportBackupFailed {
        source_kind: source.kind().as_str(),
    })?;
    sqlx::query("VACUUM INTO ?")
        .bind(destination_text)
        .execute(&mut connection)
        .await
        .map_err(|_| Error::LegacyImportBackupFailed {
            source_kind: source.kind().as_str(),
        })?;
    connection
        .close()
        .await
        .map_err(|_| Error::LegacyImportBackupFailed {
            source_kind: source.kind().as_str(),
        })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(destination, fs::Permissions::from_mode(0o600)).map_err(|source| {
            Error::LegacyImportFilesystem {
                operation: "secure legacy import backup member",
                source,
            }
        })?;
    }
    verify_legacy_snapshot(source.kind(), destination).await
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn verify_legacy_snapshot(kind: LegacySourceKind, path: &Path) -> Result<(), Error> {
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .read_only(true)
            .foreign_keys(true),
    )
    .await
    .map_err(|_| Error::LegacyImportSourceInvalid {
        source_kind: kind.as_str(),
    })?;
    let quick_check = sqlx::query_scalar::<_, String>("PRAGMA quick_check")
        .fetch_all(&mut connection)
        .await
        .map_err(|_| Error::LegacyImportSourceInvalid {
            source_kind: kind.as_str(),
        })?;
    let foreign_key_violation = sqlx::query("PRAGMA foreign_key_check")
        .fetch_optional(&mut connection)
        .await
        .map_err(|_| Error::LegacyImportSourceInvalid {
            source_kind: kind.as_str(),
        })?
        .is_some();
    connection
        .close()
        .await
        .map_err(|_| Error::LegacyImportSourceInvalid {
            source_kind: kind.as_str(),
        })?;
    if quick_check == ["ok"] && !foreign_key_violation {
        Ok(())
    } else {
        Err(Error::LegacyImportSourceInvalid {
            source_kind: kind.as_str(),
        })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn snapshot(kind: LegacySourceKind, path: &Path) -> Result<LegacySourceSnapshot, Error> {
    let (byte_length, sha256) = file_digest(path)?;
    Ok(LegacySourceSnapshot {
        kind,
        relative_path: kind.backup_file_name().to_owned(),
        byte_length,
        sha256,
    })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn file_digest(path: &Path) -> Result<(u64, MemberDigest), Error> {
    let mut file = File::open(path).map_err(|source| Error::LegacyImportFilesystem {
        operation: "open legacy import evidence member",
        source,
    })?;
    file.sync_all()
        .map_err(|source| Error::LegacyImportFilesystem {
            operation: "sync legacy import evidence member",
            source,
        })?;
    let byte_length = file
        .metadata()
        .map_err(|source| Error::LegacyImportFilesystem {
            operation: "inspect legacy import evidence member",
            source,
        })?
        .len();
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 16 * 1_024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| Error::LegacyImportFilesystem {
                operation: "hash legacy import evidence member",
                source,
            })?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok((byte_length, MemberDigest::new(digest.finalize().into())))
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn verify_prepared_evidence(prepared: &PreparedLegacyImport) -> Result<(), Error> {
    let bundle_metadata = fs::symlink_metadata(prepared.bundle_path())
        .map_err(|_| Error::LegacyImportEvidenceInvalid)?;
    if !bundle_metadata.is_dir() || bundle_metadata.file_type().is_symlink() {
        return Err(Error::LegacyImportEvidenceInvalid);
    }
    let mut expected = BTreeSet::from([LEGACY_MANIFEST.to_owned()]);
    expected.extend(
        prepared
            .snapshots()
            .iter()
            .map(|snapshot| snapshot.relative_path().to_owned()),
    );
    let mut actual = BTreeSet::new();
    for entry in
        fs::read_dir(prepared.bundle_path()).map_err(|source| Error::LegacyImportFilesystem {
            operation: "read legacy import evidence bundle",
            source,
        })?
    {
        let entry = entry.map_err(|source| Error::LegacyImportFilesystem {
            operation: "read legacy import evidence entry",
            source,
        })?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| Error::LegacyImportEvidenceInvalid)?;
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|_| Error::LegacyImportEvidenceInvalid)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || !actual.insert(name) {
            return Err(Error::LegacyImportEvidenceInvalid);
        }
    }
    if actual != expected {
        return Err(Error::LegacyImportEvidenceInvalid);
    }
    let (manifest_length, manifest_digest) =
        file_digest(&prepared.bundle_path().join(LEGACY_MANIFEST))?;
    if manifest_length != prepared.manifest_byte_length()
        || manifest_digest != prepared.manifest_sha256()
    {
        return Err(Error::LegacyImportEvidenceInvalid);
    }
    for evidence in prepared.snapshots() {
        let path = prepared.bundle_path().join(evidence.relative_path());
        if snapshot(evidence.kind(), &path)? != *evidence {
            return Err(Error::LegacyImportEvidenceInvalid);
        }
        verify_legacy_snapshot(evidence.kind(), &path).await?;
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CatalogRow {
    object_type: String,
    name: String,
    table_name: String,
    sql: Option<String>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn classify_snapshot(
    kind: LegacySourceKind,
    path: &Path,
) -> Result<LegacySourceClassification, Error> {
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .read_only(true)
            .foreign_keys(true),
    )
    .await
    .map_err(|_| Error::LegacyImportSourceInvalid {
        source_kind: kind.as_str(),
    })?;
    let raw_user_version = sqlx::query_scalar::<_, i64>("PRAGMA user_version")
        .fetch_one(&mut connection)
        .await
        .map_err(|_| Error::LegacyImportSourceInvalid {
            source_kind: kind.as_str(),
        })?;
    let catalog = read_catalog(&mut connection, kind).await?;
    let (schema, governed_catalog) = match kind {
        LegacySourceKind::EventStore => {
            classify_event_store(&mut connection, raw_user_version, &catalog).await?
        }
        LegacySourceKind::Outbox => (
            classify_fixed_catalog(kind, raw_user_version, &catalog, 0, OUTBOX_CATALOG_SHA256)?,
            catalog,
        ),
        LegacySourceKind::Private => (
            classify_fixed_catalog(kind, raw_user_version, &catalog, 1, PRIVATE_CATALOG_SHA256)?,
            catalog,
        ),
        LegacySourceKind::Studio => (
            classify_fixed_catalog(kind, raw_user_version, &catalog, 0, STUDIO_CATALOG_SHA256)?,
            catalog,
        ),
    };
    let catalog_sha256 = catalog_fingerprint(&governed_catalog);
    connection
        .close()
        .await
        .map_err(|_| Error::LegacyImportSourceInvalid {
            source_kind: kind.as_str(),
        })?;
    let user_version = u32::try_from(raw_user_version)
        .map_err(|_| unsupported_schema(kind, raw_user_version, catalog_sha256))?;
    Ok(LegacySourceClassification {
        kind,
        schema,
        user_version,
        catalog_sha256,
    })
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn read_catalog(
    connection: &mut SqliteConnection,
    kind: LegacySourceKind,
) -> Result<Vec<CatalogRow>, Error> {
    sqlx::query("SELECT type, name, tbl_name, sql FROM main.sqlite_schema")
        .fetch_all(connection)
        .await
        .map_err(|_| Error::LegacyImportSourceInvalid {
            source_kind: kind.as_str(),
        })?
        .into_iter()
        .map(|row| {
            let name =
                row.try_get::<String, _>("name")
                    .map_err(|_| Error::LegacyImportSourceInvalid {
                        source_kind: kind.as_str(),
                    })?;
            Ok(CatalogRow {
                object_type: row
                    .try_get("type")
                    .map_err(|_| Error::LegacyImportSourceInvalid {
                        source_kind: kind.as_str(),
                    })?,
                table_name: row.try_get("tbl_name").map_err(|_| {
                    Error::LegacyImportSourceInvalid {
                        source_kind: kind.as_str(),
                    }
                })?,
                sql: row
                    .try_get("sql")
                    .map_err(|_| Error::LegacyImportSourceInvalid {
                        source_kind: kind.as_str(),
                    })?,
                name,
            })
        })
        .collect::<Result<Vec<_>, Error>>()
        .map(|catalog| {
            catalog
                .into_iter()
                .filter(|row| !row.name.to_ascii_lowercase().starts_with("sqlite_"))
                .collect()
        })
}

fn classify_fixed_catalog(
    kind: LegacySourceKind,
    user_version: i64,
    catalog: &[CatalogRow],
    expected_user_version: i64,
    expected_catalog_sha256: &str,
) -> Result<LegacySchema, Error> {
    let fingerprint = catalog_fingerprint(catalog);
    if user_version != expected_user_version
        || encode_digest(fingerprint.as_bytes()) != expected_catalog_sha256
    {
        return Err(unsupported_schema(kind, user_version, fingerprint));
    }
    Ok(match kind {
        LegacySourceKind::Outbox => LegacySchema::OutboxV1,
        LegacySourceKind::Private => LegacySchema::PrivateV1,
        LegacySourceKind::Studio => LegacySchema::StudioV1HostHandoff,
        LegacySourceKind::EventStore => return Err(Error::LegacyImportMigrationHistoryInvalid),
    })
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn classify_event_store(
    connection: &mut SqliteConnection,
    user_version: i64,
    catalog: &[CatalogRow],
) -> Result<(LegacySchema, Vec<CatalogRow>), Error> {
    if user_version != 0 {
        return Err(unsupported_schema(
            LegacySourceKind::EventStore,
            user_version,
            catalog_fingerprint(catalog),
        ));
    }
    let ledger_rows = catalog
        .iter()
        .filter(|row| {
            row.name.eq_ignore_ascii_case(EVENT_STORE_LEDGER)
                || row.table_name.eq_ignore_ascii_case(EVENT_STORE_LEDGER)
        })
        .collect::<Vec<_>>();
    let governed = catalog
        .iter()
        .filter(|row| !row.name.eq_ignore_ascii_case(EVENT_STORE_LEDGER))
        .cloned()
        .collect::<Vec<_>>();
    let fingerprint = catalog_fingerprint(&governed);
    let version = if ledger_rows.is_empty() {
        if encode_digest(fingerprint.as_bytes()) != EVENT_STORE_MIGRATIONS[0].schema_sha256 {
            return Err(unsupported_schema(
                LegacySourceKind::EventStore,
                user_version,
                fingerprint,
            ));
        }
        1
    } else {
        if ledger_rows.len() != 1 {
            return Err(Error::LegacyImportMigrationHistoryInvalid);
        }
        let ledger = ledger_rows[0];
        if ledger.object_type != "table"
            || ledger.name != EVENT_STORE_LEDGER
            || ledger.table_name != EVENT_STORE_LEDGER
            || ledger.sql.as_deref() != Some(EVENT_STORE_LEDGER_DDL)
        {
            return Err(Error::LegacyImportMigrationHistoryInvalid);
        }
        validate_event_history(connection).await?
    };
    let expected = EVENT_STORE_MIGRATIONS
        .get(usize::try_from(version - 1).map_err(|_| Error::LegacyImportMigrationHistoryInvalid)?)
        .ok_or(Error::LegacyImportMigrationHistoryInvalid)?;
    if encode_digest(fingerprint.as_bytes()) != expected.schema_sha256 {
        return Err(unsupported_schema(
            LegacySourceKind::EventStore,
            user_version,
            fingerprint,
        ));
    }
    let schema = match version {
        1 => LegacySchema::EventStoreV1,
        2 => LegacySchema::EventStoreV2,
        3 => LegacySchema::EventStoreV3,
        4 => LegacySchema::EventStoreV4,
        _ => return Err(Error::LegacyImportMigrationHistoryInvalid),
    };
    Ok((schema, governed))
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn validate_event_history(connection: &mut SqliteConnection) -> Result<u32, Error> {
    let rows = sqlx::query(
        "SELECT version, name, up_sha256, down_sha256, schema_sha256 FROM main.radroots_event_store_schema_migrations ORDER BY version",
    )
    .fetch_all(connection)
    .await
    .map_err(|_| Error::LegacyImportMigrationHistoryInvalid)?;
    if rows.is_empty() || rows.len() > EVENT_STORE_MIGRATIONS.len() {
        return Err(Error::LegacyImportMigrationHistoryInvalid);
    }
    for (index, row) in rows.iter().enumerate() {
        let expected = &EVENT_STORE_MIGRATIONS[index];
        if row.try_get::<i64, _>("version").ok() != Some(i64::from(expected.version))
            || row.try_get::<String, _>("name").ok().as_deref() != Some(expected.name)
            || row.try_get::<String, _>("up_sha256").ok().as_deref() != Some(expected.up_sha256)
            || row.try_get::<String, _>("down_sha256").ok().as_deref() != Some(expected.down_sha256)
            || row.try_get::<String, _>("schema_sha256").ok().as_deref()
                != Some(expected.schema_sha256)
        {
            return Err(Error::LegacyImportMigrationHistoryInvalid);
        }
    }
    u32::try_from(rows.len()).map_err(|_| Error::LegacyImportMigrationHistoryInvalid)
}

fn catalog_fingerprint(catalog: &[CatalogRow]) -> MemberDigest {
    let mut rows = catalog.to_vec();
    rows.sort_by(|left, right| {
        (
            left.object_type.as_bytes(),
            left.name.as_bytes(),
            left.table_name.as_bytes(),
            left.sql.as_deref().unwrap_or("").as_bytes(),
        )
            .cmp(&(
                right.object_type.as_bytes(),
                right.name.as_bytes(),
                right.table_name.as_bytes(),
                right.sql.as_deref().unwrap_or("").as_bytes(),
            ))
    });
    let mut digest = Sha256::new();
    for row in rows {
        for field in [
            row.object_type.as_str(),
            row.name.as_str(),
            row.table_name.as_str(),
            row.sql.as_deref().unwrap_or(""),
        ] {
            digest.update(field.as_bytes());
            digest.update([0]);
        }
    }
    MemberDigest::new(digest.finalize().into())
}

fn unsupported_schema(
    kind: LegacySourceKind,
    user_version: i64,
    catalog_sha256: MemberDigest,
) -> Error {
    Error::UnsupportedLegacySchema {
        source_kind: kind.as_str(),
        user_version,
        catalog_sha256: encode_digest(catalog_sha256.as_bytes()),
    }
}

fn classification_digest(classified: &ClassifiedLegacyImport) -> MemberDigest {
    let mut digest = Sha256::new();
    for field in [
        classified.import_id().as_bytes().as_slice(),
        classified.target_generation().as_bytes().as_slice(),
        classified.prepared.manifest_sha256().as_bytes().as_slice(),
    ] {
        digest.update(field);
        digest.update([0]);
    }
    for source in classified.sources() {
        for field in [
            source.kind().as_str().as_bytes(),
            source.schema().as_str().as_bytes(),
            source.schema().disposition().as_str().as_bytes(),
            source.catalog_sha256().as_bytes().as_slice(),
        ] {
            digest.update(field);
            digest.update([0]);
        }
        digest.update(source.user_version().to_be_bytes());
        digest.update([0]);
    }
    MemberDigest::new(digest.finalize().into())
}

fn studio_handoff_digest(
    classified: &ClassifiedLegacyImport,
    snapshot: &LegacySourceSnapshot,
    classification: &LegacySourceClassification,
) -> MemberDigest {
    let mut digest = Sha256::new();
    for field in [
        b"radroots.legacy.studio.handoff.v1".as_slice(),
        classified.import_id().as_bytes().as_slice(),
        classified.target_generation().as_bytes().as_slice(),
        classified.prepared.manifest_sha256().as_bytes().as_slice(),
        snapshot.relative_path().as_bytes(),
        snapshot.sha256().as_bytes().as_slice(),
        classification.catalog_sha256().as_bytes().as_slice(),
    ] {
        digest.update(field);
        digest.update([0]);
    }
    digest.update(snapshot.byte_length().to_be_bytes());
    MemberDigest::new(digest.finalize().into())
}

fn studio_handoff_receipt_cursor(receipt: LegacyStudioHandoffReceipt) -> [u8; 64] {
    let mut cursor = [0_u8; 64];
    cursor[..32].copy_from_slice(receipt.handoff_sha256().as_bytes());
    cursor[32..].copy_from_slice(receipt.host_commitment_sha256().as_bytes());
    cursor
}

fn update_framed_digest(digest: &mut Sha256, value: &[u8]) -> Result<(), Error> {
    let length = u64::try_from(value.len()).map_err(|_| Error::LegacyImportStagingFailed)?;
    digest.update(length.to_be_bytes());
    digest.update(value);
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn source_import_row_count(
    classified: &ClassifiedLegacyImport,
    kind: LegacySourceKind,
) -> Result<u64, Error> {
    if kind == LegacySourceKind::Studio {
        return Ok(0);
    }
    let snapshot = classified
        .prepared
        .snapshots()
        .iter()
        .find(|snapshot| snapshot.kind() == kind)
        .ok_or(Error::LegacyImportConflict)?;
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(classified.bundle_path().join(snapshot.relative_path()))
            .read_only(true),
    )
    .await
    .map_err(|_| Error::LegacyImportStagingFailed)?;
    let query = match kind {
        LegacySourceKind::EventStore => "SELECT COUNT(*) FROM event_envelopes",
        LegacySourceKind::Outbox => {
            "SELECT (SELECT COUNT(*) FROM outbox_operations)
                  + (SELECT COUNT(*) FROM outbox_event)
                  + (SELECT COUNT(*) FROM outbox_delivery_plan)
                  + (SELECT COUNT(*) FROM outbox_delivery_target)
                  + (SELECT COUNT(*) FROM outbox_delivery_attempt)"
        }
        LegacySourceKind::Private => {
            "SELECT (SELECT COUNT(*) FROM private_metadata)
                  + (SELECT COUNT(*) FROM wrapped_profile_key)
                  + (SELECT COUNT(*) FROM wrapped_signing_secret)
                  + (SELECT COUNT(*) FROM private_farm_location)
                  + (SELECT COUNT(*) FROM private_trade_artifacts)
                  + (SELECT COUNT(*) FROM cursor_hmac_key)
                  + (SELECT COUNT(*) FROM nip46_session_private)
                  + (SELECT COUNT(*) FROM key_rotation_progress)"
        }
        LegacySourceKind::Studio => unreachable!("Studio is host-owned"),
    };
    let count = sqlx::query_scalar::<_, i64>(query)
        .fetch_one(&mut connection)
        .await
        .map_err(|_| Error::LegacyImportStagingFailed)?;
    connection
        .close()
        .await
        .map_err(|_| Error::LegacyImportStagingFailed)?;
    u64::try_from(count).map_err(|_| Error::LegacyImportStagingFailed)
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn hash_runtime_legacy_staging(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    import_id: LegacyImportId,
    digest: &mut Sha256,
) -> Result<(), Error> {
    update_framed_digest(digest, b"runtime_events")?;
    let event_rows = sqlx::query_scalar::<_, String>(
        "SELECT json_array(legacy_sequence, hex(event_id), hex(signed_event),
                 legacy_verification_status, legacy_contract_status,
                 legacy_projection_eligible, legacy_inserted_at_ms,
                 legacy_updated_at_ms)
         FROM radroots_runtime_legacy_event_staging
         WHERE import_id = ? ORDER BY legacy_sequence",
    )
    .bind(import_id.as_bytes().as_slice())
    .fetch_all(&mut **transaction)
    .await
    .map_err(|_| Error::LegacyImportStagingFailed)?;
    for row in event_rows {
        update_framed_digest(digest, row.as_bytes())?;
    }

    update_framed_digest(digest, b"runtime_outbox")?;
    let outbox_rows = sqlx::query_scalar::<_, String>(
        "SELECT json_array(table_kind, legacy_id, parent_legacy_id,
                 related_legacy_id, hex(record_json))
         FROM radroots_runtime_legacy_outbox_staging
         WHERE import_id = ? ORDER BY table_kind, legacy_id",
    )
    .bind(import_id.as_bytes().as_slice())
    .fetch_all(&mut **transaction)
    .await
    .map_err(|_| Error::LegacyImportStagingFailed)?;
    for row in outbox_rows {
        update_framed_digest(digest, row.as_bytes())?;
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn hash_private_legacy_staging(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    import_id: LegacyImportId,
    digest: &mut Sha256,
) -> Result<(), Error> {
    update_framed_digest(digest, b"private_records")?;
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT json_array(table_kind, key_cursor, parent_key_version,
                 hex(record_json))
         FROM radroots_private_legacy_import_staging
         WHERE import_id = ? ORDER BY table_kind, key_cursor",
    )
    .bind(import_id.as_bytes().as_slice())
    .fetch_all(&mut **transaction)
    .await
    .map_err(|_| Error::LegacyImportStagingFailed)?;
    for row in rows {
        update_framed_digest(digest, row.as_bytes())?;
    }
    Ok(())
}

fn outbox_stage_query(table: LegacyOutboxTable) -> &'static str {
    match table {
        LegacyOutboxTable::Operations => {
            "SELECT operation_id AS legacy_id, NULL AS parent_legacy_id,
                    NULL AS related_legacy_id,
                    CAST(json_array(operation_kind, expected_pubkey, semantic_scope,
                      trade_id, mutation_id, canonical_payload_sha256, idempotency_key,
                      operation_idempotency_digest, status, created_at_ms, updated_at_ms)
                      AS BLOB) AS record_json
             FROM outbox_operations WHERE operation_id > ?
             ORDER BY operation_id LIMIT ?"
        }
        LegacyOutboxTable::Events => {
            "SELECT outbox_event_id AS legacy_id, operation_id AS parent_legacy_id,
                    NULL AS related_legacy_id,
                    CAST(json_array(event_id, expected_pubkey, draft_json,
                      signed_event_json, raw_event_json, state, attempt_count,
                      claim_token, claim_owner, claim_expires_at_ms,
                      active_delivery_plan_id, next_attempt_after_ms, last_error,
                      event_store_ingested, event_store_inserted,
                      event_store_ingested_at_ms, created_at_ms, updated_at_ms)
                      AS BLOB) AS record_json
             FROM outbox_event WHERE outbox_event_id > ?
             ORDER BY outbox_event_id LIMIT ?"
        }
        LegacyOutboxTable::DeliveryPlans => {
            "SELECT delivery_plan_id AS legacy_id, outbox_event_id AS parent_legacy_id,
                    NULL AS related_legacy_id,
                    CAST(json_array(transport_profile_id, target_policy_fingerprint,
                      target_policy_version, satisfaction_policy, required_success_count,
                      delivery_plan_idempotency_digest, status, satisfied_at_ms,
                      created_at_ms, updated_at_ms) AS BLOB) AS record_json
             FROM outbox_delivery_plan WHERE delivery_plan_id > ?
             ORDER BY delivery_plan_id LIMIT ?"
        }
        LegacyOutboxTable::DeliveryTargets => {
            "SELECT delivery_target_id AS legacy_id, delivery_plan_id AS parent_legacy_id,
                    NULL AS related_legacy_id,
                    CAST(json_array(transport_kind, endpoint_uri, target_scope,
                      target_label, endpoint_fingerprint, status, last_outcome_kind,
                      attempt_count, last_attempt_at_ms, completed_at_ms, last_error)
                      AS BLOB) AS record_json
             FROM outbox_delivery_target WHERE delivery_target_id > ?
             ORDER BY delivery_target_id LIMIT ?"
        }
        LegacyOutboxTable::DeliveryAttempts => {
            "SELECT delivery_attempt_id AS legacy_id,
                    delivery_target_id AS parent_legacy_id,
                    delivery_plan_id AS related_legacy_id,
                    CAST(json_array(status, outcome_kind, attempted_at_ms, message)
                      AS BLOB) AS record_json
             FROM outbox_delivery_attempt WHERE delivery_attempt_id > ?
             ORDER BY delivery_attempt_id LIMIT ?"
        }
    }
}

fn encode_outbox_stage_cursor(table: LegacyOutboxTable, legacy_id: i64) -> [u8; 9] {
    let mut cursor = [0_u8; 9];
    cursor[0] = table.code();
    cursor[1..].copy_from_slice(&legacy_id.to_be_bytes());
    cursor
}

fn decode_outbox_stage_cursor(cursor: Option<&[u8]>) -> Result<(LegacyOutboxTable, i64), Error> {
    let Some(cursor) = cursor else {
        return Ok((LegacyOutboxTable::Operations, 0));
    };
    let exact = decode_exact_outbox_stage_cursor(cursor)?;
    let table = match exact[0] {
        1 => LegacyOutboxTable::Operations,
        2 => LegacyOutboxTable::Events,
        3 => LegacyOutboxTable::DeliveryPlans,
        4 => LegacyOutboxTable::DeliveryTargets,
        5 => LegacyOutboxTable::DeliveryAttempts,
        _ => return Err(Error::InvalidLegacyImportJournal),
    };
    let legacy_id = i64::from_be_bytes(
        exact[1..]
            .try_into()
            .map_err(|_| Error::InvalidLegacyImportJournal)?,
    );
    if legacy_id < 0 {
        return Err(Error::InvalidLegacyImportJournal);
    }
    Ok((table, legacy_id))
}

fn decode_exact_outbox_stage_cursor(cursor: &[u8]) -> Result<[u8; 9], Error> {
    <[u8; 9]>::try_from(cursor).map_err(|_| Error::InvalidLegacyImportJournal)
}

fn private_stage_query(table: LegacyPrivateTable) -> &'static str {
    match table {
        LegacyPrivateTable::Metadata => {
            "SELECT printf('%020d', singleton) AS key_cursor, NULL AS parent_key_version, CAST(json_array(singleton, schema_version, hex(profile_id), hex(runtime_contract_hash), key_version, sqlite_source_id, created_at_ms, updated_at_ms) AS BLOB) AS record_json FROM private_metadata WHERE printf('%020d', singleton) > ? ORDER BY key_cursor LIMIT ?"
        }
        LegacyPrivateTable::WrappedProfileKeys => {
            "SELECT printf('%020d', key_version) AS key_cursor, NULL AS parent_key_version, CAST(json_array(key_version, credential_backend, hex(wrapped_key), hex(nonce), created_at_ms, retired_at_ms) AS BLOB) AS record_json FROM wrapped_profile_key WHERE printf('%020d', key_version) > ? ORDER BY key_cursor LIMIT ?"
        }
        LegacyPrivateTable::SigningSecrets => {
            "SELECT hex(account_id) AS key_cursor, key_version AS parent_key_version, CAST(json_array(hex(account_id), hex(public_key), key_version, hex(ciphertext), hex(nonce), created_at_ms, updated_at_ms) AS BLOB) AS record_json FROM wrapped_signing_secret WHERE hex(account_id) > ? ORDER BY key_cursor LIMIT ?"
        }
        LegacyPrivateTable::FarmLocations => {
            "SELECT printf('%010d|%s|%s', farm_kind, hex(owner_pubkey), farm_d_tag) AS key_cursor, key_version AS parent_key_version, CAST(json_array(farm_kind, hex(owner_pubkey), farm_d_tag, key_version, hex(ciphertext), hex(nonce), created_at_ms, updated_at_ms) AS BLOB) AS record_json FROM private_farm_location WHERE printf('%010d|%s|%s', farm_kind, hex(owner_pubkey), farm_d_tag) > ? ORDER BY key_cursor LIMIT ?"
        }
        LegacyPrivateTable::TradeArtifacts => {
            "SELECT artifact_id AS key_cursor, key_version AS parent_key_version, CAST(json_array(artifact_id, trade_id, candidate_id, artifact_kind, schema_id, ciphertext_commitment, key_version, hex(ciphertext), hex(encryption_metadata), retention_class, created_at_ms, expires_at_ms, deleted_at_ms) AS BLOB) AS record_json FROM private_trade_artifacts WHERE artifact_id > ? ORDER BY key_cursor LIMIT ?"
        }
        LegacyPrivateTable::CursorKeys => {
            "SELECT hex(key_id) AS key_cursor, key_version AS parent_key_version, CAST(json_array(hex(key_id), key_version, hex(ciphertext), hex(nonce), created_at_ms, retired_at_ms) AS BLOB) AS record_json FROM cursor_hmac_key WHERE hex(key_id) > ? ORDER BY key_cursor LIMIT ?"
        }
        LegacyPrivateTable::Nip46Sessions => {
            "SELECT hex(session_id) AS key_cursor, key_version AS parent_key_version, CAST(json_array(hex(session_id), hex(user_pubkey), hex(remote_signer_pubkey), hex(client_pubkey), key_version, hex(ciphertext), hex(nonce), expires_at_ms, status, created_at_ms, updated_at_ms) AS BLOB) AS record_json FROM nip46_session_private WHERE hex(session_id) > ? ORDER BY key_cursor LIMIT ?"
        }
        LegacyPrivateTable::RotationProgress => {
            "SELECT printf('%020d', singleton) AS key_cursor, NULL AS parent_key_version, CAST(json_array(singleton, from_key_version, to_key_version, table_name, hex(last_primary_key), state, started_at_ms, updated_at_ms, error_code) AS BLOB) AS record_json FROM key_rotation_progress WHERE printf('%020d', singleton) > ? ORDER BY key_cursor LIMIT ?"
        }
    }
}

fn encode_private_stage_cursor(table: LegacyPrivateTable, key: &str) -> Vec<u8> {
    let mut cursor = Vec::with_capacity(key.len() + 1);
    cursor.push(table.code());
    cursor.extend_from_slice(key.as_bytes());
    cursor
}

fn decode_private_stage_cursor(
    cursor: Option<&[u8]>,
) -> Result<(LegacyPrivateTable, String), Error> {
    let Some(cursor) = cursor else {
        return Ok((LegacyPrivateTable::Metadata, String::new()));
    };
    if cursor.is_empty() || cursor.len() > 1025 {
        return Err(Error::InvalidLegacyImportJournal);
    }
    let table = match cursor[0] {
        1 => LegacyPrivateTable::Metadata,
        2 => LegacyPrivateTable::WrappedProfileKeys,
        3 => LegacyPrivateTable::SigningSecrets,
        4 => LegacyPrivateTable::FarmLocations,
        5 => LegacyPrivateTable::TradeArtifacts,
        6 => LegacyPrivateTable::CursorKeys,
        7 => LegacyPrivateTable::Nip46Sessions,
        8 => LegacyPrivateTable::RotationProgress,
        _ => return Err(Error::InvalidLegacyImportJournal),
    };
    let key = std::str::from_utf8(&cursor[1..]).map_err(|_| Error::InvalidLegacyImportJournal)?;
    Ok((table, key.to_owned()))
}

struct ConvertedLegacyEvent {
    sequence: i64,
    event_id: [u8; 32],
    signed_event: Vec<u8>,
    verification_status: String,
    contract_status: String,
    projection_eligible: i64,
    inserted_at_ms: i64,
    updated_at_ms: i64,
}

fn convert_legacy_event_row(row: &sqlx::sqlite::SqliteRow) -> Result<ConvertedLegacyEvent, Error> {
    let sequence = row
        .try_get::<i64, _>("seq")
        .map_err(|_| Error::LegacyImportStagingFailed)?;
    let invalid = || Error::LegacyImportRowInvalid {
        source_kind: LegacySourceKind::EventStore.as_str(),
        legacy_sequence: sequence,
    };
    if sequence <= 0 {
        return Err(invalid());
    }
    let event_id = row
        .try_get::<String, _>("event_id")
        .map_err(|_| invalid())?;
    let raw_json = row
        .try_get::<String, _>("raw_json")
        .map_err(|_| invalid())?;
    let signed = Codec::decode_signed_event(raw_json.as_str()).map_err(|_| invalid())?;
    if signed.id().to_hex() != event_id {
        return Err(invalid());
    }
    let verification_status = row
        .try_get::<String, _>("verification_status")
        .map_err(|_| invalid())?;
    let contract_status = row
        .try_get::<String, _>("contract_status")
        .map_err(|_| invalid())?;
    let projection_eligible = row
        .try_get::<i64, _>("projection_eligible")
        .map_err(|_| invalid())?;
    let inserted_at_ms = row
        .try_get::<i64, _>("inserted_at_ms")
        .map_err(|_| invalid())?;
    let updated_at_ms = row
        .try_get::<i64, _>("updated_at_ms")
        .map_err(|_| invalid())?;
    if verification_status.is_empty()
        || verification_status.len() > 64
        || contract_status.is_empty()
        || contract_status.len() > 64
        || !matches!(projection_eligible, 0 | 1)
        || inserted_at_ms <= 0
        || updated_at_ms < inserted_at_ms
    {
        return Err(invalid());
    }
    Ok(ConvertedLegacyEvent {
        sequence,
        event_id: *signed.id().as_bytes(),
        signed_event: raw_json.into_bytes(),
        verification_status,
        contract_status,
        projection_eligible,
        inserted_at_ms,
        updated_at_ms,
    })
}

fn encode_event_stage_cursor(sequence: i64) -> [u8; 8] {
    sequence.to_be_bytes()
}

fn decode_event_stage_cursor(cursor: Option<&[u8]>) -> Result<i64, Error> {
    cursor.map_or(Ok(0), |bytes| {
        decode_exact_event_stage_cursor(bytes).map(i64::from_be_bytes)
    })
}

fn decode_exact_event_stage_cursor(cursor: &[u8]) -> Result<[u8; 8], Error> {
    let exact = <[u8; 8]>::try_from(cursor).map_err(|_| Error::InvalidLegacyImportJournal)?;
    if i64::from_be_bytes(exact) <= 0 {
        return Err(Error::InvalidLegacyImportJournal);
    }
    Ok(exact)
}

fn journal_matches_classified(
    journal: &LegacyImportJournal,
    classified: &ClassifiedLegacyImport,
    classification_sha256: MemberDigest,
) -> bool {
    let fixed_fields_match = ![
        journal.import_id() == classified.import_id(),
        journal.target_generation() == classified.target_generation(),
        journal.manifest_sha256() == classified.prepared.manifest_sha256(),
        journal.classification_sha256() == classification_sha256,
        journal.members().len() == classified.sources().len(),
    ]
    .contains(&false);
    fixed_fields_match
        & journal
            .members()
            .iter()
            .zip(classified.sources())
            .all(|(durable, expected)| durable.classification() == expected)
}

fn decode_import_id(bytes: Vec<u8>) -> Result<LegacyImportId, Error> {
    LegacyImportId::new(decode_array(bytes)?)
}

fn decode_generation(bytes: Vec<u8>) -> Result<SourceGeneration, Error> {
    SourceGeneration::new(decode_array(bytes)?).map_err(|_| Error::InvalidLegacyImportJournal)
}

fn decode_digest(bytes: Vec<u8>) -> Result<MemberDigest, Error> {
    Ok(MemberDigest::new(decode_array(bytes)?))
}

fn decode_array<const N: usize>(bytes: Vec<u8>) -> Result<[u8; N], Error> {
    bytes
        .try_into()
        .map_err(|_| Error::InvalidLegacyImportJournal)
}

fn decode_positive_time(value: i64) -> Result<u64, Error> {
    let value = u64::try_from(value).map_err(|_| Error::InvalidLegacyImportJournal)?;
    if value == 0 {
        Err(Error::InvalidLegacyImportJournal)
    } else {
        Ok(value)
    }
}

fn parse_source_kind(value: &str) -> Result<LegacySourceKind, Error> {
    match value {
        "event_store" => Ok(LegacySourceKind::EventStore),
        "outbox" => Ok(LegacySourceKind::Outbox),
        "private" => Ok(LegacySourceKind::Private),
        "studio" => Ok(LegacySourceKind::Studio),
        _ => Err(Error::InvalidLegacyImportJournal),
    }
}

fn parse_legacy_schema(value: &str) -> Result<LegacySchema, Error> {
    match value {
        "event_store_v1" => Ok(LegacySchema::EventStoreV1),
        "event_store_v2" => Ok(LegacySchema::EventStoreV2),
        "event_store_v3" => Ok(LegacySchema::EventStoreV3),
        "event_store_v4" => Ok(LegacySchema::EventStoreV4),
        "outbox_v1" => Ok(LegacySchema::OutboxV1),
        "private_v1" => Ok(LegacySchema::PrivateV1),
        "studio_v1_host_handoff" => Ok(LegacySchema::StudioV1HostHandoff),
        _ => Err(Error::InvalidLegacyImportJournal),
    }
}

const fn expected_user_version(schema: LegacySchema) -> u32 {
    match schema {
        LegacySchema::PrivateV1 => 1,
        LegacySchema::EventStoreV1
        | LegacySchema::EventStoreV2
        | LegacySchema::EventStoreV3
        | LegacySchema::EventStoreV4
        | LegacySchema::OutboxV1
        | LegacySchema::StudioV1HostHandoff => 0,
    }
}

const fn schema_source_kind(schema: LegacySchema) -> LegacySourceKind {
    match schema {
        LegacySchema::EventStoreV1
        | LegacySchema::EventStoreV2
        | LegacySchema::EventStoreV3
        | LegacySchema::EventStoreV4 => LegacySourceKind::EventStore,
        LegacySchema::OutboxV1 => LegacySourceKind::Outbox,
        LegacySchema::PrivateV1 => LegacySourceKind::Private,
        LegacySchema::StudioV1HostHandoff => LegacySourceKind::Studio,
    }
}

fn journal_member_states_are_consistent(
    state: LegacyImportState,
    members: &[LegacyImportMemberJournal],
) -> bool {
    members.iter().all(|member| match state {
        LegacyImportState::Classified => member.state() == LegacyImportMemberState::Pending,
        LegacyImportState::Staging => true,
        LegacyImportState::Ready => matches!(
            member.state(),
            LegacyImportMemberState::Ready | LegacyImportMemberState::Complete
        ),
        LegacyImportState::Committing => matches!(
            member.state(),
            LegacyImportMemberState::Ready | LegacyImportMemberState::Complete
        ),
        LegacyImportState::Complete => member.state() == LegacyImportMemberState::Complete,
    })
}

fn parse_import_state(value: &str) -> Result<LegacyImportState, Error> {
    match value {
        "classified" => Ok(LegacyImportState::Classified),
        "staging" => Ok(LegacyImportState::Staging),
        "ready" => Ok(LegacyImportState::Ready),
        "committing" => Ok(LegacyImportState::Committing),
        "complete" => Ok(LegacyImportState::Complete),
        _ => Err(Error::InvalidLegacyImportJournal),
    }
}

fn parse_member_state(value: &str) -> Result<LegacyImportMemberState, Error> {
    match value {
        "pending" => Ok(LegacyImportMemberState::Pending),
        "staging" => Ok(LegacyImportMemberState::Staging),
        "ready" => Ok(LegacyImportMemberState::Ready),
        "complete" => Ok(LegacyImportMemberState::Complete),
        _ => Err(Error::InvalidLegacyImportJournal),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn write_manifest(
    plan: &LegacyImportPlan,
    target_generation: SourceGeneration,
    snapshots: &[LegacySourceSnapshot],
    path: &Path,
) -> Result<(), Error> {
    let mut body = format!(
        "schema_version=1\nimport_id={}\ntarget_generation={}\nrequested_at_unix_ms={}\n",
        encode_id(plan.import_id().as_bytes()),
        encode_digest(target_generation.as_bytes()),
        plan.requested_at_unix_ms()
    );
    for (source, evidence) in plan.sources().iter().zip(snapshots) {
        body.push_str("member=");
        body.push_str(evidence.kind().as_str());
        body.push('|');
        body.push_str(&encode_hex(source.path().as_os_str().as_encoded_bytes()));
        body.push('|');
        body.push_str(evidence.relative_path());
        body.push('|');
        body.push_str(&evidence.byte_length().to_string());
        body.push('|');
        body.push_str(&encode_digest(evidence.sha256().as_bytes()));
        body.push('\n');
    }
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|source| Error::LegacyImportFilesystem {
            operation: "create legacy import manifest",
            source,
        })?;
    use std::io::Write;
    file.write_all(body.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|source| Error::LegacyImportFilesystem {
            operation: "persist legacy import manifest",
            source,
        })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn validate_source_path(path: &Path) -> Result<(), Error> {
    if !path.is_absolute()
        || path.to_str().is_none()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(Error::InvalidLegacySource(path.to_path_buf()));
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(Error::InvalidLegacySource(path.to_path_buf())),
        Err(_) => Err(Error::InvalidLegacySource(path.to_path_buf())),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn paths_refer_to_same_file(left: &Path, right: &Path) -> Result<bool, Error> {
    let left_canonical =
        fs::canonicalize(left).map_err(|source| Error::LegacyImportFilesystem {
            operation: "resolve legacy import source identity",
            source,
        })?;
    let right_canonical =
        fs::canonicalize(right).map_err(|source| Error::LegacyImportFilesystem {
            operation: "resolve owned storage identity",
            source,
        })?;
    if left_canonical == right_canonical {
        return Ok(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let left_metadata = fs::metadata(left).map_err(|source| Error::LegacyImportFilesystem {
            operation: "inspect legacy import source identity",
            source,
        })?;
        let right_metadata =
            fs::metadata(right).map_err(|source| Error::LegacyImportFilesystem {
                operation: "inspect owned storage identity",
                source,
            })?;
        Ok(left_metadata.dev() == right_metadata.dev()
            && left_metadata.ino() == right_metadata.ino())
    }
    #[cfg(not(unix))]
    Ok(false)
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn sync_directory(path: &Path, operation: &'static str) -> Result<(), Error> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| Error::LegacyImportFilesystem { operation, source })
}

fn encode_id(bytes: &[u8; 16]) -> String {
    encode_hex(bytes)
}

fn encode_digest(bytes: &[u8; 32]) -> String {
    encode_hex(bytes)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

const fn bytes_are_zero<const N: usize>(bytes: &[u8; N]) -> bool {
    let mut index = 0;
    while index < N {
        if bytes[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use radroots_event::{SignedEvent, wire::Nip01EventWire};
    use radroots_storage::event::SourceGeneration;
    use serde::Deserialize;

    use crate::{OpenMode, OpenOptions, Paths};

    use super::*;

    const POLICY: &str =
        include_str!("../../../contracts/storage/legacy_import_backup_policy_v1.toml");
    const CLASSIFICATION_POLICY: &str =
        include_str!("../../../contracts/storage/legacy_schema_classification_v1.toml");
    const JOURNAL_POLICY: &str =
        include_str!("../../../contracts/storage/legacy_import_journal_policy_v1.toml");
    const EVENT_STAGING_POLICY: &str =
        include_str!("../../../contracts/storage/legacy_event_staging_policy_v1.toml");
    const OUTBOX_STAGING_POLICY: &str =
        include_str!("../../../contracts/storage/legacy_outbox_staging_policy_v1.toml");
    const EVENT_STORE_V1_SQL: &str =
        include_str!("../../event_store/migrations/0001_event_store.up.sql");
    const OUTBOX_V1_SQL: &str = include_str!("../../outbox/migrations/0001_outbox.up.sql");
    const PRIVATE_STAGING_POLICY: &str =
        include_str!("../../../contracts/storage/legacy_private_staging_policy_v1.toml");
    const STUDIO_HANDOFF_POLICY: &str =
        include_str!("../../../contracts/storage/legacy_studio_handoff_policy_v1.toml");
    const IMPORT_VALIDATION_POLICY: &str =
        include_str!("../../../contracts/storage/legacy_import_validation_policy_v1.toml");
    const IMPORT_FINALIZE_POLICY: &str =
        include_str!("../../../contracts/storage/legacy_import_finalize_policy_v1.toml");
    const IMPORT_QUALIFICATION_POLICY: &str =
        include_str!("../../../contracts/storage/legacy_import_qualification_v1.toml");
    const PRIVATE_STORE_V1_SQL: &str = include_str!("fixtures/private_store_v1.sql");

    #[derive(Deserialize)]
    struct Policy {
        schema_version: u32,
        mode: String,
        source_kinds: Vec<String>,
        source_path: String,
        source_cardinality: String,
        owned_file_alias: String,
        authority: String,
        capture: String,
        staging: String,
        finalized: String,
        member_mode: String,
        manifest: String,
        verification: Vec<String>,
        finalization: String,
        mutation_before_finalized_backup: bool,
        collision: String,
        hidden_entropy_or_clock: bool,
    }

    #[derive(Deserialize)]
    struct ClassificationPolicy {
        schema_version: u32,
        catalog_algorithm: String,
        unknown_objects: String,
        mixed_source_families: String,
        newer_versions: String,
        target_mutation: bool,
        event_store: EventStorePolicy,
        outbox: FixedSchemaPolicy,
        private: FixedSchemaPolicy,
        studio: StudioSchemaPolicy,
    }

    #[derive(Deserialize)]
    struct EventStorePolicy {
        versions: Vec<u32>,
        unledgered_version: u32,
        ledger: String,
        schema_sha256: Vec<String>,
        names: Vec<String>,
        up_sha256: Vec<String>,
        down_sha256: Vec<String>,
    }

    #[derive(Deserialize)]
    struct FixedSchemaPolicy {
        version: u32,
        user_version: i64,
        catalog_sha256: String,
        source: String,
        schema_sql_sha256: String,
    }

    #[derive(Deserialize)]
    struct StudioSchemaPolicy {
        version: u32,
        user_version: i64,
        catalog_sha256: String,
        source: String,
        schema_sql_sha256: String,
        disposition: String,
    }

    #[derive(Deserialize)]
    struct JournalPolicy {
        schema_version: u32,
        authority: String,
        identity: Vec<String>,
        import_states: Vec<String>,
        member_states: Vec<String>,
        imports_per_target_generation: u32,
        begin: String,
        resume: String,
        source_members: String,
        resume_cursor: String,
        staged_row_count: String,
        host_timestamp: String,
        hidden_clock_or_entropy: bool,
        legacy_row_conversion: bool,
        live_product_row_mutation: bool,
    }

    #[derive(Deserialize)]
    struct EventStagingPolicy {
        schema_version: u32,
        authority: String,
        source_kind: String,
        supported_schemas: Vec<String>,
        source_table: String,
        ordering: String,
        cursor: String,
        page_limit_max: u16,
        conversion: Vec<String>,
        transaction: String,
        idempotency: String,
        staging_rows: String,
        evidence_revalidation: String,
        live_product_row_mutation: bool,
        hidden_clock_or_entropy: bool,
    }

    #[derive(Deserialize)]
    struct OutboxStagingPolicy {
        schema_version: u32,
        authority: String,
        source_kind: String,
        supported_schema: String,
        table_order: Vec<String>,
        cursor: String,
        page_limit_max: u16,
        record: String,
        references: Vec<String>,
        transaction: String,
        idempotency: String,
        staging_rows: String,
        evidence_revalidation: String,
        live_product_row_mutation: bool,
        hidden_clock_or_entropy: bool,
    }

    #[derive(Deserialize)]
    struct PrivateStagingPolicy {
        schema_version: u32,
        runtime_authority: String,
        private_authority: String,
        source_kind: String,
        supported_schema: String,
        table_order: Vec<String>,
        cursor: String,
        page_limit_max: u16,
        record: String,
        secret_bearing_staging_database: String,
        wrapping_key_reference: String,
        recovery: Vec<String>,
        crash_before_private_commit: String,
        crash_after_private_commit: String,
        conflicting_replay: String,
        live_private_artifact_mutation: bool,
        hidden_clock_or_entropy: bool,
    }

    #[derive(Deserialize)]
    struct StudioHandoffPolicy {
        schema_version: u32,
        source_kind: String,
        supported_schema: String,
        disposition: String,
        evidence: String,
        handoff_identity: String,
        receipt: String,
        receipt_cursor_bytes: usize,
        staged_row_count: u64,
        exact_retry: String,
        conflicting_retry: String,
        sdk_runtime_row_import: bool,
        sdk_private_row_import: bool,
        sdk_owned_studio_database: bool,
        source_deletion: bool,
        hidden_clock_or_entropy: bool,
    }

    #[derive(Deserialize)]
    struct ImportValidationPolicy {
        schema_version: u32,
        required_import_state: String,
        required_member_state: String,
        source_evidence: String,
        source_count_match: String,
        studio_source_count: u64,
        snapshot: String,
        validation_identity: String,
        runtime_staging_rows: Vec<String>,
        private_staging_rows: Vec<String>,
        studio_receipt: String,
        validation_mutation: bool,
        source_deletion: bool,
        dual_write: bool,
        hidden_clock_or_entropy: bool,
    }

    #[derive(Deserialize)]
    struct ImportFinalizePolicy {
        schema_version: u32,
        input: String,
        commit_order: Vec<String>,
        private_replay: String,
        runtime_replay: String,
        crash_before_private_commit: String,
        crash_after_private_commit: String,
        crash_during_runtime_completion: String,
        lost_success_response: String,
        retained_representation: String,
        live_product_dual_write: bool,
        source_deletion: bool,
        studio_row_import: bool,
        host_timestamp: String,
        hidden_clock_or_entropy: bool,
    }

    #[derive(Deserialize)]
    struct ImportQualificationPolicy {
        schema_version: u32,
        source_matrix: Vec<String>,
        required_cases: Vec<String>,
        mixed_imported_row_count: u64,
        mixed_host_handoff_row_count: u64,
        exact_retry: bool,
        hidden_clock_or_entropy: bool,
    }

    fn generation(byte: u8) -> SourceGeneration {
        SourceGeneration::new([byte; 32]).expect("source generation")
    }

    async fn target(directory: &Path) -> (Paths, SqliteStorage) {
        let paths = Paths::from_directory(directory).expect("target paths");
        let store = SqliteStorage::open(
            OpenOptions::new(paths.clone(), OpenMode::Create)
                .with_source_generation(generation(121), 12_100)
                .expect("source generation"),
        )
        .await
        .expect("target storage");
        (paths, store)
    }

    async fn legacy_database(path: &Path, table: &'static str) -> SqliteConnection {
        let mut connection = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
        .expect("legacy database");
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(&mut connection)
            .await
            .expect("legacy WAL");
        let (create, insert) = match table {
            "event_envelopes" => (
                "CREATE TABLE event_envelopes(value INTEGER NOT NULL)",
                "INSERT INTO event_envelopes(value) VALUES (41)",
            ),
            "sdk_studio_state" => (
                "CREATE TABLE sdk_studio_state(value INTEGER NOT NULL)",
                "INSERT INTO sdk_studio_state(value) VALUES (41)",
            ),
            _ => panic!("unsupported legacy test table"),
        };
        sqlx::query(create)
            .execute(&mut connection)
            .await
            .expect("legacy schema");
        sqlx::query(insert)
            .execute(&mut connection)
            .await
            .expect("legacy row");
        connection
    }

    async fn supported_studio_database(path: &Path) -> SqliteConnection {
        let mut connection = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
        .expect("supported Studio database");
        sqlx::query(
            "CREATE TABLE sdk_studio_state (
  key TEXT PRIMARY KEY NOT NULL,
  value_json TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL
)",
        )
        .execute(&mut connection)
        .await
        .expect("supported Studio schema");
        sqlx::query(
            "INSERT INTO sdk_studio_state(key, value_json, updated_at_ms) VALUES ('theme', '{}', 1)",
        )
        .execute(&mut connection)
        .await
        .expect("supported Studio row");
        connection
    }

    fn signed_event(content: &str) -> SignedEvent {
        let mut wire = Nip01EventWire {
            id: "0".repeat(64),
            pubkey: "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df".to_owned(),
            created_at: 1_800_000_100,
            kind: 1,
            tags: vec![],
            content: content.to_owned(),
            sig: "42".repeat(64),
            extra: Default::default(),
        };
        wire.id = wire
            .computed_event_id()
            .expect("canonical event id")
            .to_hex();
        let raw_json = serde_json::json!({
            "id": &wire.id,
            "pubkey": &wire.pubkey,
            "created_at": wire.created_at,
            "kind": wire.kind,
            "tags": &wire.tags,
            "content": &wire.content,
            "sig": &wire.sig,
        })
        .to_string();
        SignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
    }

    async fn supported_event_database(path: &Path, events: &[SignedEvent]) -> SqliteConnection {
        let mut connection = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
        .expect("supported event database");
        sqlx::raw_sql(EVENT_STORE_V1_SQL)
            .execute(&mut connection)
            .await
            .expect("event-store v1 schema");
        for (index, event) in events.iter().enumerate() {
            let timestamp = 13_000_i64 + i64::try_from(index).expect("event index");
            sqlx::query(
                "INSERT INTO event_envelopes(
                    event_id, pubkey, created_at, kind, tags_json, content, sig,
                    raw_json, verification_status, contract_status, contract_id,
                    event_class, projection_eligible, inserted_at_ms, updated_at_ms
                 ) VALUES (?, ?, ?, ?, '[]', ?, ?, ?, 'verified', 'admitted',
                           NULL, NULL, 1, ?, ?)",
            )
            .bind(event.id().to_hex())
            .bind(event.envelope().author().to_hex())
            .bind(i64::try_from(event.created_at()).expect("created at"))
            .bind(i64::from(event.kind()))
            .bind(event.content())
            .bind(event.signature_hex())
            .bind(event.raw_json())
            .bind(timestamp)
            .bind(timestamp)
            .execute(&mut connection)
            .await
            .expect("legacy event");
        }
        connection
    }

    async fn supported_outbox_database(path: &Path) -> SqliteConnection {
        let mut connection = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
        .expect("supported outbox database");
        sqlx::raw_sql(OUTBOX_V1_SQL)
            .execute(&mut connection)
            .await
            .expect("outbox v1 schema");
        sqlx::query(
            "INSERT INTO outbox_operations(
              operation_kind, expected_pubkey, semantic_scope, trade_id, mutation_id,
              canonical_payload_sha256, idempotency_key, operation_idempotency_digest,
              status, created_at_ms, updated_at_ms
             ) VALUES ('publish', 'author', 'generic_event', NULL, NULL, NULL,
                       'key', 'operation-digest', 'queued', 1, 1)",
        )
        .execute(&mut connection)
        .await
        .expect("outbox operation");
        sqlx::query(
            "INSERT INTO outbox_event(
              operation_id, event_id, expected_pubkey, draft_json, signed_event_json,
              raw_event_json, state, attempt_count, claim_token, claim_owner,
              claim_expires_at_ms, active_delivery_plan_id, next_attempt_after_ms,
              last_error, event_store_ingested, event_store_inserted,
              event_store_ingested_at_ms, created_at_ms, updated_at_ms
             ) VALUES (1, 'event', 'author', '{}', NULL, NULL, 'draft_queued', 0,
                       NULL, NULL, NULL, NULL, 1, NULL, 0, 0, NULL, 1, 1)",
        )
        .execute(&mut connection)
        .await
        .expect("outbox event");
        sqlx::query(
            "INSERT INTO outbox_delivery_plan(
              outbox_event_id, transport_profile_id, target_policy_fingerprint,
              target_policy_version, satisfaction_policy, required_success_count,
              delivery_plan_idempotency_digest, status, satisfied_at_ms,
              created_at_ms, updated_at_ms
             ) VALUES (1, 'nostr', 'policy', 1, 'all', 1, 'plan-digest',
                       'queued', NULL, 1, 1)",
        )
        .execute(&mut connection)
        .await
        .expect("outbox plan");
        sqlx::query(
            "INSERT INTO outbox_delivery_target(
              delivery_plan_id, transport_kind, endpoint_uri, target_scope,
              target_label, endpoint_fingerprint, status, last_outcome_kind,
              attempt_count, last_attempt_at_ms, completed_at_ms, last_error
             ) VALUES (1, 'nostr', 'wss://relay.example', NULL, NULL, 'endpoint',
                       'pending', NULL, 0, NULL, NULL, NULL)",
        )
        .execute(&mut connection)
        .await
        .expect("outbox target");
        sqlx::query(
            "INSERT INTO outbox_delivery_attempt(
              delivery_plan_id, delivery_target_id, status, outcome_kind,
              attempted_at_ms, message
             ) VALUES (1, 1, 'complete', 'accepted', 2, 'accepted')",
        )
        .execute(&mut connection)
        .await
        .expect("outbox attempt");
        connection
    }

    async fn supported_private_database(path: &Path) -> SqliteConnection {
        let mut connection = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
        .expect("supported private database");
        sqlx::raw_sql(PRIVATE_STORE_V1_SQL)
            .execute(&mut connection)
            .await
            .expect("private v1 schema");
        sqlx::query("PRAGMA user_version = 1")
            .execute(&mut connection)
            .await
            .expect("private version");
        sqlx::query("INSERT INTO private_metadata VALUES (1,1,?,?,1,'source',1,1)")
            .bind([1_u8; 16].as_slice())
            .bind([2_u8; 32].as_slice())
            .execute(&mut connection)
            .await
            .expect("metadata");
        sqlx::query(
            "INSERT INTO wrapped_profile_key VALUES (1,'memory_test_wrapped_v1',?,?,1,NULL)",
        )
        .bind([3_u8; 32].as_slice())
        .bind([4_u8; 24].as_slice())
        .execute(&mut connection)
        .await
        .expect("wrapped key");
        sqlx::query("INSERT INTO wrapped_signing_secret VALUES (?,?,?,?,?,1,1)")
            .bind([5_u8; 16].as_slice())
            .bind([6_u8; 32].as_slice())
            .bind(1_i64)
            .bind([7_u8; 8].as_slice())
            .bind([8_u8; 24].as_slice())
            .execute(&mut connection)
            .await
            .expect("signing secret");
        sqlx::query("INSERT INTO private_farm_location VALUES (30340,?,?,1,?,?,1,1)")
            .bind([9_u8; 32].as_slice())
            .bind("farm")
            .bind([10_u8; 8].as_slice())
            .bind([11_u8; 24].as_slice())
            .execute(&mut connection)
            .await
            .expect("farm location");
        sqlx::query("INSERT INTO private_trade_artifacts VALUES ('artifact','01234567890123456789012345678901',NULL,'message','schema',?,1,?,?,'retain',1,NULL,NULL)")
            .bind("a".repeat(64)).bind([12_u8;8].as_slice()).bind([13_u8;4].as_slice()).execute(&mut connection).await.expect("trade artifact");
        sqlx::query("INSERT INTO cursor_hmac_key VALUES (?,1,?,?,1,NULL)")
            .bind([14_u8; 16].as_slice())
            .bind([15_u8; 8].as_slice())
            .bind([16_u8; 24].as_slice())
            .execute(&mut connection)
            .await
            .expect("cursor key");
        sqlx::query("INSERT INTO nip46_session_private VALUES (?,?,?,?,1,?,?,2,'active',1,1)")
            .bind([17_u8; 16].as_slice())
            .bind([18_u8; 32].as_slice())
            .bind([19_u8; 32].as_slice())
            .bind([20_u8; 32].as_slice())
            .bind([21_u8; 8].as_slice())
            .bind([22_u8; 24].as_slice())
            .execute(&mut connection)
            .await
            .expect("nip46 session");
        sqlx::query(
            "INSERT INTO key_rotation_progress VALUES (1,1,2,'done',NULL,'complete',1,1,NULL)",
        )
        .execute(&mut connection)
        .await
        .expect("rotation progress");
        connection
    }

    #[test]
    fn implementation_matches_the_governed_legacy_backup_policy() {
        let policy = toml::from_str::<Policy>(POLICY).expect("legacy backup policy");
        assert_eq!(policy.schema_version, 1);
        assert_eq!(policy.mode, "explicit_forward_only_one_shot");
        assert_eq!(
            policy.source_kinds,
            ["event_store", "outbox", "private", "studio"]
        );
        assert_eq!(
            policy.source_path,
            "absolute_existing_utf8_regular_file_no_symlink"
        );
        assert_eq!(
            policy.source_cardinality,
            "one_to_four_unique_kinds_and_paths"
        );
        assert_eq!(
            policy.owned_file_alias,
            "reject_path_canonical_or_file_identity"
        );
        assert_eq!(policy.authority, "open_writable_target_backend");
        assert_eq!(policy.capture, "sqlite_vacuum_into");
        assert_eq!(
            policy.staging,
            ".radroots-legacy-import-{import_id_hex}.staging"
        );
        assert_eq!(policy.finalized, "radroots-legacy-import-{import_id_hex}");
        assert_eq!(policy.member_mode, "0600");
        assert_eq!(
            policy.manifest,
            "manifest.v1_exact_identity_target_generation_timestamp_source_provenance_inventory_lengths_sha256"
        );
        assert_eq!(
            policy.verification,
            [
                "sqlite_quick_check",
                "foreign_key_check",
                "exact_length",
                "sha256"
            ]
        );
        assert_eq!(policy.finalization, "same_root_atomic_directory_rename");
        assert!(!policy.mutation_before_finalized_backup);
        assert_eq!(policy.collision, "reject");
        assert!(!policy.hidden_entropy_or_clock);
    }

    #[test]
    fn implementation_matches_the_governed_schema_classification_policy() {
        let policy = toml::from_str::<ClassificationPolicy>(CLASSIFICATION_POLICY)
            .expect("legacy classification policy");
        assert_eq!(policy.schema_version, 1);
        assert_eq!(
            policy.catalog_algorithm,
            "sqlite_schema_non_internal_type_name_table_sql_nul_sha256_v1"
        );
        assert_eq!(policy.unknown_objects, "reject");
        assert_eq!(policy.mixed_source_families, "reject");
        assert_eq!(policy.newer_versions, "reject");
        assert!(!policy.target_mutation);
        assert_eq!(policy.event_store.versions, [1, 2, 3, 4]);
        assert_eq!(policy.event_store.unledgered_version, 1);
        assert_eq!(
            policy.event_store.ledger,
            "radroots_event_store_schema_migrations_exact_catalog_and_contiguous_rows"
        );
        assert_eq!(
            policy.event_store.schema_sha256,
            EVENT_STORE_MIGRATIONS
                .iter()
                .map(|migration| migration.schema_sha256)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            policy.event_store.names,
            EVENT_STORE_MIGRATIONS
                .iter()
                .map(|migration| migration.name)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            policy.event_store.up_sha256,
            EVENT_STORE_MIGRATIONS
                .iter()
                .map(|migration| migration.up_sha256)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            policy.event_store.down_sha256,
            EVENT_STORE_MIGRATIONS
                .iter()
                .map(|migration| migration.down_sha256)
                .collect::<Vec<_>>()
        );
        assert_fixed_schema_policy(
            &policy.outbox,
            0,
            OUTBOX_CATALOG_SHA256,
            "crates/outbox/migrations/0001_outbox.up.sql",
            "a7ee775d32c2b9f845961425362e1b1e558ce0d025f7d22dd58f118ba4dab4fa",
        );
        assert_fixed_schema_policy(
            &policy.private,
            1,
            PRIVATE_CATALOG_SHA256,
            "oss/sdk/crates/sdk/src/private_store.rs::PRIVATE_STORE_MIGRATION_UP",
            "f7e71d2cf4347f9b78bafd37980901441b09d111f15d94d260eb7133b626fbe9",
        );
        assert_eq!(policy.studio.version, 1);
        assert_eq!(policy.studio.user_version, 0);
        assert_eq!(policy.studio.catalog_sha256, STUDIO_CATALOG_SHA256);
        assert_eq!(
            policy.studio.source,
            "oss/sdk/crates/sdk/src/studio_store.rs::STUDIO_STORE_MIGRATION_UP"
        );
        assert_eq!(
            policy.studio.schema_sql_sha256,
            "9c5b33810ad9746421dc843651eb02c77d6fc8f00fd630cb835c15b4e36d0590"
        );
        assert_eq!(policy.studio.disposition, "host_handoff_not_sdk_import");
    }

    #[test]
    fn implementation_matches_the_governed_import_journal_policy() {
        let policy =
            toml::from_str::<JournalPolicy>(JOURNAL_POLICY).expect("legacy journal policy");
        assert_eq!(policy.schema_version, 1);
        assert_eq!(
            policy.authority,
            "runtime_sqlite_owned_forward_migration_v6"
        );
        assert_eq!(
            policy.identity,
            [
                "import_id",
                "target_generation",
                "manifest_sha256",
                "classification_sha256"
            ]
        );
        assert_eq!(
            policy.import_states,
            ["classified", "staging", "ready", "committing", "complete"]
        );
        assert_eq!(
            policy.member_states,
            ["pending", "staging", "ready", "complete"]
        );
        assert_eq!(policy.imports_per_target_generation, 1);
        assert_eq!(policy.begin, "atomic_exact_idempotent_or_conflict");
        assert_eq!(policy.resume, "read_exact_durable_state");
        assert_eq!(policy.source_members, "one_exact_row_per_classified_source");
        assert_eq!(policy.resume_cursor, "opaque_nullable_bytes");
        assert_eq!(policy.staged_row_count, "non_negative");
        assert_eq!(policy.host_timestamp, "positive_monotonic_per_import");
        assert!(!policy.hidden_clock_or_entropy);
        assert!(!policy.legacy_row_conversion);
        assert!(!policy.live_product_row_mutation);
    }

    #[test]
    fn implementation_matches_the_governed_event_staging_policy() {
        let policy = toml::from_str::<EventStagingPolicy>(EVENT_STAGING_POLICY)
            .expect("legacy event staging policy");
        assert_eq!(policy.schema_version, 1);
        assert_eq!(
            policy.authority,
            "runtime_sqlite_owned_forward_migration_v7"
        );
        assert_eq!(policy.source_kind, "event_store");
        assert_eq!(
            policy.supported_schemas,
            [
                "event_store_v1",
                "event_store_v2",
                "event_store_v3",
                "event_store_v4"
            ]
        );
        assert_eq!(policy.source_table, "event_envelopes");
        assert_eq!(policy.ordering, "strict_legacy_sequence_ascending");
        assert_eq!(policy.cursor, "positive_i64_big_endian_8_bytes");
        assert_eq!(policy.page_limit_max, LEGACY_STAGE_PAGE_LIMIT_MAX);
        assert_eq!(
            policy.conversion,
            [
                "decode_id_verified_nip01_signed_event",
                "require_legacy_event_id_match",
                "event_id_hex_to_32_bytes",
                "preserve_exact_signed_event_json_bytes",
                "preserve_legacy_admission_evidence_without_trust_upgrade"
            ]
        );
        assert_eq!(
            policy.transaction,
            "target_begin_immediate_rows_cursor_count_and_state_atomic"
        );
        assert_eq!(
            policy.idempotency,
            "durable_cursor_exact_resume_completed_retry_noop"
        );
        assert_eq!(policy.staging_rows, "append_only_immutable");
        assert_eq!(policy.evidence_revalidation, "before_every_page");
        assert!(!policy.live_product_row_mutation);
        assert!(!policy.hidden_clock_or_entropy);
    }

    #[test]
    fn implementation_matches_the_governed_outbox_staging_policy() {
        let policy = toml::from_str::<OutboxStagingPolicy>(OUTBOX_STAGING_POLICY)
            .expect("legacy outbox staging policy");
        assert_eq!(policy.schema_version, 1);
        assert_eq!(
            policy.authority,
            "runtime_sqlite_owned_forward_migration_v8"
        );
        assert_eq!(policy.source_kind, "outbox");
        assert_eq!(policy.supported_schema, "outbox_v1");
        assert_eq!(
            policy.table_order,
            [
                "operations",
                "events",
                "delivery_plans",
                "delivery_targets",
                "delivery_attempts"
            ]
        );
        assert_eq!(
            policy.cursor,
            "table_discriminator_u8_plus_non_negative_i64_big_endian"
        );
        assert_eq!(policy.page_limit_max, LEGACY_STAGE_PAGE_LIMIT_MAX);
        assert_eq!(
            policy.record,
            "sqlite_json_array_exact_governed_column_order_blob"
        );
        assert_eq!(
            policy.references,
            [
                "event_to_operation",
                "delivery_plan_to_event",
                "delivery_target_to_plan",
                "delivery_attempt_to_target_and_same_plan"
            ]
        );
        assert_eq!(
            policy.transaction,
            "target_begin_immediate_rows_cursor_count_and_state_atomic"
        );
        assert_eq!(
            policy.idempotency,
            "durable_table_cursor_exact_resume_completed_retry_noop"
        );
        assert_eq!(policy.staging_rows, "append_only_immutable");
        assert_eq!(policy.evidence_revalidation, "before_every_page");
        assert!(!policy.live_product_row_mutation);
        assert!(!policy.hidden_clock_or_entropy);
    }

    #[test]
    fn implementation_matches_the_governed_private_staging_policy() {
        let policy = toml::from_str::<PrivateStagingPolicy>(PRIVATE_STAGING_POLICY)
            .expect("private staging policy");
        assert_eq!(policy.schema_version, 1);
        assert_eq!(policy.runtime_authority, "runtime_sqlite_import_journal_v8");
        assert_eq!(
            policy.private_authority,
            "private_sqlite_forward_migration_v2"
        );
        assert_eq!(policy.source_kind, "private");
        assert_eq!(policy.supported_schema, "private_v1");
        assert_eq!(
            policy.table_order,
            [
                "metadata",
                "wrapped_profile_keys",
                "signing_secrets",
                "farm_locations",
                "trade_artifacts",
                "cursor_keys",
                "nip46_sessions",
                "rotation_progress"
            ]
        );
        assert_eq!(
            policy.cursor,
            "table_discriminator_u8_plus_utf8_canonical_key_max_1024"
        );
        assert_eq!(policy.page_limit_max, LEGACY_STAGE_PAGE_LIMIT_MAX);
        assert_eq!(
            policy.record,
            "sqlite_json_array_governed_column_order_with_blob_hex"
        );
        assert_eq!(policy.secret_bearing_staging_database, "private.sqlite");
        assert_eq!(
            policy.wrapping_key_reference,
            "required_before_dependent_record"
        );
        assert_eq!(
            policy.recovery,
            [
                "runtime_enter_staging",
                "private_exact_idempotent_page_commit",
                "runtime_cursor_count_commit"
            ]
        );
        assert_eq!(
            policy.crash_before_private_commit,
            "no_private_rows_and_old_runtime_cursor"
        );
        assert_eq!(
            policy.crash_after_private_commit,
            "exact_replay_verification_from_old_runtime_cursor"
        );
        assert_eq!(policy.conflicting_replay, "reject");
        assert!(!policy.live_private_artifact_mutation);
        assert!(!policy.hidden_clock_or_entropy);
    }

    #[test]
    fn implementation_matches_the_governed_studio_handoff_policy() {
        let policy = toml::from_str::<StudioHandoffPolicy>(STUDIO_HANDOFF_POLICY)
            .expect("Studio handoff policy");
        assert_eq!(policy.schema_version, 1);
        assert_eq!(policy.source_kind, "studio");
        assert_eq!(policy.supported_schema, "studio_v1_host_handoff");
        assert_eq!(policy.disposition, "host_handoff");
        assert_eq!(policy.evidence, "immutable_preimport_backup_member");
        assert_eq!(
            policy.handoff_identity,
            "sha256_domain_import_target_manifest_relative_path_source_catalog_length"
        );
        assert_eq!(
            policy.receipt,
            "handoff_sha256_plus_nonzero_opaque_host_commitment_sha256"
        );
        assert_eq!(policy.receipt_cursor_bytes, 64);
        assert_eq!(policy.staged_row_count, 0);
        assert_eq!(policy.exact_retry, "idempotent");
        assert_eq!(policy.conflicting_retry, "reject");
        assert!(!policy.sdk_runtime_row_import);
        assert!(!policy.sdk_private_row_import);
        assert!(!policy.sdk_owned_studio_database);
        assert!(!policy.source_deletion);
        assert!(!policy.hidden_clock_or_entropy);
    }

    #[test]
    fn implementation_matches_the_governed_import_validation_policy() {
        let policy = toml::from_str::<ImportValidationPolicy>(IMPORT_VALIDATION_POLICY)
            .expect("import validation policy");
        assert_eq!(policy.schema_version, 1);
        assert_eq!(policy.required_import_state, "ready");
        assert_eq!(policy.required_member_state, "ready");
        assert_eq!(policy.source_evidence, "reverified_immutable_backup");
        assert_eq!(policy.source_count_match, "exact_per_member");
        assert_eq!(policy.studio_source_count, 0);
        assert_eq!(
            policy.snapshot,
            "runtime_begin_immediate_then_private_begin_immediate"
        );
        assert_eq!(
            policy.validation_identity,
            "sha256_framed_import_target_manifest_classification_members_staged_rows"
        );
        assert_eq!(policy.runtime_staging_rows, ["events", "outbox_graph"]);
        assert_eq!(policy.private_staging_rows, ["private_records"]);
        assert_eq!(policy.studio_receipt, "member_cursor_only");
        assert!(!policy.validation_mutation);
        assert!(!policy.source_deletion);
        assert!(!policy.dual_write);
        assert!(!policy.hidden_clock_or_entropy);
    }

    #[test]
    fn implementation_matches_the_governed_import_finalize_policy() {
        let policy = toml::from_str::<ImportFinalizePolicy>(IMPORT_FINALIZE_POLICY)
            .expect("import finalize policy");
        assert_eq!(policy.schema_version, 1);
        assert_eq!(policy.input, "exact_legacy_import_validation");
        assert_eq!(
            policy.commit_order,
            ["private_commit_marker", "runtime_atomic_completion"]
        );
        assert_eq!(policy.private_replay, "insert_or_ignore_then_exact_verify");
        assert_eq!(policy.runtime_replay, "completed_receipt_exact_verify");
        assert_eq!(
            policy.crash_before_private_commit,
            "journal_ready_no_private_marker"
        );
        assert_eq!(
            policy.crash_after_private_commit,
            "journal_ready_exact_private_marker_replay"
        );
        assert_eq!(
            policy.crash_during_runtime_completion,
            "runtime_transaction_rolls_back"
        );
        assert_eq!(
            policy.lost_success_response,
            "exact_completed_receipt_reconstructed"
        );
        assert_eq!(
            policy.retained_representation,
            "immutable_owned_legacy_staging"
        );
        assert!(!policy.live_product_dual_write);
        assert!(!policy.source_deletion);
        assert!(!policy.studio_row_import);
        assert_eq!(policy.host_timestamp, "positive_monotonic_completion_time");
        assert!(!policy.hidden_clock_or_entropy);
    }

    #[test]
    fn implementation_matches_the_governed_import_qualification_policy() {
        let policy = toml::from_str::<ImportQualificationPolicy>(IMPORT_QUALIFICATION_POLICY)
            .expect("import qualification policy");
        assert_eq!(policy.schema_version, 1);
        assert_eq!(
            policy.source_matrix,
            [
                "event_store_v1_to_v4",
                "outbox_v1",
                "private_v1",
                "studio_v1_host_handoff"
            ]
        );
        assert_eq!(
            policy.required_cases,
            [
                "mandatory_backup",
                "unsupported_schema_rejection",
                "mixed_source_golden",
                "bounded_resume",
                "close_reopen",
                "invalid_row_rollback",
                "private_commit_recovery",
                "lost_success_retry",
                "conflicting_identity_rejection",
                "source_retention",
                "no_live_dual_write"
            ]
        );
        assert_eq!(policy.mixed_imported_row_count, 14);
        assert_eq!(policy.mixed_host_handoff_row_count, 0);
        assert!(policy.exact_retry);
        assert!(!policy.hidden_clock_or_entropy);
    }

    fn assert_fixed_schema_policy(
        policy: &FixedSchemaPolicy,
        user_version: i64,
        catalog_sha256: &str,
        source: &str,
        schema_sql_sha256: &str,
    ) {
        assert_eq!(policy.version, 1);
        assert_eq!(policy.user_version, user_version);
        assert_eq!(policy.catalog_sha256, catalog_sha256);
        assert_eq!(policy.source, source);
        assert_eq!(policy.schema_sql_sha256, schema_sql_sha256);
    }

    #[tokio::test]
    async fn preparation_captures_wal_state_and_finalizes_exact_immutable_evidence() {
        let target_root = tempfile::tempdir().expect("target root");
        let legacy_root = tempfile::tempdir().expect("legacy root");
        let backup_root = tempfile::tempdir().expect("backup root");
        let event_path = legacy_root.path().join("event_store.sqlite");
        let studio_path = legacy_root.path().join("studio.sqlite");
        let event_connection = legacy_database(&event_path, "event_envelopes").await;
        let studio_connection = legacy_database(&studio_path, "sdk_studio_state").await;
        let event_source =
            LegacySource::new(LegacySourceKind::EventStore, event_path).expect("event source");
        let studio_source =
            LegacySource::new(LegacySourceKind::Studio, studio_path).expect("studio source");
        let plan = LegacyImportPlan::new(
            LegacyImportId::new([122; 16]).expect("import id"),
            vec![event_source, studio_source],
            backup_root.path(),
            12_200,
        )
        .expect("import plan");
        let (target_paths, store) = target(target_root.path()).await;
        let owned_alias = legacy_root.path().join("owned-alias.sqlite");
        fs::hard_link(target_paths.runtime(), &owned_alias).expect("owned database hard link");
        let alias_plan = LegacyImportPlan::new(
            LegacyImportId::new([124; 16]).expect("alias import id"),
            vec![
                LegacySource::new(LegacySourceKind::Private, owned_alias)
                    .expect("owned database alias source"),
            ],
            backup_root.path(),
            12_201,
        )
        .expect("owned alias import plan");
        assert!(matches!(
            store.prepare_legacy_import(&alias_plan).await,
            Err(Error::InvalidLegacySource(_))
        ));
        let prepared = store
            .prepare_legacy_import(&plan)
            .await
            .expect("prepare import");
        assert_eq!(prepared.import_id(), plan.import_id());
        assert_eq!(prepared.target_generation(), generation(121));
        assert!(prepared.bundle_path().is_dir());
        assert_eq!(prepared.snapshots().len(), 2);
        let manifest_path = prepared.bundle_path().join(LEGACY_MANIFEST);
        let manifest = fs::read_to_string(&manifest_path).expect("legacy import manifest");
        assert_eq!(
            file_digest(&manifest_path).expect("manifest digest"),
            (prepared.manifest_byte_length(), prepared.manifest_sha256())
        );
        assert_eq!(
            manifest,
            format!(
                concat!(
                    "schema_version=1\n",
                    "import_id=7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a\n",
                    "target_generation={}\n",
                    "requested_at_unix_ms=12200\n",
                    "member=event_store|{}|event_store.sqlite|{}|{}\n",
                    "member=studio|{}|studio.sqlite|{}|{}\n"
                ),
                encode_digest(generation(121).as_bytes()),
                encode_hex(plan.sources()[0].path().as_os_str().as_encoded_bytes()),
                prepared.snapshots()[0].byte_length(),
                encode_digest(prepared.snapshots()[0].sha256().as_bytes()),
                encode_hex(plan.sources()[1].path().as_os_str().as_encoded_bytes()),
                prepared.snapshots()[1].byte_length(),
                encode_digest(prepared.snapshots()[1].sha256().as_bytes()),
            )
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&manifest_path)
                    .expect("manifest permissions")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        for evidence in prepared.snapshots() {
            let path = prepared.bundle_path().join(evidence.relative_path());
            assert!(path.is_file());
            assert_eq!(snapshot(evidence.kind(), &path).expect("rehash"), *evidence);
            let mut backup = SqliteConnection::connect_with(
                &SqliteConnectOptions::new().filename(&path).read_only(true),
            )
            .await
            .expect("open immutable backup");
            let select = match evidence.kind() {
                LegacySourceKind::EventStore => "SELECT value FROM event_envelopes",
                LegacySourceKind::Studio => "SELECT value FROM sdk_studio_state",
                _ => unreachable!("test source kind"),
            };
            let row = sqlx::query(select)
                .fetch_one(&mut backup)
                .await
                .expect("latest WAL row");
            assert_eq!(row.get::<i64, _>(0), 41);
            backup.close().await.expect("close immutable backup");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    fs::metadata(path)
                        .expect("backup permissions")
                        .permissions()
                        .mode()
                        & 0o777,
                    0o600
                );
            }
        }
        assert!(matches!(
            store.prepare_legacy_import(&plan).await,
            Err(Error::LegacyImportBackupAlreadyExists(_))
        ));
        assert!(target_paths.runtime().is_file());
        assert!(target_paths.private().is_file());
        event_connection.close().await.expect("close event source");
        studio_connection
            .close()
            .await
            .expect("close studio source");
        store.close().await.expect("close target");
    }

    #[tokio::test]
    async fn classification_accepts_only_exact_studio_handoff_and_reverifies_evidence() {
        let target_root = tempfile::tempdir().expect("target root");
        let legacy_root = tempfile::tempdir().expect("legacy root");
        let backup_root = tempfile::tempdir().expect("backup root");
        let studio_path = legacy_root.path().join("studio.sqlite");
        let studio_connection = supported_studio_database(&studio_path).await;
        let plan = LegacyImportPlan::new(
            LegacyImportId::new([125; 16]).expect("import id"),
            vec![LegacySource::new(LegacySourceKind::Studio, &studio_path).expect("Studio source")],
            backup_root.path(),
            12_500,
        )
        .expect("Studio import plan");
        let (target_paths, store) = target(target_root.path()).await;
        let prepared = store
            .prepare_legacy_import(&plan)
            .await
            .expect("prepared Studio import");
        let classified = store
            .classify_legacy_import(&prepared)
            .await
            .expect("classified Studio import");
        assert_eq!(classified.import_id(), plan.import_id());
        assert_eq!(classified.target_generation(), generation(121));
        assert_eq!(classified.bundle_path(), prepared.bundle_path());
        assert_eq!(classified.sources().len(), 1);
        let source = &classified.sources()[0];
        assert_eq!(source.kind(), LegacySourceKind::Studio);
        assert_eq!(source.schema(), LegacySchema::StudioV1HostHandoff);
        assert_eq!(
            source.schema().disposition(),
            LegacyImportDisposition::HostHandoff
        );
        assert_eq!(source.user_version(), 0);
        assert_eq!(
            encode_digest(source.catalog_sha256().as_bytes()),
            STUDIO_CATALOG_SHA256
        );
        let journal = store
            .begin_legacy_import(&classified, 12_501)
            .await
            .expect("begin durable import");
        assert_eq!(journal.import_id(), plan.import_id());
        assert_eq!(journal.target_generation(), generation(121));
        assert_eq!(journal.manifest_sha256(), prepared.manifest_sha256());
        assert_eq!(
            journal.classification_sha256(),
            classification_digest(&classified)
        );
        assert_eq!(journal.state(), LegacyImportState::Classified);
        assert_eq!(journal.started_at_unix_ms(), 12_501);
        assert_eq!(journal.updated_at_unix_ms(), 12_501);
        assert_eq!(journal.completed_at_unix_ms(), None);
        assert_eq!(journal.members().len(), 1);
        assert_eq!(
            journal.members()[0].classification(),
            &classified.sources()[0]
        );
        assert_eq!(
            journal.members()[0].state(),
            LegacyImportMemberState::Pending
        );
        assert_eq!(journal.members()[0].resume_cursor(), None);
        assert_eq!(journal.members()[0].staged_row_count(), 0);
        assert_eq!(journal.members()[0].updated_at_unix_ms(), 12_501);
        assert_eq!(
            store
                .begin_legacy_import(&classified, 12_599)
                .await
                .expect("idempotent begin"),
            journal
        );
        assert_eq!(
            store
                .legacy_import_journal(plan.import_id())
                .await
                .expect("read journal"),
            Some(journal.clone())
        );
        for statement in [
            "UPDATE radroots_runtime_legacy_imports SET state = 'ready'",
            "UPDATE radroots_runtime_legacy_imports SET import_id = zeroblob(16)",
            "DELETE FROM radroots_runtime_legacy_imports",
            "UPDATE radroots_runtime_legacy_import_members SET state = 'ready'",
            "UPDATE radroots_runtime_legacy_import_members SET source_kind = 'outbox'",
            "DELETE FROM radroots_runtime_legacy_import_members",
        ] {
            assert!(
                sqlx::query(statement).execute(&store.pool).await.is_err(),
                "journal guard accepted `{statement}`"
            );
        }

        let conflicting_backup_root = tempfile::tempdir().expect("conflicting backup root");
        let conflicting_plan = LegacyImportPlan::new(
            LegacyImportId::new([127; 16]).expect("conflicting import id"),
            vec![
                LegacySource::new(LegacySourceKind::Studio, &studio_path)
                    .expect("conflicting Studio source"),
            ],
            conflicting_backup_root.path(),
            12_700,
        )
        .expect("conflicting import plan");
        let conflicting_prepared = store
            .prepare_legacy_import(&conflicting_plan)
            .await
            .expect("conflicting prepared import");
        let conflicting_classified = store
            .classify_legacy_import(&conflicting_prepared)
            .await
            .expect("conflicting classified import");
        assert!(matches!(
            store
                .begin_legacy_import(&conflicting_classified, 12_701)
                .await,
            Err(Error::LegacyImportConflict)
        ));

        let other_root = tempfile::tempdir().expect("other target root");
        let other_paths = Paths::from_directory(other_root.path()).expect("other target paths");
        let other_store = SqliteStorage::open(
            OpenOptions::new(other_paths, OpenMode::Create)
                .with_source_generation(generation(126), 12_600)
                .expect("other source generation"),
        )
        .await
        .expect("other target storage");
        assert!(matches!(
            other_store.classify_legacy_import(&prepared).await,
            Err(Error::LegacyImportTargetMismatch)
        ));
        assert!(matches!(
            other_store.begin_legacy_import(&classified, 12_601).await,
            Err(Error::LegacyImportTargetMismatch)
        ));
        other_store.close().await.expect("close other target");

        fs::write(prepared.bundle_path().join("unexpected"), b"unsupported")
            .expect("unexpected evidence member");
        assert!(matches!(
            store.classify_legacy_import(&prepared).await,
            Err(Error::LegacyImportEvidenceInvalid)
        ));
        studio_connection
            .close()
            .await
            .expect("close Studio source");
        store.close().await.expect("close target");
        let reopened =
            SqliteStorage::open(OpenOptions::new(target_paths, OpenMode::ReadWriteExisting))
                .await
                .expect("reopen target");
        assert_eq!(
            reopened
                .legacy_import_journal(plan.import_id())
                .await
                .expect("read journal after reopen"),
            Some(journal)
        );
        reopened.close().await.expect("close reopened target");
    }

    #[tokio::test]
    async fn event_staging_is_bounded_resumable_atomic_and_isolated_from_live_rows() {
        let target_root = tempfile::tempdir().expect("target root");
        let legacy_root = tempfile::tempdir().expect("legacy root");
        let backup_root = tempfile::tempdir().expect("backup root");
        let event_path = legacy_root.path().join("event_store.sqlite");
        let events = [
            signed_event("one"),
            signed_event("two"),
            signed_event("three"),
        ];
        let event_connection = supported_event_database(&event_path, &events).await;
        let plan = LegacyImportPlan::new(
            LegacyImportId::new([128; 16]).expect("import id"),
            vec![
                LegacySource::new(LegacySourceKind::EventStore, &event_path).expect("event source"),
            ],
            backup_root.path(),
            12_800,
        )
        .expect("event import plan");
        let (target_paths, store) = target(target_root.path()).await;
        let prepared = store
            .prepare_legacy_import(&plan)
            .await
            .expect("prepared event import");
        let classified = store
            .classify_legacy_import(&prepared)
            .await
            .expect("classified event import");
        assert_eq!(classified.sources()[0].schema(), LegacySchema::EventStoreV1);
        store
            .begin_legacy_import(&classified, 12_801)
            .await
            .expect("begin event import");
        assert!(matches!(
            store.stage_legacy_events(&classified, 0, 12_802).await,
            Err(Error::InvalidLegacyImportStageRequest)
        ));

        let first = store
            .stage_legacy_events(&classified, 2, 12_802)
            .await
            .expect("first event page");
        assert_eq!(first.staged_rows(), 2);
        assert_eq!(first.staged_row_count(), 2);
        assert_eq!(first.resume_cursor(), Some(&encode_event_stage_cursor(2)));
        assert!(!first.is_complete());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM radroots_runtime_events")
                .fetch_one(&store.pool)
                .await
                .expect("live event count"),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM radroots_runtime_legacy_event_staging",
            )
            .fetch_one(&store.pool)
            .await
            .expect("staged event count"),
            2
        );
        let journal = store
            .legacy_import_journal(plan.import_id())
            .await
            .expect("staging journal")
            .expect("durable staging journal");
        assert_eq!(journal.state(), LegacyImportState::Staging);
        assert_eq!(
            journal.members()[0].state(),
            LegacyImportMemberState::Staging
        );
        assert_eq!(journal.members()[0].staged_row_count(), 2);
        assert_eq!(
            journal.members()[0].resume_cursor(),
            Some(encode_event_stage_cursor(2).as_slice())
        );

        store.close().await.expect("close target between pages");
        let reopened =
            SqliteStorage::open(OpenOptions::new(target_paths, OpenMode::ReadWriteExisting))
                .await
                .expect("reopen target between pages");
        let second = reopened
            .stage_legacy_events(&classified, 2, 12_803)
            .await
            .expect("second event page");
        assert_eq!(second.staged_rows(), 1);
        assert_eq!(second.staged_row_count(), 3);
        assert_eq!(second.resume_cursor(), Some(&encode_event_stage_cursor(3)));
        assert!(second.is_complete());
        let complete = reopened
            .legacy_import_journal(plan.import_id())
            .await
            .expect("ready journal")
            .expect("durable ready journal");
        assert_eq!(complete.state(), LegacyImportState::Ready);
        assert_eq!(
            complete.members()[0].state(),
            LegacyImportMemberState::Ready
        );
        let retry = reopened
            .stage_legacy_events(&classified, 2, 12_803)
            .await
            .expect("completed retry");
        assert_eq!(retry.staged_rows(), 0);
        assert_eq!(retry.staged_row_count(), 3);
        assert_eq!(retry.resume_cursor(), second.resume_cursor());
        assert!(retry.is_complete());
        let validation = reopened
            .validate_legacy_import(&classified)
            .await
            .expect("validate event import");
        assert_eq!(validation.imported_row_count(), 3);
        assert!(!bytes_are_zero(validation.validation_sha256().as_bytes()));
        for statement in [
            "UPDATE radroots_runtime_legacy_event_staging SET legacy_sequence = 4 WHERE legacy_sequence = 1",
            "DELETE FROM radroots_runtime_legacy_event_staging WHERE legacy_sequence = 1",
        ] {
            assert!(
                sqlx::query(statement)
                    .execute(&reopened.pool)
                    .await
                    .is_err(),
                "staging guard accepted `{statement}`"
            );
        }
        event_connection.close().await.expect("close event source");
        reopened.close().await.expect("close reopened target");
    }

    #[tokio::test]
    async fn invalid_legacy_event_rolls_back_rows_cursor_count_and_state() {
        let target_root = tempfile::tempdir().expect("target root");
        let legacy_root = tempfile::tempdir().expect("legacy root");
        let backup_root = tempfile::tempdir().expect("backup root");
        let event_path = legacy_root.path().join("event_store.sqlite");
        let events = [signed_event("valid"), signed_event("invalid identity")];
        let mut event_connection = supported_event_database(&event_path, &events).await;
        sqlx::query("UPDATE event_envelopes SET event_id = ? WHERE seq = 2")
            .bind("0".repeat(64))
            .execute(&mut event_connection)
            .await
            .expect("corrupt legacy event identity");
        let plan = LegacyImportPlan::new(
            LegacyImportId::new([129; 16]).expect("import id"),
            vec![
                LegacySource::new(LegacySourceKind::EventStore, &event_path).expect("event source"),
            ],
            backup_root.path(),
            12_900,
        )
        .expect("event import plan");
        let (_target_paths, store) = target(target_root.path()).await;
        let prepared = store
            .prepare_legacy_import(&plan)
            .await
            .expect("prepared event import");
        let classified = store
            .classify_legacy_import(&prepared)
            .await
            .expect("classified event import");
        store
            .begin_legacy_import(&classified, 12_901)
            .await
            .expect("begin event import");

        assert!(matches!(
            store.stage_legacy_events(&classified, 2, 12_902).await,
            Err(Error::LegacyImportRowInvalid {
                source_kind: "event_store",
                legacy_sequence: 2,
            })
        ));
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM radroots_runtime_legacy_event_staging",
            )
            .fetch_one(&store.pool)
            .await
            .expect("rolled-back staging count"),
            0
        );
        let journal = store
            .legacy_import_journal(plan.import_id())
            .await
            .expect("rolled-back journal")
            .expect("durable import journal");
        assert_eq!(journal.state(), LegacyImportState::Classified);
        assert_eq!(
            journal.members()[0].state(),
            LegacyImportMemberState::Pending
        );
        assert_eq!(journal.members()[0].resume_cursor(), None);
        assert_eq!(journal.members()[0].staged_row_count(), 0);
        event_connection.close().await.expect("close event source");
        store.close().await.expect("close target");
    }

    #[tokio::test]
    async fn legacy_event_row_conversion_rejects_each_invalid_scalar_boundary() {
        let mut connection = SqliteConnection::connect("sqlite::memory:")
            .await
            .expect("row conversion database");
        let event = signed_event("row conversion matrix");
        #[allow(clippy::too_many_arguments)]
        async fn row(
            connection: &mut SqliteConnection,
            event: &SignedEvent,
            sequence: i64,
            verification_status: &str,
            contract_status: &str,
            projection_eligible: i64,
            inserted_at_ms: i64,
            updated_at_ms: i64,
        ) -> sqlx::sqlite::SqliteRow {
            sqlx::query(
                "SELECT ? AS seq, ? AS event_id, ? AS raw_json,
                        ? AS verification_status, ? AS contract_status,
                        ? AS projection_eligible, ? AS inserted_at_ms, ? AS updated_at_ms",
            )
            .bind(sequence)
            .bind(event.id().to_hex())
            .bind(event.raw_json())
            .bind(verification_status)
            .bind(contract_status)
            .bind(projection_eligible)
            .bind(inserted_at_ms)
            .bind(updated_at_ms)
            .fetch_one(connection)
            .await
            .expect("legacy event row")
        }

        assert!(
            convert_legacy_event_row(
                &row(
                    &mut connection,
                    &event,
                    1,
                    "verified",
                    "accepted",
                    1,
                    10,
                    11
                )
                .await
            )
            .is_ok()
        );
        let long_verification = "v".repeat(65);
        let long_contract = "c".repeat(65);
        for (sequence, verification, contract, eligible, inserted, updated) in [
            (0, "verified", "accepted", 1, 10, 11),
            (1, "", "accepted", 1, 10, 11),
            (1, long_verification.as_str(), "accepted", 1, 10, 11),
            (1, "verified", "", 1, 10, 11),
            (1, "verified", long_contract.as_str(), 1, 10, 11),
            (1, "verified", "accepted", 2, 10, 11),
            (1, "verified", "accepted", 1, 0, 11),
            (1, "verified", "accepted", 1, 10, 9),
        ] {
            let candidate = row(
                &mut connection,
                &event,
                sequence,
                verification,
                contract,
                eligible,
                inserted,
                updated,
            )
            .await;
            assert!(matches!(
                convert_legacy_event_row(&candidate),
                Err(Error::LegacyImportRowInvalid {
                    source_kind: "event_store",
                    legacy_sequence: _
                })
            ));
        }
    }

    #[tokio::test]
    async fn outbox_staging_resumes_across_the_exact_ordered_graph_without_live_mutation() {
        let target_root = tempfile::tempdir().expect("target root");
        let legacy_root = tempfile::tempdir().expect("legacy root");
        let backup_root = tempfile::tempdir().expect("backup root");
        let outbox_path = legacy_root.path().join("outbox.sqlite");
        let outbox_connection = supported_outbox_database(&outbox_path).await;
        let plan = LegacyImportPlan::new(
            LegacyImportId::new([130; 16]).expect("import id"),
            vec![LegacySource::new(LegacySourceKind::Outbox, &outbox_path).expect("outbox source")],
            backup_root.path(),
            13_000,
        )
        .expect("outbox import plan");
        let (target_paths, store) = target(target_root.path()).await;
        let prepared = store
            .prepare_legacy_import(&plan)
            .await
            .expect("prepared outbox import");
        let classified = store
            .classify_legacy_import(&prepared)
            .await
            .expect("classified outbox import");
        assert_eq!(classified.sources()[0].schema(), LegacySchema::OutboxV1);
        store
            .begin_legacy_import(&classified, 13_001)
            .await
            .expect("begin outbox import");
        let expected = [
            LegacyOutboxTable::Operations,
            LegacyOutboxTable::Events,
            LegacyOutboxTable::DeliveryPlans,
            LegacyOutboxTable::DeliveryTargets,
        ];
        for (index, table) in expected.into_iter().enumerate() {
            let page = store
                .stage_legacy_outbox(
                    &classified,
                    1,
                    13_002 + u64::try_from(index).expect("page index"),
                )
                .await
                .expect("outbox graph page");
            assert_eq!(page.table(), table);
            assert_eq!(page.staged_rows(), 1);
            assert!(!page.is_complete());
        }
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM radroots_runtime_outbox_items")
                .fetch_one(&store.pool)
                .await
                .expect("live outbox count"),
            0
        );
        store
            .close()
            .await
            .expect("close target before attempt page");
        let reopened =
            SqliteStorage::open(OpenOptions::new(target_paths, OpenMode::ReadWriteExisting))
                .await
                .expect("reopen target before attempt page");
        let final_page = reopened
            .stage_legacy_outbox(&classified, 1, 13_006)
            .await
            .expect("attempt page");
        assert_eq!(final_page.table(), LegacyOutboxTable::DeliveryAttempts);
        assert_eq!(final_page.staged_rows(), 1);
        assert_eq!(final_page.staged_row_count(), 5);
        assert_eq!(
            final_page.resume_cursor(),
            &encode_outbox_stage_cursor(LegacyOutboxTable::DeliveryAttempts, 1)
        );
        assert!(final_page.is_complete());
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM radroots_runtime_legacy_outbox_staging",
            )
            .fetch_one(&reopened.pool)
            .await
            .expect("staged graph count"),
            5
        );
        let journal = reopened
            .legacy_import_journal(plan.import_id())
            .await
            .expect("outbox journal")
            .expect("durable outbox journal");
        assert_eq!(journal.state(), LegacyImportState::Ready);
        assert_eq!(journal.members()[0].state(), LegacyImportMemberState::Ready);
        assert_eq!(journal.members()[0].staged_row_count(), 5);
        let retry = reopened
            .stage_legacy_outbox(&classified, 1, 13_006)
            .await
            .expect("completed outbox retry");
        assert_eq!(retry.staged_rows(), 0);
        assert_eq!(retry.staged_row_count(), 5);
        assert!(retry.is_complete());
        let validation = reopened
            .validate_legacy_import(&classified)
            .await
            .expect("validate outbox import");
        assert_eq!(validation.imported_row_count(), 5);
        assert!(!bytes_are_zero(validation.validation_sha256().as_bytes()));
        for statement in [
            "UPDATE radroots_runtime_legacy_outbox_staging SET legacy_id = 2 WHERE legacy_id = 1",
            "DELETE FROM radroots_runtime_legacy_outbox_staging WHERE legacy_id = 1",
        ] {
            assert!(
                sqlx::query(statement)
                    .execute(&reopened.pool)
                    .await
                    .is_err(),
                "outbox staging guard accepted `{statement}`"
            );
        }
        outbox_connection
            .close()
            .await
            .expect("close outbox source");
        reopened.close().await.expect("close reopened target");
    }

    #[tokio::test]
    async fn private_staging_recovers_exact_replay_across_both_databases() {
        let target_root = tempfile::tempdir().expect("target root");
        let legacy_root = tempfile::tempdir().expect("legacy root");
        let backup_root = tempfile::tempdir().expect("backup root");
        let private_path = legacy_root.path().join("private.sqlite");
        let private_connection = supported_private_database(&private_path).await;
        let plan = LegacyImportPlan::new(
            LegacyImportId::new([131; 16]).expect("import id"),
            vec![
                LegacySource::new(LegacySourceKind::Private, &private_path)
                    .expect("private source"),
            ],
            backup_root.path(),
            13_100,
        )
        .expect("private import plan");
        let (target_paths, store) = target(target_root.path()).await;
        let prepared = store
            .prepare_legacy_import(&plan)
            .await
            .expect("prepared private import");
        let classified = store
            .classify_legacy_import(&prepared)
            .await
            .expect("classified private import");
        store
            .begin_legacy_import(&classified, 13_101)
            .await
            .expect("begin private import");

        let snapshot = prepared
            .snapshots()
            .iter()
            .find(|snapshot| snapshot.kind() == LegacySourceKind::Private)
            .expect("private snapshot");
        let mut evidence = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(prepared.bundle_path().join(snapshot.relative_path()))
                .read_only(true),
        )
        .await
        .expect("private evidence");
        let row = sqlx::query(private_stage_query(LegacyPrivateTable::Metadata))
            .bind("")
            .bind(2_i64)
            .fetch_one(&mut evidence)
            .await
            .expect("metadata replay row");
        sqlx::query("INSERT INTO radroots_private_legacy_import_staging(import_id, table_kind, key_cursor, parent_key_version, record_json) VALUES (?, 'metadata', ?, NULL, ?)")
            .bind(plan.import_id().as_bytes().as_slice()).bind(row.get::<String,_>("key_cursor")).bind(row.get::<Vec<u8>,_>("record_json")).execute(store.private_pool()).await.expect("simulate private commit before runtime cursor");
        evidence.close().await.expect("close evidence");

        let tables = [
            LegacyPrivateTable::Metadata,
            LegacyPrivateTable::WrappedProfileKeys,
            LegacyPrivateTable::SigningSecrets,
            LegacyPrivateTable::FarmLocations,
            LegacyPrivateTable::TradeArtifacts,
            LegacyPrivateTable::CursorKeys,
            LegacyPrivateTable::Nip46Sessions,
        ];
        for (index, table) in tables.into_iter().enumerate() {
            let page = store
                .stage_legacy_private(
                    &classified,
                    1,
                    13_102 + u64::try_from(index).expect("page index"),
                )
                .await
                .expect("private page");
            assert_eq!(page.table(), table);
            assert_eq!(page.staged_rows(), 1);
            assert!(!page.is_complete());
        }
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM radroots_private_artifacts")
                .fetch_one(store.private_pool())
                .await
                .expect("live private count"),
            0
        );
        store
            .close()
            .await
            .expect("close before final private page");
        let reopened =
            SqliteStorage::open(OpenOptions::new(target_paths, OpenMode::ReadWriteExisting))
                .await
                .expect("reopen private target");
        let final_page = reopened
            .stage_legacy_private(&classified, 1, 13_109)
            .await
            .expect("rotation page");
        assert_eq!(final_page.table(), LegacyPrivateTable::RotationProgress);
        assert_eq!(final_page.staged_rows(), 1);
        assert_eq!(final_page.staged_row_count(), 8);
        assert!(final_page.is_complete());
        let retry = reopened
            .stage_legacy_private(&classified, 1, 13_109)
            .await
            .expect("private completed retry");
        assert_eq!(retry.staged_rows(), 0);
        assert_eq!(retry.staged_row_count(), 8);
        assert!(retry.is_complete());
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM radroots_private_legacy_import_staging"
            )
            .fetch_one(reopened.private_pool())
            .await
            .expect("private staging count"),
            8
        );
        let journal = reopened
            .legacy_import_journal(plan.import_id())
            .await
            .expect("private journal")
            .expect("durable private journal");
        assert_eq!(journal.state(), LegacyImportState::Ready);
        assert_eq!(journal.members()[0].state(), LegacyImportMemberState::Ready);
        let validation = reopened
            .validate_legacy_import(&classified)
            .await
            .expect("validate private import");
        assert_eq!(validation.imported_row_count(), 8);
        assert!(!bytes_are_zero(validation.validation_sha256().as_bytes()));
        assert_eq!(
            reopened
                .validate_legacy_import(&classified)
                .await
                .expect("repeat private validation"),
            validation
        );
        sqlx::query("INSERT INTO radroots_private_legacy_import_commits(import_id, validation_sha256, imported_row_count, committed_at_ms) VALUES (?, ?, 8, 13110)")
            .bind(plan.import_id().as_bytes().as_slice()).bind(validation.validation_sha256().as_bytes().as_slice()).execute(reopened.private_pool()).await.expect("simulate private commit before runtime completion");
        let receipt = reopened
            .finalize_legacy_import(&classified, validation, 13_999)
            .await
            .expect("recover and finalize private import");
        assert_eq!(receipt.validation_sha256(), validation.validation_sha256());
        assert_eq!(receipt.imported_row_count(), 8);
        assert_eq!(receipt.completed_at_unix_ms(), 13_110);
        let completed = reopened
            .legacy_import_journal(plan.import_id())
            .await
            .expect("completed journal")
            .expect("durable completed journal");
        assert_eq!(completed.state(), LegacyImportState::Complete);
        assert_eq!(completed.completed_at_unix_ms(), Some(13_110));
        assert_eq!(
            completed.members()[0].state(),
            LegacyImportMemberState::Complete
        );
        assert_eq!(
            reopened
                .finalize_legacy_import(&classified, validation, 13_999)
                .await
                .expect("lost success response retry"),
            receipt
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM radroots_runtime_legacy_import_commits"
            )
            .fetch_one(reopened.pool())
            .await
            .expect("runtime commit count"),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM radroots_private_legacy_import_commits"
            )
            .fetch_one(reopened.private_pool())
            .await
            .expect("private commit count"),
            1
        );
        for statement in [
            "UPDATE radroots_runtime_legacy_import_commits SET imported_row_count = 9",
            "DELETE FROM radroots_runtime_legacy_import_commits",
        ] {
            assert!(
                sqlx::query(statement)
                    .execute(reopened.pool())
                    .await
                    .is_err(),
                "runtime commit guard accepted `{statement}`"
            );
        }
        for statement in [
            "UPDATE radroots_private_legacy_import_commits SET imported_row_count = 9",
            "DELETE FROM radroots_private_legacy_import_commits",
        ] {
            assert!(
                sqlx::query(statement)
                    .execute(reopened.private_pool())
                    .await
                    .is_err(),
                "private commit guard accepted `{statement}`"
            );
        }
        private_connection
            .close()
            .await
            .expect("close private source");
        reopened.close().await.expect("close private target");
    }

    #[tokio::test]
    async fn studio_handoff_requires_exact_host_receipt_and_imports_no_rows() {
        let target_root = tempfile::tempdir().expect("target root");
        let legacy_root = tempfile::tempdir().expect("legacy root");
        let backup_root = tempfile::tempdir().expect("backup root");
        let host_root = tempfile::tempdir().expect("host root");
        let studio_path = legacy_root.path().join("studio.sqlite");
        let studio_connection = supported_studio_database(&studio_path).await;
        let plan = LegacyImportPlan::new(
            LegacyImportId::new([132; 16]).expect("import id"),
            vec![LegacySource::new(LegacySourceKind::Studio, &studio_path).expect("Studio source")],
            backup_root.path(),
            13_200,
        )
        .expect("Studio import plan");
        let (_, store) = target(target_root.path()).await;
        let prepared = store
            .prepare_legacy_import(&plan)
            .await
            .expect("prepared Studio import");
        let classified = store
            .classify_legacy_import(&prepared)
            .await
            .expect("classified Studio import");
        store
            .begin_legacy_import(&classified, 13_201)
            .await
            .expect("begin Studio import");

        let handoff = store
            .prepare_legacy_studio_handoff(&classified)
            .await
            .expect("Studio handoff");
        assert_eq!(handoff.import_id(), plan.import_id());
        assert!(handoff.evidence_path().starts_with(prepared.bundle_path()));
        assert_eq!(
            handoff.catalog_sha256(),
            classified.sources()[0].catalog_sha256()
        );
        let host_path = host_root.path().join("studio.sqlite");
        std::fs::copy(handoff.evidence_path(), &host_path).expect("host accepts evidence");
        let (host_length, host_commitment) = file_digest(&host_path).expect("host commitment");
        assert_eq!(host_length, handoff.byte_length());
        assert_eq!(host_commitment, handoff.source_sha256());
        assert!(matches!(
            LegacyStudioHandoffReceipt::new(handoff.handoff_sha256(), MemberDigest::new([0; 32])),
            Err(Error::InvalidLegacyImportStageRequest)
        ));
        let receipt = LegacyStudioHandoffReceipt::new(handoff.handoff_sha256(), host_commitment)
            .expect("host receipt");
        let acknowledged = store
            .acknowledge_legacy_studio_handoff(&classified, receipt, 13_202)
            .await
            .expect("acknowledge Studio handoff");
        assert_eq!(acknowledged.state(), LegacyImportState::Ready);
        assert_eq!(
            acknowledged.members()[0].state(),
            LegacyImportMemberState::Ready
        );
        assert_eq!(acknowledged.members()[0].staged_row_count(), 0);
        assert_eq!(
            acknowledged.members()[0].resume_cursor().map(<[u8]>::len),
            Some(64)
        );

        let retry = store
            .acknowledge_legacy_studio_handoff(&classified, receipt, 13_202)
            .await
            .expect("exact receipt retry");
        assert_eq!(retry, acknowledged);
        let validation = store
            .validate_legacy_import(&classified)
            .await
            .expect("validate Studio handoff");
        assert_eq!(validation.imported_row_count(), 0);
        assert!(!bytes_are_zero(validation.validation_sha256().as_bytes()));
        assert_eq!(
            store
                .validate_legacy_import(&classified)
                .await
                .expect("repeat Studio validation"),
            validation
        );
        let conflict =
            LegacyStudioHandoffReceipt::new(handoff.handoff_sha256(), MemberDigest::new([99; 32]))
                .expect("conflicting host receipt");
        assert!(matches!(
            store
                .acknowledge_legacy_studio_handoff(&classified, conflict, 13_203)
                .await,
            Err(Error::LegacyImportConflict)
        ));
        for pool in [store.pool(), store.private_pool()] {
            assert_eq!(
                sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE lower(name) LIKE '%studio%'"
                )
                .fetch_one(pool)
                .await
                .expect("Studio schema isolation"),
                0
            );
        }
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM radroots_runtime_events")
                .fetch_one(store.pool())
                .await
                .expect("runtime event isolation"),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM radroots_private_artifacts")
                .fetch_one(store.private_pool())
                .await
                .expect("private artifact isolation"),
            0
        );
        sqlx::query(
            "UPDATE radroots_runtime_legacy_import_members
             SET staged_row_count = 1, updated_at_ms = 13204
             WHERE import_id = ? AND source_kind = 'studio'",
        )
        .bind(plan.import_id().as_bytes().as_slice())
        .execute(store.pool())
        .await
        .expect("simulate inconsistent Studio count");
        assert!(matches!(
            store.validate_legacy_import(&classified).await,
            Err(Error::LegacyImportConflict)
        ));
        studio_connection
            .close()
            .await
            .expect("close Studio source");
        store.close().await.expect("close target");
    }

    #[tokio::test]
    async fn mixed_source_import_qualifies_backup_resume_validation_and_completion() {
        let target_root = tempfile::tempdir().expect("target root");
        let legacy_root = tempfile::tempdir().expect("legacy root");
        let backup_root = tempfile::tempdir().expect("backup root");
        let event_path = legacy_root.path().join("event_store.sqlite");
        let outbox_path = legacy_root.path().join("outbox.sqlite");
        let private_path = legacy_root.path().join("private.sqlite");
        let studio_path = legacy_root.path().join("studio.sqlite");
        let event_connection =
            supported_event_database(&event_path, &[signed_event("mixed")]).await;
        let outbox_connection = supported_outbox_database(&outbox_path).await;
        let private_connection = supported_private_database(&private_path).await;
        let studio_connection = supported_studio_database(&studio_path).await;
        let plan = LegacyImportPlan::new(
            LegacyImportId::new([133; 16]).expect("import id"),
            vec![
                LegacySource::new(LegacySourceKind::Studio, &studio_path).expect("Studio source"),
                LegacySource::new(LegacySourceKind::Private, &private_path)
                    .expect("private source"),
                LegacySource::new(LegacySourceKind::Outbox, &outbox_path).expect("outbox source"),
                LegacySource::new(LegacySourceKind::EventStore, &event_path).expect("event source"),
            ],
            backup_root.path(),
            14_000,
        )
        .expect("mixed import plan");
        let (target_paths, store) = target(target_root.path()).await;
        let prepared = store
            .prepare_legacy_import(&plan)
            .await
            .expect("prepare mixed import");
        assert_eq!(prepared.snapshots().len(), 4);
        let classified = store
            .classify_legacy_import(&prepared)
            .await
            .expect("classify mixed import");
        store
            .begin_legacy_import(&classified, 14_001)
            .await
            .expect("begin mixed import");

        assert!(
            store
                .stage_legacy_events(&classified, 1, 14_002)
                .await
                .expect("stage mixed event")
                .is_complete()
        );
        for page in 0_u64..5 {
            let result = store
                .stage_legacy_outbox(&classified, 1, 14_003 + page)
                .await
                .expect("stage mixed outbox");
            assert_eq!(result.is_complete(), page == 4);
        }
        for page in 0_u64..8 {
            let result = store
                .stage_legacy_private(&classified, 1, 14_008 + page)
                .await
                .expect("stage mixed private");
            assert_eq!(result.is_complete(), page == 7);
        }
        let handoff = store
            .prepare_legacy_studio_handoff(&classified)
            .await
            .expect("prepare mixed Studio handoff");
        let host_receipt =
            LegacyStudioHandoffReceipt::new(handoff.handoff_sha256(), MemberDigest::new([88; 32]))
                .expect("mixed Studio host receipt");
        store
            .acknowledge_legacy_studio_handoff(&classified, host_receipt, 14_016)
            .await
            .expect("acknowledge mixed Studio handoff");
        let validation = store
            .validate_legacy_import(&classified)
            .await
            .expect("validate mixed import");
        assert_eq!(validation.imported_row_count(), 14);
        store.close().await.expect("close before mixed finalize");

        let reopened =
            SqliteStorage::open(OpenOptions::new(target_paths, OpenMode::ReadWriteExisting))
                .await
                .expect("reopen mixed import");
        let receipt = reopened
            .finalize_legacy_import(&classified, validation, 14_017)
            .await
            .expect("finalize mixed import");
        assert_eq!(receipt.imported_row_count(), 14);
        assert_eq!(receipt.validation_sha256(), validation.validation_sha256());
        assert_eq!(
            reopened
                .finalize_legacy_import(&classified, validation, 14_999)
                .await
                .expect("retry mixed completion"),
            receipt
        );
        let journal = reopened
            .legacy_import_journal(plan.import_id())
            .await
            .expect("mixed journal")
            .expect("durable mixed journal");
        assert_eq!(journal.state(), LegacyImportState::Complete);
        assert!(
            journal
                .members()
                .iter()
                .all(|member| member.state() == LegacyImportMemberState::Complete)
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM radroots_runtime_events")
                .fetch_one(reopened.pool())
                .await
                .expect("mixed live event isolation"),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM radroots_private_artifacts")
                .fetch_one(reopened.private_pool())
                .await
                .expect("mixed live private isolation"),
            0
        );
        for source in plan.sources() {
            assert!(source.path().is_file(), "predecessor source was removed");
        }
        event_connection.close().await.expect("close event source");
        outbox_connection
            .close()
            .await
            .expect("close outbox source");
        private_connection
            .close()
            .await
            .expect("close private source");
        studio_connection
            .close()
            .await
            .expect("close Studio source");
        reopened.close().await.expect("close mixed target");
    }

    #[test]
    fn pure_catalog_classification_rejects_mixed_unknown_and_newer_schemas() {
        let studio = vec![CatalogRow {
            object_type: "table".to_owned(),
            name: "sdk_studio_state".to_owned(),
            table_name: "sdk_studio_state".to_owned(),
            sql: Some(
                "CREATE TABLE sdk_studio_state (\n  key TEXT PRIMARY KEY NOT NULL,\n  value_json TEXT NOT NULL,\n  updated_at_ms INTEGER NOT NULL\n)"
                    .to_owned(),
            ),
        }];
        assert_eq!(
            classify_fixed_catalog(
                LegacySourceKind::Studio,
                0,
                &studio,
                0,
                STUDIO_CATALOG_SHA256,
            )
            .expect("exact Studio schema"),
            LegacySchema::StudioV1HostHandoff
        );
        assert!(matches!(
            classify_fixed_catalog(
                LegacySourceKind::Outbox,
                0,
                &studio,
                0,
                OUTBOX_CATALOG_SHA256,
            ),
            Err(Error::UnsupportedLegacySchema { .. })
        ));
        let mut unknown = studio.clone();
        unknown.push(CatalogRow {
            object_type: "table".to_owned(),
            name: "unknown".to_owned(),
            table_name: "unknown".to_owned(),
            sql: Some("CREATE TABLE unknown(value INTEGER)".to_owned()),
        });
        assert!(matches!(
            classify_fixed_catalog(
                LegacySourceKind::Studio,
                0,
                &unknown,
                0,
                STUDIO_CATALOG_SHA256,
            ),
            Err(Error::UnsupportedLegacySchema { .. })
        ));
        assert!(matches!(
            classify_fixed_catalog(
                LegacySourceKind::Studio,
                2,
                &studio,
                0,
                STUDIO_CATALOG_SHA256,
            ),
            Err(Error::UnsupportedLegacySchema { .. })
        ));
    }

    #[tokio::test]
    async fn event_store_history_requires_the_exact_contiguous_governed_ledger() {
        let mut connection = SqliteConnection::connect("sqlite::memory:")
            .await
            .expect("event-store history database");
        sqlx::query(EVENT_STORE_LEDGER_DDL)
            .execute(&mut connection)
            .await
            .expect("event-store ledger");
        for migration in EVENT_STORE_MIGRATIONS {
            sqlx::query(
                "INSERT INTO radroots_event_store_schema_migrations(version, name, up_sha256, down_sha256, schema_sha256) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(i64::from(migration.version))
            .bind(migration.name)
            .bind(migration.up_sha256)
            .bind(migration.down_sha256)
            .bind(migration.schema_sha256)
            .execute(&mut connection)
            .await
            .expect("event-store history row");
        }
        assert_eq!(
            validate_event_history(&mut connection)
                .await
                .expect("exact event-store history"),
            4
        );
        sqlx::query(
            "UPDATE radroots_event_store_schema_migrations SET up_sha256 = ? WHERE version = 4",
        )
        .bind("0".repeat(64))
        .execute(&mut connection)
        .await
        .expect("tamper history");
        assert!(matches!(
            validate_event_history(&mut connection).await,
            Err(Error::LegacyImportMigrationHistoryInvalid)
        ));
        connection.close().await.expect("close history database");
    }

    #[test]
    fn plans_reject_zero_identity_duplicates_missing_paths_and_symlinks() {
        let root = tempfile::tempdir().expect("root");
        let backup_root = tempfile::tempdir().expect("backup root");
        let source_path = root.path().join("event.sqlite");
        fs::write(&source_path, b"not yet inspected").expect("source file");
        assert!(matches!(
            LegacyImportId::new([0; 16]),
            Err(Error::InvalidLegacyImportPlan)
        ));
        let source =
            LegacySource::new(LegacySourceKind::EventStore, &source_path).expect("regular source");
        let import_id = LegacyImportId::new([123; 16]).expect("import id");
        assert!(matches!(
            LegacyImportPlan::new(import_id, Vec::new(), backup_root.path(), 12_300),
            Err(Error::InvalidLegacyImportPlan)
        ));
        assert!(matches!(
            LegacyImportPlan::new(import_id, vec![source.clone()], backup_root.path(), 0),
            Err(Error::InvalidLegacyImportPlan)
        ));
        assert!(matches!(
            LegacyImportPlan::new(
                import_id,
                vec![source.clone(); LEGACY_SOURCE_MAX + 1],
                backup_root.path(),
                12_300,
            ),
            Err(Error::InvalidLegacyImportPlan)
        ));
        assert!(matches!(
            LegacyImportPlan::new(
                import_id,
                vec![source.clone(), source],
                backup_root.path(),
                12_300,
            ),
            Err(Error::InvalidLegacyImportPlan)
        ));
        assert!(matches!(
            LegacySource::new(LegacySourceKind::Outbox, root.path().join("missing.sqlite")),
            Err(Error::InvalidLegacySource(_))
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let alias = root.path().join("alias.sqlite");
            symlink(&source_path, &alias).expect("source symlink");
            assert!(matches!(
                LegacySource::new(LegacySourceKind::Private, alias),
                Err(Error::InvalidLegacySource(_))
            ));
        }
    }

    #[test]
    fn stage_cursors_reject_every_malformed_boundary() {
        assert_eq!(
            decode_outbox_stage_cursor(None).expect("initial outbox cursor"),
            (LegacyOutboxTable::Operations, 0)
        );
        for table in [
            LegacyOutboxTable::Operations,
            LegacyOutboxTable::Events,
            LegacyOutboxTable::DeliveryPlans,
            LegacyOutboxTable::DeliveryTargets,
            LegacyOutboxTable::DeliveryAttempts,
        ] {
            let encoded = encode_outbox_stage_cursor(table, 1);
            assert_eq!(
                decode_outbox_stage_cursor(Some(&encoded)).expect("outbox cursor"),
                (table, 1)
            );
        }
        for corrupt in [
            Vec::new(),
            vec![0; 8],
            vec![0; 9],
            encode_outbox_stage_cursor(LegacyOutboxTable::Operations, -1).to_vec(),
        ] {
            assert!(matches!(
                decode_outbox_stage_cursor(Some(&corrupt)),
                Err(Error::InvalidLegacyImportJournal)
            ));
        }

        assert_eq!(
            decode_private_stage_cursor(None).expect("initial private cursor"),
            (LegacyPrivateTable::Metadata, String::new())
        );
        for table in [
            LegacyPrivateTable::Metadata,
            LegacyPrivateTable::WrappedProfileKeys,
            LegacyPrivateTable::SigningSecrets,
            LegacyPrivateTable::FarmLocations,
            LegacyPrivateTable::TradeArtifacts,
            LegacyPrivateTable::CursorKeys,
            LegacyPrivateTable::Nip46Sessions,
            LegacyPrivateTable::RotationProgress,
        ] {
            let encoded = encode_private_stage_cursor(table, "cursor");
            assert_eq!(
                decode_private_stage_cursor(Some(&encoded)).expect("private cursor"),
                (table, "cursor".to_owned())
            );
            assert!(!private_stage_query(table).is_empty());
        }
        for corrupt in [Vec::new(), vec![0], vec![1; 1026], vec![1, 0xff]] {
            assert!(matches!(
                decode_private_stage_cursor(Some(&corrupt)),
                Err(Error::InvalidLegacyImportJournal)
            ));
        }

        assert_eq!(
            decode_event_stage_cursor(None).expect("initial event cursor"),
            0
        );
        let event_cursor = encode_event_stage_cursor(1);
        assert_eq!(
            decode_event_stage_cursor(Some(&event_cursor)).expect("event cursor"),
            1
        );
        for corrupt in [Vec::new(), vec![0; 8], (-1_i64).to_be_bytes().to_vec()] {
            assert!(matches!(
                decode_event_stage_cursor(Some(&corrupt)),
                Err(Error::InvalidLegacyImportJournal)
            ));
        }
        assert!(matches!(
            decode_positive_time(-1),
            Err(Error::InvalidLegacyImportJournal)
        ));
        assert!(matches!(
            decode_positive_time(0),
            Err(Error::InvalidLegacyImportJournal)
        ));
        assert_eq!(decode_positive_time(1).expect("positive time"), 1);
    }
}

//! Explicit one-shot legacy import planning and immutable source backup.

use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
};

use radroots_storage::{backup::MemberDigest, event::SourceGeneration, status::EventStoreMode};
use sha2::{Digest, Sha256};
use sqlx::{Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};

use crate::{Error, SqliteStorage};

const LEGACY_SOURCE_MAX: usize = 4;
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
    import_id: LegacyImportId,
    target_generation: SourceGeneration,
    bundle_path: PathBuf,
    sources: Vec<LegacySourceClassification>,
}

impl ClassifiedLegacyImport {
    /// Returns the stable import-attempt identity.
    pub const fn import_id(&self) -> LegacyImportId {
        self.import_id
    }

    /// Returns the exact destination storage generation.
    pub const fn target_generation(&self) -> SourceGeneration {
        self.target_generation
    }

    /// Returns the reverified finalized evidence bundle.
    pub fn bundle_path(&self) -> &Path {
        &self.bundle_path
    }

    /// Returns exact classifications in stable source-family order.
    pub fn sources(&self) -> &[LegacySourceClassification] {
        &self.sources
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
            import_id: prepared.import_id(),
            target_generation: prepared.target_generation(),
            bundle_path: prepared.bundle_path().to_path_buf(),
            sources,
        })
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

fn snapshot(kind: LegacySourceKind, path: &Path) -> Result<LegacySourceSnapshot, Error> {
    let (byte_length, sha256) = file_digest(path)?;
    Ok(LegacySourceSnapshot {
        kind,
        relative_path: kind.backup_file_name().to_owned(),
        byte_length,
        sha256,
    })
}

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
mod tests {
    use radroots_storage::event::SourceGeneration;
    use serde::Deserialize;

    use crate::{OpenMode, OpenOptions, Paths};

    use super::*;

    const POLICY: &str =
        include_str!("../../../contracts/storage/legacy_import_backup_policy_v1.toml");
    const CLASSIFICATION_POLICY: &str =
        include_str!("../../../contracts/storage/legacy_schema_classification_v1.toml");

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
            vec![LegacySource::new(LegacySourceKind::Studio, studio_path).expect("Studio source")],
            backup_root.path(),
            12_500,
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
        assert!(matches!(
            LegacyImportPlan::new(
                LegacyImportId::new([123; 16]).expect("import id"),
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
}

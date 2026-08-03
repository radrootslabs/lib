//! Explicit one-shot legacy import planning and immutable source backup.

use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
};

use radroots_storage::{backup::MemberDigest, status::EventStoreMode};
use sha2::{Digest, Sha256};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

use crate::{Error, SqliteStorage};

const LEGACY_SOURCE_MAX: usize = 4;
const LEGACY_MANIFEST: &str = "manifest.v1";

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
    bundle_path: PathBuf,
    snapshots: Vec<LegacySourceSnapshot>,
}

impl PreparedLegacyImport {
    /// Returns the stable import-attempt identity.
    pub const fn import_id(&self) -> LegacyImportId {
        self.import_id
    }

    /// Returns the finalized immutable evidence bundle.
    pub fn bundle_path(&self) -> &Path {
        &self.bundle_path
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
        write_manifest(plan, &snapshots, &layout.staging.join(LEGACY_MANIFEST))?;
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
            bundle_path: layout.finalized,
            snapshots,
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
    let mut file = File::open(path).map_err(|source| Error::LegacyImportFilesystem {
        operation: "open legacy import backup member",
        source,
    })?;
    file.sync_all()
        .map_err(|source| Error::LegacyImportFilesystem {
            operation: "sync legacy import backup member",
            source,
        })?;
    let byte_length = file
        .metadata()
        .map_err(|source| Error::LegacyImportFilesystem {
            operation: "inspect legacy import backup member",
            source,
        })?
        .len();
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 16 * 1_024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| Error::LegacyImportFilesystem {
                operation: "hash legacy import backup member",
                source,
            })?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(LegacySourceSnapshot {
        kind,
        relative_path: kind.backup_file_name().to_owned(),
        byte_length,
        sha256: MemberDigest::new(digest.finalize().into()),
    })
}

fn write_manifest(
    plan: &LegacyImportPlan,
    snapshots: &[LegacySourceSnapshot],
    path: &Path,
) -> Result<(), Error> {
    let mut body = format!(
        "schema_version=1\nimport_id={}\nrequested_at_unix_ms={}\n",
        encode_id(plan.import_id().as_bytes()),
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
    use sqlx::Row;

    use crate::{OpenMode, OpenOptions, Paths};

    use super::*;

    const POLICY: &str =
        include_str!("../../../contracts/storage/legacy_import_backup_policy_v1.toml");

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
            "manifest.v1_exact_identity_timestamp_source_provenance_inventory_lengths_sha256"
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
        assert!(prepared.bundle_path().is_dir());
        assert_eq!(prepared.snapshots().len(), 2);
        let manifest_path = prepared.bundle_path().join(LEGACY_MANIFEST);
        let manifest = fs::read_to_string(&manifest_path).expect("legacy import manifest");
        assert_eq!(
            manifest,
            format!(
                concat!(
                    "schema_version=1\n",
                    "import_id=7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a\n",
                    "requested_at_unix_ms=12200\n",
                    "member=event_store|{}|event_store.sqlite|{}|{}\n",
                    "member=studio|{}|studio.sqlite|{}|{}\n"
                ),
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

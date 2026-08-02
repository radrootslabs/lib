//! Consistent SQLite backup capture and bundle layout.

use std::{
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
};

use radroots_storage::backup::{
    BackupFormatVersion, BackupManifest, BackupMember, BackupMemberKind, BackupPlan,
    BackupSecretPolicy, MemberDigest,
};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::{Error, SqliteStorage};

const RUNTIME_DATABASE: &str = "runtime.sqlite";
const PRIVATE_DATABASE: &str = "private.sqlite";
const RUNTIME_MEMBER: &str = "runtime/runtime.sqlite";
const PRIVATE_MEMBER: &str = "private/private.sqlite";

impl SqliteStorage {
    /// Captures consistent SQLite snapshots into a new deterministic staging
    /// bundle under the configured host-owned backup root.
    pub async fn capture_backup(&self, plan: &BackupPlan) -> Result<BackupManifest, Error> {
        self.lifecycle
            .require_open()
            .map_err(|_| Error::BackupBackendUnavailable)?;
        if plan.format_version() != BackupFormatVersion::V1 {
            return Err(Error::UnsupportedBackupVersion);
        }
        let backup_root = self
            .backup_root
            .as_deref()
            .ok_or(Error::BackupRootRequired)?;
        validate_backup_root(backup_root)?;
        let layout = BackupLayout::new(backup_root, plan);
        layout.create(plan.secret_policy())?;

        let mut members = Vec::with_capacity(
            if plan.secret_policy() == BackupSecretPolicy::IncludeProtectedStorage {
                2
            } else {
                1
            },
        );
        members.push(
            capture_member(
                &self.pool,
                &layout.runtime_file,
                RUNTIME_MEMBER,
                BackupMemberKind::Runtime,
            )
            .await?,
        );
        sync_directory(&layout.runtime_directory, "sync runtime member directory")?;

        if plan.secret_policy() == BackupSecretPolicy::IncludeProtectedStorage {
            members.push(
                capture_member(
                    &self.private_pool,
                    &layout.private_file,
                    PRIVATE_MEMBER,
                    BackupMemberKind::Protected,
                )
                .await?,
            );
            sync_directory(&layout.private_directory, "sync private member directory")?;
        }
        sync_directory(&layout.staging, "sync staging bundle directory")?;
        sync_directory(backup_root, "sync backup root")?;

        BackupManifest::new(
            plan.format_version(),
            plan.backup_id(),
            plan.requested_at_unix_ms(),
            plan.secret_policy(),
            members,
        )
        .map_err(|_| Error::BackupCaptureFailed { member: "manifest" })
    }
}

pub(crate) fn validate_backup_root(path: &Path) -> Result<(), Error> {
    if !path.is_absolute()
        || path.to_str().is_none()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(Error::InvalidBackupRoot(path.to_path_buf()));
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(Error::InvalidBackupRoot(path.to_path_buf()))
        }
        Ok(_) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            Err(Error::InvalidBackupRoot(path.to_path_buf()))
        }
        Err(source) => Err(Error::BackupFilesystem {
            operation: "inspect backup root",
            source,
        }),
    }
}

struct BackupLayout {
    staging: PathBuf,
    finalized: PathBuf,
    runtime_directory: PathBuf,
    private_directory: PathBuf,
    runtime_file: PathBuf,
    private_file: PathBuf,
}

impl BackupLayout {
    fn new(root: &Path, plan: &BackupPlan) -> Self {
        let id = encode_backup_id(plan);
        let staging = root.join(format!(".radroots-backup-{id}.staging"));
        let finalized = root.join(format!("radroots-backup-{id}"));
        let runtime_directory = staging.join("runtime");
        let private_directory = staging.join("private");
        let runtime_file = runtime_directory.join(RUNTIME_DATABASE);
        let private_file = private_directory.join(PRIVATE_DATABASE);
        Self {
            staging,
            finalized,
            runtime_directory,
            private_directory,
            runtime_file,
            private_file,
        }
    }

    fn create(&self, secret_policy: BackupSecretPolicy) -> Result<(), Error> {
        for path in [&self.staging, &self.finalized] {
            if path
                .try_exists()
                .map_err(|source| Error::BackupFilesystem {
                    operation: "inspect bundle path",
                    source,
                })?
            {
                return Err(Error::BackupBundleAlreadyExists(path.clone()));
            }
        }
        create_private_directory(&self.staging, "create staging bundle")?;
        create_private_directory(&self.runtime_directory, "create runtime member directory")?;
        if secret_policy == BackupSecretPolicy::IncludeProtectedStorage {
            create_private_directory(&self.private_directory, "create private member directory")?;
        }
        Ok(())
    }
}

fn encode_backup_id(plan: &BackupPlan) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(32);
    for byte in plan.backup_id().as_bytes() {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn create_private_directory(path: &Path, operation: &'static str) -> Result<(), Error> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(path)
        .map_err(|source| Error::BackupFilesystem { operation, source })
}

async fn capture_member(
    pool: &SqlitePool,
    destination: &Path,
    relative_path: &'static str,
    kind: BackupMemberKind,
) -> Result<BackupMember, Error> {
    let destination = destination
        .to_str()
        .ok_or_else(|| Error::InvalidBackupRoot(destination.to_path_buf()))?;
    let mut connection = pool
        .acquire()
        .await
        .map_err(|_| Error::BackupBackendUnavailable)?;
    sqlx::query("VACUUM INTO ?")
        .bind(destination)
        .execute(&mut *connection)
        .await
        .map_err(|_| Error::BackupCaptureFailed {
            member: relative_path,
        })?;
    drop(connection);
    member_from_file(Path::new(destination), relative_path, kind)
}

fn member_from_file(
    path: &Path,
    relative_path: &'static str,
    kind: BackupMemberKind,
) -> Result<BackupMember, Error> {
    let mut file = File::open(path).map_err(|source| Error::BackupFilesystem {
        operation: "open captured member",
        source,
    })?;
    file.sync_all().map_err(|source| Error::BackupFilesystem {
        operation: "sync captured member",
        source,
    })?;
    let byte_length = file
        .metadata()
        .map_err(|source| Error::BackupFilesystem {
            operation: "inspect captured member",
            source,
        })?
        .len();
    let mut sha256 = Sha256::new();
    let mut buffer = [0_u8; 16 * 1_024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| Error::BackupFilesystem {
                operation: "hash captured member",
                source,
            })?;
        if read == 0 {
            break;
        }
        sha256.update(&buffer[..read]);
    }
    BackupMember::new(
        relative_path,
        kind,
        byte_length,
        MemberDigest::new(sha256.finalize().into()),
    )
    .map_err(|_| Error::BackupCaptureFailed {
        member: relative_path,
    })
}

fn sync_directory(path: &Path, operation: &'static str) -> Result<(), Error> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| Error::BackupFilesystem { operation, source })
}

#[cfg(test)]
mod tests {
    use radroots_storage::{
        backup::{BackupFormatVersion, BackupId, BackupMemberKind, BackupPlan, BackupSecretPolicy},
        event::SourceGeneration,
    };
    use serde::Deserialize;
    use sqlx::{Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};

    use crate::{OpenMode, OpenOptions, Paths};

    use super::*;

    const POLICY: &str = include_str!("../../../contracts/storage/backup_capture_policy_v1.toml");

    #[derive(Deserialize)]
    struct Policy {
        schema_version: u32,
        format_version: u16,
        backup_root: String,
        staging_name: String,
        final_name: String,
        capture: String,
        runtime_member: String,
        protected_member: String,
        exclude_protected_members: Vec<String>,
        include_protected_members: Vec<String>,
        created_at: String,
        member_digest: String,
        member_length: String,
        filesystem_sync: Vec<String>,
        existing_staging_or_final: String,
        hidden_clock: bool,
        unsafe_ffi: bool,
    }

    fn generation(byte: u8) -> SourceGeneration {
        SourceGeneration::new([byte; 32]).expect("source generation")
    }

    fn plan(byte: u8, policy: BackupSecretPolicy, at: u64) -> BackupPlan {
        BackupPlan::new(
            BackupId::new([byte; 16]).expect("backup id"),
            BackupFormatVersion::V1,
            policy,
            at,
        )
        .expect("backup plan")
    }

    async fn create(database_root: &Path, backup_root: Option<&Path>) -> (Paths, SqliteStorage) {
        let paths = Paths::from_directory(database_root).expect("owned paths");
        let mut options = OpenOptions::new(paths.clone(), OpenMode::Create)
            .with_source_generation(generation(91), 9_100)
            .expect("source generation");
        if let Some(root) = backup_root {
            options = options.with_backup_root(root).expect("backup root");
        }
        let store = SqliteStorage::open(options).await.expect("create storage");
        (paths, store)
    }

    async fn scalar(path: &Path, query: &'static str) -> i64 {
        let mut connection = SqliteConnection::connect_with(
            &SqliteConnectOptions::new().filename(path).read_only(true),
        )
        .await
        .expect("open captured member");
        let value = sqlx::query(query)
            .fetch_one(&mut connection)
            .await
            .expect("query captured member")
            .try_get(0)
            .expect("decode captured member");
        connection.close().await.expect("close captured member");
        value
    }

    #[test]
    fn implementation_matches_the_governed_backup_capture_policy() {
        let policy = toml::from_str::<Policy>(POLICY).expect("backup capture policy");
        assert_eq!(policy.schema_version, 1);
        assert_eq!(policy.format_version, 1);
        assert_eq!(
            policy.backup_root,
            "explicit_existing_host_owned_absolute_utf8_directory"
        );
        assert_eq!(
            policy.staging_name,
            ".radroots-backup-{backup_id_hex}.staging"
        );
        assert_eq!(policy.final_name, "radroots-backup-{backup_id_hex}");
        assert_eq!(policy.capture, "sqlite_vacuum_into");
        assert_eq!(policy.runtime_member, RUNTIME_MEMBER);
        assert_eq!(policy.protected_member, PRIVATE_MEMBER);
        assert_eq!(policy.exclude_protected_members, [RUNTIME_MEMBER]);
        assert_eq!(
            policy.include_protected_members,
            [RUNTIME_MEMBER, PRIVATE_MEMBER]
        );
        assert_eq!(policy.created_at, "plan_requested_at_unix_ms");
        assert_eq!(policy.member_digest, "sha256");
        assert_eq!(policy.member_length, "exact_bytes");
        assert_eq!(
            policy.filesystem_sync,
            [
                "member_file",
                "member_directory",
                "staging_directory",
                "backup_root"
            ]
        );
        assert_eq!(policy.existing_staging_or_final, "reject");
        assert!(!policy.hidden_clock);
        assert!(!policy.unsafe_ffi);
    }

    #[tokio::test]
    async fn capture_excludes_protected_storage_and_includes_latest_wal_state() {
        let database_root = tempfile::tempdir().expect("database root");
        let backup_parent = tempfile::tempdir().expect("backup parent");
        let backup_root = backup_parent.path().join("backups");
        fs::create_dir(&backup_root).expect("backup root");
        let (_, store) = create(database_root.path(), Some(&backup_root)).await;
        sqlx::raw_sql(
            "CREATE TABLE runtime_backup_probe (value INTEGER NOT NULL);
             INSERT INTO runtime_backup_probe (value) VALUES (41);",
        )
        .execute(&store.pool)
        .await
        .expect("runtime WAL mutation");
        sqlx::raw_sql(
            "CREATE TABLE private_backup_probe (value INTEGER NOT NULL);
             INSERT INTO private_backup_probe (value) VALUES (42);",
        )
        .execute(&store.private_pool)
        .await
        .expect("private WAL mutation");

        let plan = plan(92, BackupSecretPolicy::ExcludeProtectedStorage, 9_200);
        let manifest = store.capture_backup(&plan).await.expect("capture backup");
        let layout = BackupLayout::new(&backup_root, &plan);
        assert_eq!(manifest.created_at_unix_ms(), 9_200);
        assert_eq!(
            manifest.secret_policy(),
            BackupSecretPolicy::ExcludeProtectedStorage
        );
        assert_eq!(manifest.members().len(), 1);
        let runtime = &manifest.members()[0];
        assert_eq!(runtime.relative_path(), RUNTIME_MEMBER);
        assert_eq!(runtime.kind(), BackupMemberKind::Runtime);
        assert_eq!(
            runtime.byte_length(),
            fs::metadata(&layout.runtime_file)
                .expect("runtime metadata")
                .len()
        );
        assert_eq!(
            runtime.sha256(),
            member_from_file(
                &layout.runtime_file,
                RUNTIME_MEMBER,
                BackupMemberKind::Runtime
            )
            .expect("rehash runtime member")
            .sha256()
        );
        assert_eq!(
            scalar(
                &layout.runtime_file,
                "SELECT value FROM runtime_backup_probe"
            )
            .await,
            41
        );
        assert!(!layout.private_directory.exists());
        assert!(!layout.finalized.exists());
        assert!(matches!(
            store.capture_backup(&plan).await,
            Err(Error::BackupBundleAlreadyExists(path)) if path == layout.staging
        ));
    }

    #[tokio::test]
    async fn read_only_capture_includes_protected_member_and_rejects_invalid_lifecycle() {
        let database_root = tempfile::tempdir().expect("database root");
        let backup_parent = tempfile::tempdir().expect("backup parent");
        let backup_root = backup_parent.path().join("backups");
        fs::create_dir(&backup_root).expect("backup root");
        let (paths, writer) = create(database_root.path(), Some(&backup_root)).await;
        writer.close().await.expect("close writer");

        let reader = SqliteStorage::open(
            OpenOptions::new(paths, OpenMode::ReadOnly)
                .with_backup_root(&backup_root)
                .expect("backup root"),
        )
        .await
        .expect("read-only storage");
        let include_plan = plan(93, BackupSecretPolicy::IncludeProtectedStorage, 9_300);
        let manifest = reader
            .capture_backup(&include_plan)
            .await
            .expect("read-only capture");
        let layout = BackupLayout::new(&backup_root, &include_plan);
        assert_eq!(manifest.members().len(), 2);
        assert_eq!(manifest.members()[1].relative_path(), PRIVATE_MEMBER);
        assert_eq!(manifest.members()[1].kind(), BackupMemberKind::Protected);
        assert_eq!(
            scalar(
                &layout.private_file,
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'radroots_private_artifacts'"
            )
            .await,
            1
        );

        let unsupported = BackupPlan::new(
            BackupId::new([94; 16]).expect("backup id"),
            BackupFormatVersion::new(2).expect("version"),
            BackupSecretPolicy::ExcludeProtectedStorage,
            9_400,
        )
        .expect("unsupported plan");
        assert!(matches!(
            reader.capture_backup(&unsupported).await,
            Err(Error::UnsupportedBackupVersion)
        ));
        reader.close().await.expect("close reader");
        assert!(matches!(
            reader
                .capture_backup(&plan(
                    95,
                    BackupSecretPolicy::ExcludeProtectedStorage,
                    9_500
                ))
                .await,
            Err(Error::BackupBackendUnavailable)
        ));
    }

    #[tokio::test]
    async fn backup_root_is_explicit_and_fail_closed() {
        let database_root = tempfile::tempdir().expect("database root");
        let (_, store) = create(database_root.path(), None).await;
        assert!(matches!(
            store
                .capture_backup(&plan(
                    96,
                    BackupSecretPolicy::ExcludeProtectedStorage,
                    9_600
                ))
                .await,
            Err(Error::BackupRootRequired)
        ));

        let relative = PathBuf::from("backups");
        assert!(matches!(
            OpenOptions::new(
                Paths::from_directory(database_root.path()).expect("owned paths"),
                OpenMode::ReadOnly
            )
            .with_backup_root(relative),
            Err(Error::InvalidBackupRoot(_))
        ));
        let file = database_root.path().join("backup-file");
        fs::write(&file, b"not a directory").expect("backup file");
        assert!(matches!(
            validate_backup_root(&file),
            Err(Error::InvalidBackupRoot(_))
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let directory = database_root.path().join("real-backups");
            let alias = database_root.path().join("backup-alias");
            fs::create_dir(&directory).expect("real backup root");
            symlink(&directory, &alias).expect("backup root symlink");
            assert!(matches!(
                validate_backup_root(&alias),
                Err(Error::InvalidBackupRoot(_))
            ));
        }
    }
}

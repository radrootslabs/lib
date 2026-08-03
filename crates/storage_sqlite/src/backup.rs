//! Consistent SQLite backup capture and bundle layout.

use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
};

use radroots_storage::backup::{
    BackupFormatVersion, BackupManifest, BackupMember, BackupMemberKind, BackupPlan,
    BackupSecretPolicy, MemberDigest, MemberVerification, RestoreMemberStatus, RestorePlan,
};
use radroots_storage::status::EventStoreMode;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

use crate::{Error, OpenMode, SqliteStorage, integrity, migration};

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

    /// Verifies the complete staged bundle without mutating or finalizing it.
    pub async fn verify_backup(
        &self,
        plan: &BackupPlan,
        manifest: &BackupManifest,
    ) -> Result<(), Error> {
        self.lifecycle
            .require_open()
            .map_err(|_| Error::BackupBackendUnavailable)?;
        let backup_root = self
            .backup_root
            .as_deref()
            .ok_or(Error::BackupRootRequired)?;
        validate_backup_root(backup_root)?;
        let layout = BackupLayout::new(backup_root, plan);
        verify_bundle(&layout.staging, plan, manifest).await
    }

    /// Verifies and atomically renames a complete staging bundle. A retry
    /// against an already finalized valid bundle succeeds idempotently.
    pub async fn finalize_backup(
        &self,
        plan: &BackupPlan,
        manifest: &BackupManifest,
    ) -> Result<PathBuf, Error> {
        self.lifecycle
            .require_open()
            .map_err(|_| Error::BackupBackendUnavailable)?;
        let backup_root = self
            .backup_root
            .as_deref()
            .ok_or(Error::BackupRootRequired)?;
        validate_backup_root(backup_root)?;
        let layout = BackupLayout::new(backup_root, plan);
        let staging = entry_kind(&layout.staging)?;
        let finalized = entry_kind(&layout.finalized)?;
        match (staging, finalized) {
            (EntryKind::Missing, EntryKind::Directory) => {
                verify_bundle(&layout.finalized, plan, manifest).await?;
                Ok(layout.finalized)
            }
            (EntryKind::Directory, EntryKind::Missing) => {
                verify_bundle(&layout.staging, plan, manifest).await?;
                fs::rename(&layout.staging, &layout.finalized).map_err(|source| {
                    Error::BackupFilesystem {
                        operation: "atomically finalize backup bundle",
                        source,
                    }
                })?;
                sync_directory(backup_root, "sync finalized backup root")?;
                Ok(layout.finalized)
            }
            (EntryKind::Missing, EntryKind::Missing) => {
                Err(Error::BackupBundleMissing(layout.staging))
            }
            (_, EntryKind::Directory) => Err(Error::BackupBundleAlreadyExists(layout.finalized)),
            (EntryKind::Other, _) => Err(Error::BackupUnexpectedEntry(layout.staging)),
            (_, EntryKind::Other) => Err(Error::BackupUnexpectedEntry(layout.finalized)),
        }
    }

    /// Copies a verified finalized bundle into create-new files adjacent to
    /// the live databases and verifies every staged copy before replacement.
    pub async fn stage_restore(
        &self,
        plan: &RestorePlan,
    ) -> Result<Vec<RestoreMemberStatus>, Error> {
        self.lifecycle
            .require_open()
            .map_err(|_| Error::BackupBackendUnavailable)?;
        if self.mode != EventStoreMode::ReadWrite {
            return Err(Error::RestoreRequiresWritableStorage);
        }
        let backup_root = self
            .backup_root
            .as_deref()
            .ok_or(Error::BackupRootRequired)?;
        let live_paths = self
            .paths
            .as_deref()
            .ok_or(Error::BackupBackendUnavailable)?;
        validate_backup_root(backup_root)?;
        let manifest = plan.manifest();
        let backup_plan = BackupPlan::new(
            manifest.backup_id(),
            manifest.format_version(),
            manifest.secret_policy(),
            manifest.created_at_unix_ms(),
        )
        .map_err(|_| Error::RestoreStagingFailed { member: "manifest" })?;
        let bundle = BackupLayout::new(backup_root, &backup_plan).finalized;
        verify_bundle(&bundle, &backup_plan, manifest).await?;
        let staging = RestoreStaging::new(live_paths, manifest)?;
        staging.require_absent(manifest.secret_policy())?;

        copy_staged_member(
            &bundle.join(RUNTIME_MEMBER),
            &staging.runtime,
            manifest
                .member(RUNTIME_MEMBER)
                .ok_or(Error::RestoreStagingFailed {
                    member: RUNTIME_MEMBER,
                })?,
            BackupMemberKind::Runtime,
            RUNTIME_MEMBER,
            true,
        )
        .await?;
        sync_parent(&staging.runtime, "sync runtime restore parent")?;
        let mut statuses = vec![
            RestoreMemberStatus::new(RUNTIME_MEMBER, MemberVerification::Verified).map_err(
                |_| Error::RestoreStagingFailed {
                    member: RUNTIME_MEMBER,
                },
            )?,
        ];

        if manifest.secret_policy() == BackupSecretPolicy::IncludeProtectedStorage {
            copy_staged_member(
                &bundle.join(PRIVATE_MEMBER),
                &staging.private,
                manifest
                    .member(PRIVATE_MEMBER)
                    .ok_or(Error::RestoreStagingFailed {
                        member: PRIVATE_MEMBER,
                    })?,
                BackupMemberKind::Protected,
                PRIVATE_MEMBER,
                false,
            )
            .await?;
            sync_parent(&staging.private, "sync private restore parent")?;
            statuses.push(
                RestoreMemberStatus::new(PRIVATE_MEMBER, MemberVerification::Verified).map_err(
                    |_| Error::RestoreStagingFailed {
                        member: PRIVATE_MEMBER,
                    },
                )?,
            );
        }
        Ok(statuses)
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

#[derive(Clone, Copy, Eq, PartialEq)]
enum EntryKind {
    Missing,
    Directory,
    Other,
}

fn entry_kind(path: &Path) -> Result<EntryKind, Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            Ok(EntryKind::Directory)
        }
        Ok(_) => Ok(EntryKind::Other),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(EntryKind::Missing),
        Err(source) => Err(Error::BackupFilesystem {
            operation: "inspect backup bundle",
            source,
        }),
    }
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
    encode_id(plan.backup_id().as_bytes())
}

fn encode_id(bytes: &[u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(32);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

struct RestoreStaging {
    runtime: PathBuf,
    private: PathBuf,
}

impl RestoreStaging {
    fn new(paths: &crate::Paths, manifest: &BackupManifest) -> Result<Self, Error> {
        let id = encode_id(manifest.backup_id().as_bytes());
        Ok(Self {
            runtime: staged_restore_path(paths.runtime(), &id)?,
            private: staged_restore_path(paths.private(), &id)?,
        })
    }

    fn require_absent(&self, policy: BackupSecretPolicy) -> Result<(), Error> {
        let paths = if policy == BackupSecretPolicy::IncludeProtectedStorage {
            vec![&self.runtime, &self.private]
        } else {
            vec![&self.runtime]
        };
        for path in paths {
            if entry_kind(path)? != EntryKind::Missing {
                return Err(Error::RestoreStagingAlreadyExists(path.clone()));
            }
        }
        Ok(())
    }
}

fn staged_restore_path(live: &Path, id: &str) -> Result<PathBuf, Error> {
    let name = live
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| Error::InvalidPath(live.to_path_buf()))?;
    Ok(live.with_file_name(format!(".{name}.restore-{id}.staging")))
}

async fn copy_staged_member(
    source: &Path,
    destination: &Path,
    expected: &BackupMember,
    kind: BackupMemberKind,
    member_name: &'static str,
    runtime: bool,
) -> Result<(), Error> {
    let mut source_file = File::open(source).map_err(|_| Error::RestoreStagingFailed {
        member: member_name,
    })?;
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut destination_file = options.open(destination).map_err(|source| {
        if source.kind() == std::io::ErrorKind::AlreadyExists {
            Error::RestoreStagingAlreadyExists(destination.to_path_buf())
        } else {
            Error::BackupFilesystem {
                operation: "create restore staging member",
                source,
            }
        }
    })?;
    std::io::copy(&mut source_file, &mut destination_file).map_err(|_| {
        Error::RestoreStagingFailed {
            member: member_name,
        }
    })?;
    destination_file
        .sync_all()
        .map_err(|source| Error::BackupFilesystem {
            operation: "sync restore staging member",
            source,
        })?;
    drop(destination_file);
    verify_member(destination, expected, kind, member_name, runtime).await
}

fn sync_parent(path: &Path, operation: &'static str) -> Result<(), Error> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::InvalidPath(path.to_path_buf()))?;
    sync_directory(parent, operation)
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

async fn verify_bundle(
    bundle: &Path,
    plan: &BackupPlan,
    manifest: &BackupManifest,
) -> Result<(), Error> {
    if entry_kind(bundle)? != EntryKind::Directory {
        return Err(Error::BackupBundleMissing(bundle.to_path_buf()));
    }
    validate_manifest(plan, manifest)?;
    let expected_root = if plan.secret_policy() == BackupSecretPolicy::IncludeProtectedStorage {
        BTreeSet::from(["private", "runtime"])
    } else {
        BTreeSet::from(["runtime"])
    };
    validate_entries(bundle, &expected_root)?;
    let runtime_directory = bundle.join("runtime");
    validate_entries(&runtime_directory, &BTreeSet::from([RUNTIME_DATABASE]))?;
    verify_member(
        &runtime_directory.join(RUNTIME_DATABASE),
        manifest
            .member(RUNTIME_MEMBER)
            .ok_or(Error::BackupVerificationFailed {
                member: RUNTIME_MEMBER,
            })?,
        BackupMemberKind::Runtime,
        RUNTIME_MEMBER,
        true,
    )
    .await?;
    if plan.secret_policy() == BackupSecretPolicy::IncludeProtectedStorage {
        let private_directory = bundle.join("private");
        validate_entries(&private_directory, &BTreeSet::from([PRIVATE_DATABASE]))?;
        verify_member(
            &private_directory.join(PRIVATE_DATABASE),
            manifest
                .member(PRIVATE_MEMBER)
                .ok_or(Error::BackupVerificationFailed {
                    member: PRIVATE_MEMBER,
                })?,
            BackupMemberKind::Protected,
            PRIVATE_MEMBER,
            false,
        )
        .await?;
    }
    Ok(())
}

fn validate_manifest(plan: &BackupPlan, manifest: &BackupManifest) -> Result<(), Error> {
    if plan.format_version() != BackupFormatVersion::V1
        || manifest.format_version() != plan.format_version()
        || manifest.backup_id() != plan.backup_id()
        || manifest.secret_policy() != plan.secret_policy()
        || manifest.created_at_unix_ms() != plan.requested_at_unix_ms()
    {
        return Err(Error::BackupVerificationFailed { member: "manifest" });
    }
    let expected = if plan.secret_policy() == BackupSecretPolicy::IncludeProtectedStorage {
        BTreeSet::from([PRIVATE_MEMBER, RUNTIME_MEMBER])
    } else {
        BTreeSet::from([RUNTIME_MEMBER])
    };
    let actual = manifest
        .members()
        .iter()
        .map(BackupMember::relative_path)
        .collect::<BTreeSet<_>>();
    if actual == expected {
        Ok(())
    } else {
        Err(Error::BackupVerificationFailed { member: "manifest" })
    }
}

fn validate_entries(directory: &Path, expected: &BTreeSet<&str>) -> Result<(), Error> {
    let mut actual = BTreeSet::new();
    let entries = fs::read_dir(directory).map_err(|source| Error::BackupFilesystem {
        operation: "read backup bundle directory",
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::BackupFilesystem {
            operation: "read backup bundle entry",
            source,
        })?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| Error::BackupUnexpectedEntry(entry.path()))?;
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|source| Error::BackupFilesystem {
                operation: "inspect backup bundle entry",
                source,
            })?;
        if metadata.file_type().is_symlink() || !expected.contains(name.as_str()) {
            return Err(Error::BackupUnexpectedEntry(entry.path()));
        }
        actual.insert(name);
    }
    if actual.iter().map(String::as_str).collect::<BTreeSet<_>>() == *expected {
        Ok(())
    } else {
        Err(Error::BackupVerificationFailed {
            member: "inventory",
        })
    }
}

async fn verify_member(
    path: &Path,
    expected: &BackupMember,
    expected_kind: BackupMemberKind,
    member_name: &'static str,
    runtime: bool,
) -> Result<(), Error> {
    if expected.kind() != expected_kind || !entry_kind_file(path)? {
        return Err(Error::BackupVerificationFailed {
            member: member_name,
        });
    }
    let (byte_length, sha256) = fingerprint(path)?;
    if byte_length != expected.byte_length() || sha256 != expected.sha256() {
        return Err(Error::BackupVerificationFailed {
            member: member_name,
        });
    }
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .read_only(true)
            .foreign_keys(true),
    )
    .await
    .map_err(|_| Error::BackupVerificationFailed {
        member: member_name,
    })?;
    let schema = if runtime {
        migration::migrate_runtime(&mut connection, OpenMode::ReadOnly).await
    } else {
        migration::migrate_private(&mut connection, OpenMode::ReadOnly).await
    };
    if schema.is_err()
        || integrity::check_connection(&mut connection).await != integrity::MemberOutcome::Verified
    {
        return Err(Error::BackupVerificationFailed {
            member: member_name,
        });
    }
    connection
        .close()
        .await
        .map_err(|_| Error::BackupVerificationFailed {
            member: member_name,
        })
}

fn entry_kind_file(path: &Path) -> Result<bool, Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.is_file() && !metadata.file_type().is_symlink()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(Error::BackupFilesystem {
            operation: "inspect backup member",
            source,
        }),
    }
}

fn fingerprint(path: &Path) -> Result<(u64, MemberDigest), Error> {
    let mut file = File::open(path).map_err(|source| Error::BackupFilesystem {
        operation: "open backup member for verification",
        source,
    })?;
    let byte_length = file
        .metadata()
        .map_err(|source| Error::BackupFilesystem {
            operation: "inspect backup member for verification",
            source,
        })?
        .len();
    let mut sha256 = Sha256::new();
    let mut buffer = [0_u8; 16 * 1_024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| Error::BackupFilesystem {
                operation: "hash backup member for verification",
                source,
            })?;
        if read == 0 {
            break;
        }
        sha256.update(&buffer[..read]);
    }
    Ok((byte_length, MemberDigest::new(sha256.finalize().into())))
}

#[cfg(test)]
mod tests {
    use radroots_storage::{
        backup::{
            BackupFormatVersion, BackupId, BackupMemberKind, BackupPlan, BackupSecretPolicy,
            MemberVerification, RestorePlan,
        },
        event::SourceGeneration,
    };
    use serde::Deserialize;
    use sqlx::{Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};

    use crate::{OpenMode, OpenOptions, Paths};

    use super::*;

    const POLICY: &str = include_str!("../../../contracts/storage/backup_capture_policy_v1.toml");
    const FINALIZE_POLICY: &str =
        include_str!("../../../contracts/storage/backup_finalize_policy_v1.toml");
    const RESTORE_POLICY: &str =
        include_str!("../../../contracts/storage/restore_staging_policy_v1.toml");

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

    #[derive(Deserialize)]
    struct FinalizePolicy {
        schema_version: u32,
        verification: Vec<String>,
        finalization: String,
        root_sync_after_rename: bool,
        finalized_retry: String,
        missing_bundle: String,
        staging_and_final_present: String,
        unexpected_entry: String,
        mutation_before_complete_verification: bool,
    }

    #[derive(Deserialize)]
    struct RestorePolicy {
        schema_version: u32,
        source: String,
        authority: String,
        runtime_staging: String,
        protected_staging: String,
        creation: String,
        verification: Vec<String>,
        live_mutation: bool,
        existing_staging: String,
        protected_member: String,
        filesystem_sync: Vec<String>,
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

    #[test]
    fn implementation_matches_the_governed_backup_finalize_policy() {
        let policy = toml::from_str::<FinalizePolicy>(FINALIZE_POLICY).expect("finalize policy");
        assert_eq!(policy.schema_version, 1);
        assert_eq!(
            policy.verification,
            [
                "exact_plan_manifest",
                "exact_inventory",
                "no_symlinks",
                "exact_length",
                "sha256",
                "current_schema_catalog",
                "sqlite_integrity_check",
                "foreign_key_check"
            ]
        );
        assert_eq!(policy.finalization, "same_root_atomic_directory_rename");
        assert!(policy.root_sync_after_rename);
        assert_eq!(policy.finalized_retry, "verify_and_succeed");
        assert_eq!(policy.missing_bundle, "reject");
        assert_eq!(policy.staging_and_final_present, "reject");
        assert_eq!(policy.unexpected_entry, "reject");
        assert!(!policy.mutation_before_complete_verification);
    }

    #[test]
    fn implementation_matches_the_governed_restore_staging_policy() {
        let policy = toml::from_str::<RestorePolicy>(RESTORE_POLICY).expect("restore policy");
        assert_eq!(policy.schema_version, 1);
        assert_eq!(policy.source, "verified_finalized_backup_bundle");
        assert_eq!(policy.authority, "writable_storage_only");
        assert_eq!(
            policy.runtime_staging,
            ".runtime.sqlite.restore-{backup_id_hex}.staging"
        );
        assert_eq!(
            policy.protected_staging,
            ".private.sqlite.restore-{backup_id_hex}.staging"
        );
        assert_eq!(policy.creation, "create_new_mode_0600");
        assert_eq!(
            policy.verification,
            [
                "exact_length",
                "sha256",
                "current_schema_catalog",
                "sqlite_integrity_check",
                "foreign_key_check"
            ]
        );
        assert!(!policy.live_mutation);
        assert_eq!(policy.existing_staging, "reject");
        assert_eq!(policy.protected_member, "manifest_policy_controlled");
        assert_eq!(
            policy.filesystem_sync,
            ["staged_member_file", "destination_parent"]
        );
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

    #[tokio::test]
    async fn complete_bundle_verifies_finalizes_atomically_and_retries_idempotently() {
        let database_root = tempfile::tempdir().expect("database root");
        let backup_parent = tempfile::tempdir().expect("backup parent");
        let backup_root = backup_parent.path().join("backups");
        fs::create_dir(&backup_root).expect("backup root");
        let (_, store) = create(database_root.path(), Some(&backup_root)).await;
        let plan = plan(97, BackupSecretPolicy::IncludeProtectedStorage, 9_700);
        let manifest = store.capture_backup(&plan).await.expect("capture backup");
        let layout = BackupLayout::new(&backup_root, &plan);

        store
            .verify_backup(&plan, &manifest)
            .await
            .expect("verify staging bundle");
        let finalized = store
            .finalize_backup(&plan, &manifest)
            .await
            .expect("finalize backup");
        assert_eq!(finalized, layout.finalized);
        assert!(!layout.staging.exists());
        assert!(layout.finalized.is_dir());
        assert_eq!(
            store
                .finalize_backup(&plan, &manifest)
                .await
                .expect("idempotent finalization"),
            layout.finalized
        );
    }

    #[tokio::test]
    async fn verification_rejects_tampering_unexpected_entries_and_missing_bundles() {
        let database_root = tempfile::tempdir().expect("database root");
        let backup_parent = tempfile::tempdir().expect("backup parent");
        let backup_root = backup_parent.path().join("backups");
        fs::create_dir(&backup_root).expect("backup root");
        let (_, store) = create(database_root.path(), Some(&backup_root)).await;

        let tampered_plan = plan(98, BackupSecretPolicy::ExcludeProtectedStorage, 9_800);
        let tampered_manifest = store
            .capture_backup(&tampered_plan)
            .await
            .expect("capture tamper target");
        let tampered_layout = BackupLayout::new(&backup_root, &tampered_plan);
        use std::io::Write;
        fs::OpenOptions::new()
            .append(true)
            .open(&tampered_layout.runtime_file)
            .expect("open tamper target")
            .write_all(b"tamper")
            .expect("tamper member");
        assert!(matches!(
            store
                .verify_backup(&tampered_plan, &tampered_manifest)
                .await,
            Err(Error::BackupVerificationFailed {
                member: RUNTIME_MEMBER
            })
        ));
        assert!(!tampered_layout.finalized.exists());

        let unexpected_plan = plan(99, BackupSecretPolicy::ExcludeProtectedStorage, 9_900);
        let unexpected_manifest = store
            .capture_backup(&unexpected_plan)
            .await
            .expect("capture unexpected target");
        let unexpected_layout = BackupLayout::new(&backup_root, &unexpected_plan);
        fs::write(unexpected_layout.staging.join("unexpected"), b"data").expect("unexpected entry");
        assert!(matches!(
            store
                .verify_backup(&unexpected_plan, &unexpected_manifest)
                .await,
            Err(Error::BackupUnexpectedEntry(_))
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            fs::remove_file(unexpected_layout.staging.join("unexpected"))
                .expect("remove unexpected entry");
            fs::remove_file(&unexpected_layout.runtime_file).expect("remove captured member");
            symlink(
                database_root.path().join(RUNTIME_DATABASE),
                &unexpected_layout.runtime_file,
            )
            .expect("symlink captured member");
            assert!(matches!(
                store
                    .verify_backup(&unexpected_plan, &unexpected_manifest)
                    .await,
                Err(Error::BackupUnexpectedEntry(_))
            ));
        }

        let missing_plan = plan(100, BackupSecretPolicy::ExcludeProtectedStorage, 10_000);
        let missing_manifest = BackupManifest::new(
            missing_plan.format_version(),
            missing_plan.backup_id(),
            missing_plan.requested_at_unix_ms(),
            missing_plan.secret_policy(),
            vec![
                BackupMember::new(
                    RUNTIME_MEMBER,
                    BackupMemberKind::Runtime,
                    1,
                    MemberDigest::new([1; 32]),
                )
                .expect("member"),
            ],
        )
        .expect("manifest");
        assert!(matches!(
            store
                .finalize_backup(&missing_plan, &missing_manifest)
                .await,
            Err(Error::BackupBundleMissing(_))
        ));
    }

    #[tokio::test]
    async fn restore_staging_is_verified_isolated_and_leaves_live_state_untouched() {
        let database_root = tempfile::tempdir().expect("database root");
        let backup_parent = tempfile::tempdir().expect("backup parent");
        let backup_root = backup_parent.path().join("backups");
        fs::create_dir(&backup_root).expect("backup root");
        let (paths, store) = create(database_root.path(), Some(&backup_root)).await;
        sqlx::query(
            "UPDATE radroots_runtime_source_generations SET sequence_head = 51 WHERE state = 'active'",
        )
        .execute(&store.pool)
        .await
        .expect("initial live state");
        let backup_plan = plan(101, BackupSecretPolicy::IncludeProtectedStorage, 10_100);
        let manifest = store
            .capture_backup(&backup_plan)
            .await
            .expect("capture restore source");
        store
            .finalize_backup(&backup_plan, &manifest)
            .await
            .expect("finalize restore source");
        sqlx::query(
            "UPDATE radroots_runtime_source_generations SET sequence_head = 52 WHERE state = 'active'",
        )
            .execute(&store.pool)
            .await
            .expect("advance live state");

        let restore = RestorePlan::new(
            manifest.clone(),
            BackupSecretPolicy::IncludeProtectedStorage,
            10_200,
        )
        .expect("restore plan");
        let statuses = store.stage_restore(&restore).await.expect("stage restore");
        assert_eq!(statuses.len(), 2);
        assert!(
            statuses
                .iter()
                .all(|status| status.verification() == MemberVerification::Verified)
        );
        let staging = RestoreStaging::new(&paths, &manifest).expect("restore staging paths");
        assert_eq!(
            scalar(
                paths.runtime(),
                "SELECT sequence_head FROM radroots_runtime_source_generations WHERE state = 'active'"
            )
            .await,
            52
        );
        assert_eq!(
            scalar(
                &staging.runtime,
                "SELECT sequence_head FROM radroots_runtime_source_generations WHERE state = 'active'"
            )
            .await,
            51
        );
        assert!(staging.private.is_file());
        assert!(matches!(
            store.stage_restore(&restore).await,
            Err(Error::RestoreStagingAlreadyExists(_))
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&staging.runtime)
                    .expect("staged runtime metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        store.close().await.expect("close writer");
        let reader = SqliteStorage::open(
            OpenOptions::new(paths, OpenMode::ReadOnly)
                .with_backup_root(&backup_root)
                .expect("backup root"),
        )
        .await
        .expect("read-only store");
        assert!(matches!(
            reader.stage_restore(&restore).await,
            Err(Error::RestoreRequiresWritableStorage)
        ));
    }
}

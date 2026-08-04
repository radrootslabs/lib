//! Consistent SQLite backup capture and bundle layout.

use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use radroots_storage::backup::{
    BackupFormatVersion, BackupId, BackupManifest, BackupMember, BackupMemberKind, BackupOperation,
    BackupPlan, BackupSecretPolicy, BackupTransition, MemberDigest, MemberVerification,
    ReliabilityRevision, RestoreMemberStatus, RestoreOperation, RestorePlan, RestoreTransition,
    StorageReliability,
};
use radroots_storage::status::EventStoreMode;
use radroots_storage::{Error as StorageError, outbox::BoxFuture};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

use crate::{Error, OpenMode, SqliteStorage, integrity, migration};

const RUNTIME_DATABASE: &str = "runtime.sqlite";
const PRIVATE_DATABASE: &str = "private.sqlite";
const RUNTIME_MEMBER: &str = "runtime/runtime.sqlite";
const PRIVATE_MEMBER: &str = "private/private.sqlite";
const RESTORE_MARKER_MAGIC: &[u8; 8] = b"RDRSTR01";
const RESTORE_MARKER_BYTES: usize = 105;

#[derive(Default)]
pub(crate) struct ReliabilityState {
    backups: Vec<BackupOperation>,
    restores: Vec<RestoreOperation>,
}

impl SqliteStorage {
    fn reliability_state(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, ReliabilityState>, StorageError> {
        self.lifecycle.require_open()?;
        self.reliability
            .lock()
            .map_err(|_| StorageError::BackendUnavailable)
    }
}

impl StorageReliability for SqliteStorage {
    fn begin_backup(
        &self,
        plan: BackupPlan,
    ) -> BoxFuture<'_, Result<BackupOperation, StorageError>> {
        Box::pin(async move {
            let mut state = self.reliability_state()?;
            if let Some(existing) = state
                .backups
                .iter()
                .find(|operation| operation.plan().backup_id() == plan.backup_id())
            {
                return if existing.plan() == &plan {
                    Ok(existing.clone())
                } else {
                    Err(StorageError::ReliabilityRevisionConflict)
                };
            }
            let operation = BackupOperation::planned(plan);
            state.backups.push(operation.clone());
            Ok(operation)
        })
    }

    fn transition_backup(
        &self,
        backup_id: BackupId,
        expected_revision: ReliabilityRevision,
        transition: BackupTransition,
        at_unix_ms: u64,
    ) -> BoxFuture<'_, Result<BackupOperation, StorageError>> {
        Box::pin(async move {
            let mut state = self.reliability_state()?;
            let operation = state
                .backups
                .iter_mut()
                .find(|operation| operation.plan().backup_id() == backup_id)
                .ok_or(StorageError::CorruptReliabilityOperation)?;
            let next = operation.transition(expected_revision, transition, at_unix_ms)?;
            *operation = next.clone();
            Ok(next)
        })
    }

    fn begin_restore(
        &self,
        plan: RestorePlan,
    ) -> BoxFuture<'_, Result<RestoreOperation, StorageError>> {
        Box::pin(async move {
            let mut state = self.reliability_state()?;
            let backup_id = plan.manifest().backup_id();
            if let Some(existing) = state
                .restores
                .iter()
                .find(|operation| operation.plan().manifest().backup_id() == backup_id)
            {
                return if existing.plan() == &plan {
                    Ok(existing.clone())
                } else {
                    Err(StorageError::ReliabilityRevisionConflict)
                };
            }
            let operation = RestoreOperation::staging(plan);
            state.restores.push(operation.clone());
            Ok(operation)
        })
    }

    fn transition_restore(
        &self,
        backup_id: BackupId,
        expected_revision: ReliabilityRevision,
        transition: RestoreTransition,
        at_unix_ms: u64,
    ) -> BoxFuture<'_, Result<RestoreOperation, StorageError>> {
        Box::pin(async move {
            let mut state = self.reliability_state()?;
            let operation = state
                .restores
                .iter_mut()
                .find(|operation| operation.plan().manifest().backup_id() == backup_id)
                .ok_or(StorageError::CorruptReliabilityOperation)?;
            let next = operation.transition(expected_revision, transition, at_unix_ms)?;
            *operation = next.clone();
            Ok(next)
        })
    }

    fn integrity(
        &self,
    ) -> BoxFuture<'_, Result<radroots_storage::status::IntegrityStatus, StorageError>> {
        Box::pin(async move { SqliteStorage::integrity(self).await })
    }

    fn status(&self) -> BoxFuture<'_, Result<radroots_storage::StorageStatus, StorageError>> {
        Box::pin(async move { SqliteStorage::storage_status(self).await })
    }

    fn close(&self) -> BoxFuture<'_, Result<radroots_storage::StorageStatus, StorageError>> {
        Box::pin(async move { SqliteStorage::close(self).await })
    }
}

impl SqliteStorage {
    /// Captures consistent SQLite snapshots into a new deterministic staging
    /// bundle under the configured host-owned backup root.
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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

    /// Quiesces this writable backend, records a durable interruption marker,
    /// and installs every completely verified staged member. The backend is
    /// closed after the attempt and must be reopened to observe restored state.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn finalize_restore(&self, plan: &RestorePlan) -> Result<(), Error> {
        self.lifecycle
            .require_open()
            .map_err(|_| Error::BackupBackendUnavailable)?;
        if self.mode != EventStoreMode::ReadWrite {
            return Err(Error::RestoreRequiresWritableStorage);
        }
        let paths = self
            .paths
            .as_deref()
            .ok_or(Error::BackupBackendUnavailable)?;
        let marker = RestoreMarker::from_manifest(plan.manifest())?;
        let layout = RestoreLayout::new(paths, marker.backup_id())?;
        verify_staged_restore(&layout, &marker).await?;
        layout.require_previous_absent(marker.secret_policy())?;

        self.lifecycle
            .begin_restore_close()
            .map_err(|_| Error::BackupBackendUnavailable)?;
        self.pool.close().await;
        self.private_pool.close().await;
        let installation = async {
            verify_staged_restore(&layout, &marker).await?;
            write_restore_marker(&layout.marker, &marker)?;
            recover_interrupted_restore(paths, OpenMode::ReadWriteExisting).await
        }
        .await;
        let close = self.lifecycle.finish_restore_close();
        installation?;
        close.map_err(|_| Error::BackupBackendUnavailable)
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
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

#[cfg_attr(coverage_nightly, coverage(off))]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

struct RestoreLayout {
    runtime_live: PathBuf,
    private_live: PathBuf,
    runtime_staging: PathBuf,
    private_staging: PathBuf,
    runtime_previous: PathBuf,
    private_previous: PathBuf,
    marker: PathBuf,
}

impl RestoreLayout {
    fn new(paths: &crate::Paths, backup_id: BackupId) -> Result<Self, Error> {
        let id = encode_id(backup_id.as_bytes());
        let runtime_parent = paths
            .runtime()
            .parent()
            .ok_or_else(|| Error::InvalidPath(paths.runtime().to_path_buf()))?;
        Ok(Self {
            runtime_live: paths.runtime().to_path_buf(),
            private_live: paths.private().to_path_buf(),
            runtime_staging: restore_sidecar_path(paths.runtime(), &id, "staging")?,
            private_staging: restore_sidecar_path(paths.private(), &id, "staging")?,
            runtime_previous: restore_sidecar_path(paths.runtime(), &id, "previous")?,
            private_previous: restore_sidecar_path(paths.private(), &id, "previous")?,
            marker: runtime_parent.join(format!(".radroots-storage-restore-{id}.marker")),
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn require_previous_absent(&self, policy: BackupSecretPolicy) -> Result<(), Error> {
        let paths = if policy == BackupSecretPolicy::IncludeProtectedStorage {
            vec![&self.runtime_previous, &self.private_previous]
        } else {
            vec![&self.runtime_previous]
        };
        for path in paths {
            if restore_entry_kind(path)? != RestoreEntryKind::Missing {
                return Err(Error::RestoreRecoveryConflict(path.clone()));
            }
        }
        if restore_entry_kind(&self.marker)? != RestoreEntryKind::Missing {
            return Err(Error::RestoreRecoveryConflict(self.marker.clone()));
        }
        Ok(())
    }
}

fn staged_restore_path(live: &Path, id: &str) -> Result<PathBuf, Error> {
    restore_sidecar_path(live, id, "staging")
}

fn restore_sidecar_path(live: &Path, id: &str, role: &str) -> Result<PathBuf, Error> {
    let name = live
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| Error::InvalidPath(live.to_path_buf()))?;
    Ok(live.with_file_name(format!(".{name}.restore-{id}.{role}")))
}

#[derive(Clone, Copy)]
struct RestoreMemberExpectation {
    byte_length: u64,
    sha256: MemberDigest,
}

impl RestoreMemberExpectation {
    fn from_member(member: &BackupMember, expected_kind: BackupMemberKind) -> Result<Self, Error> {
        if member.kind() != expected_kind {
            return Err(Error::RestoreReplacementFailed { member: "manifest" });
        }
        Ok(Self {
            byte_length: member.byte_length(),
            sha256: member.sha256(),
        })
    }

    fn member(
        self,
        relative_path: &'static str,
        kind: BackupMemberKind,
    ) -> Result<BackupMember, Error> {
        BackupMember::new(relative_path, kind, self.byte_length, self.sha256).map_err(|_| {
            Error::RestoreReplacementFailed {
                member: relative_path,
            }
        })
    }
}

struct RestoreMarker {
    backup_id: BackupId,
    secret_policy: BackupSecretPolicy,
    runtime: RestoreMemberExpectation,
    private: Option<RestoreMemberExpectation>,
}

impl RestoreMarker {
    fn from_manifest(manifest: &BackupManifest) -> Result<Self, Error> {
        if manifest.format_version() != BackupFormatVersion::V1 {
            return Err(Error::UnsupportedBackupVersion);
        }
        let expected = if manifest.secret_policy() == BackupSecretPolicy::IncludeProtectedStorage {
            BTreeSet::from([PRIVATE_MEMBER, RUNTIME_MEMBER])
        } else {
            BTreeSet::from([RUNTIME_MEMBER])
        };
        let actual = manifest
            .members()
            .iter()
            .map(BackupMember::relative_path)
            .collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(Error::RestoreReplacementFailed { member: "manifest" });
        }
        let runtime = RestoreMemberExpectation::from_member(
            manifest
                .member(RUNTIME_MEMBER)
                .ok_or(Error::RestoreReplacementFailed { member: "manifest" })?,
            BackupMemberKind::Runtime,
        )?;
        let private = manifest
            .member(PRIVATE_MEMBER)
            .map(|member| {
                RestoreMemberExpectation::from_member(member, BackupMemberKind::Protected)
            })
            .transpose()?;
        Ok(Self {
            backup_id: manifest.backup_id(),
            secret_policy: manifest.secret_policy(),
            runtime,
            private,
        })
    }

    const fn backup_id(&self) -> BackupId {
        self.backup_id
    }

    const fn secret_policy(&self) -> BackupSecretPolicy {
        self.secret_policy
    }

    fn encode(&self) -> [u8; RESTORE_MARKER_BYTES] {
        let mut encoded = [0_u8; RESTORE_MARKER_BYTES];
        encoded[..8].copy_from_slice(RESTORE_MARKER_MAGIC);
        encoded[8] = u8::from(self.private.is_some());
        encoded[9..25].copy_from_slice(self.backup_id.as_bytes());
        encoded[25..33].copy_from_slice(&self.runtime.byte_length.to_be_bytes());
        encoded[33..65].copy_from_slice(self.runtime.sha256.as_bytes());
        if let Some(private) = self.private {
            encoded[65..73].copy_from_slice(&private.byte_length.to_be_bytes());
            encoded[73..105].copy_from_slice(private.sha256.as_bytes());
        }
        encoded
    }

    fn decode(path: &Path, encoded: &[u8]) -> Result<Self, Error> {
        if encoded.len() != RESTORE_MARKER_BYTES || &encoded[..8] != RESTORE_MARKER_MAGIC {
            return Err(Error::RestoreMarkerCorrupt(path.to_path_buf()));
        }
        let secret_policy = match encoded[8] {
            0 => BackupSecretPolicy::ExcludeProtectedStorage,
            1 => BackupSecretPolicy::IncludeProtectedStorage,
            _ => return Err(Error::RestoreMarkerCorrupt(path.to_path_buf())),
        };
        let backup_id = BackupId::new(
            encoded[9..25]
                .try_into()
                .map_err(|_| Error::RestoreMarkerCorrupt(path.to_path_buf()))?,
        )
        .map_err(|_| Error::RestoreMarkerCorrupt(path.to_path_buf()))?;
        let runtime = RestoreMemberExpectation {
            byte_length: u64::from_be_bytes(
                encoded[25..33]
                    .try_into()
                    .map_err(|_| Error::RestoreMarkerCorrupt(path.to_path_buf()))?,
            ),
            sha256: MemberDigest::new(
                encoded[33..65]
                    .try_into()
                    .map_err(|_| Error::RestoreMarkerCorrupt(path.to_path_buf()))?,
            ),
        };
        if runtime.byte_length == 0 {
            return Err(Error::RestoreMarkerCorrupt(path.to_path_buf()));
        }
        let private_length = u64::from_be_bytes(
            encoded[65..73]
                .try_into()
                .map_err(|_| Error::RestoreMarkerCorrupt(path.to_path_buf()))?,
        );
        let private_digest: [u8; 32] = encoded[73..105]
            .try_into()
            .map_err(|_| Error::RestoreMarkerCorrupt(path.to_path_buf()))?;
        let private = match secret_policy {
            BackupSecretPolicy::ExcludeProtectedStorage
                if private_length == 0 && private_digest == [0; 32] =>
            {
                None
            }
            BackupSecretPolicy::IncludeProtectedStorage if private_length > 0 => {
                Some(RestoreMemberExpectation {
                    byte_length: private_length,
                    sha256: MemberDigest::new(private_digest),
                })
            }
            _ => return Err(Error::RestoreMarkerCorrupt(path.to_path_buf())),
        };
        Ok(Self {
            backup_id,
            secret_policy,
            runtime,
            private,
        })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn write_restore_marker(path: &Path, marker: &RestoreMarker) -> Result<(), Error> {
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|source| Error::RestoreFilesystem {
            operation: "create restore interruption marker",
            source,
        })?;
    file.write_all(&marker.encode())
        .and_then(|()| file.sync_all())
        .map_err(|source| Error::RestoreFilesystem {
            operation: "persist restore interruption marker",
            source,
        })?;
    sync_parent(path, "sync restore marker parent")
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn read_restore_marker(path: &Path) -> Result<RestoreMarker, Error> {
    let metadata = fs::symlink_metadata(path).map_err(|source| Error::RestoreFilesystem {
        operation: "inspect restore interruption marker",
        source,
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() != RESTORE_MARKER_BYTES as u64
    {
        return Err(Error::RestoreMarkerCorrupt(path.to_path_buf()));
    }
    let encoded = fs::read(path).map_err(|source| Error::RestoreFilesystem {
        operation: "read restore interruption marker",
        source,
    })?;
    RestoreMarker::decode(path, &encoded)
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) async fn recover_interrupted_restore(
    paths: &crate::Paths,
    mode: OpenMode,
) -> Result<(), Error> {
    let Some(marker_path) = discover_restore_marker(paths)? else {
        return Ok(());
    };
    if !mode.is_writable() {
        return Err(Error::RestoreRequiresWritableStorage);
    }
    let marker = read_restore_marker(&marker_path)?;
    let layout = RestoreLayout::new(paths, marker.backup_id())?;
    if layout.marker != marker_path {
        return Err(Error::RestoreMarkerCorrupt(marker_path));
    }
    require_sqlite_sidecars_absent(paths)?;
    install_restore_member(
        &layout.runtime_live,
        &layout.runtime_staging,
        &layout.runtime_previous,
        marker.runtime,
        BackupMemberKind::Runtime,
        RUNTIME_MEMBER,
        true,
    )
    .await?;
    if let Some(private) = marker.private {
        install_restore_member(
            &layout.private_live,
            &layout.private_staging,
            &layout.private_previous,
            private,
            BackupMemberKind::Protected,
            PRIVATE_MEMBER,
            false,
        )
        .await?;
    }
    verify_installed_restore(&layout, &marker).await?;
    remove_restore_file(&layout.runtime_previous, "remove previous runtime database")?;
    if marker.private.is_some() {
        remove_restore_file(&layout.private_previous, "remove previous private database")?;
    }
    remove_restore_file(&layout.marker, "remove restore interruption marker")?;
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn discover_restore_marker(paths: &crate::Paths) -> Result<Option<PathBuf>, Error> {
    let parent = paths
        .runtime()
        .parent()
        .ok_or_else(|| Error::InvalidPath(paths.runtime().to_path_buf()))?;
    let parent_metadata = match fs::symlink_metadata(parent) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(Error::RestoreFilesystem {
                operation: "inspect restore marker parent",
                source,
            });
        }
    };
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        return Ok(None);
    }
    let mut marker = None;
    for entry in fs::read_dir(parent).map_err(|source| Error::RestoreFilesystem {
        operation: "scan restore interruption markers",
        source,
    })? {
        let entry = entry.map_err(|source| Error::RestoreFilesystem {
            operation: "read restore interruption marker entry",
            source,
        })?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !name.starts_with(".radroots-storage-restore-") || !name.ends_with(".marker") {
            continue;
        }
        if marker.replace(entry.path()).is_some() {
            return Err(Error::RestoreRecoveryConflict(parent.to_path_buf()));
        }
    }
    Ok(marker)
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn verify_staged_restore(
    layout: &RestoreLayout,
    marker: &RestoreMarker,
) -> Result<(), Error> {
    verify_restore_path(
        &layout.runtime_staging,
        marker.runtime,
        BackupMemberKind::Runtime,
        RUNTIME_MEMBER,
        true,
    )
    .await?;
    if let Some(private) = marker.private {
        verify_restore_path(
            &layout.private_staging,
            private,
            BackupMemberKind::Protected,
            PRIVATE_MEMBER,
            false,
        )
        .await?;
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn verify_installed_restore(
    layout: &RestoreLayout,
    marker: &RestoreMarker,
) -> Result<(), Error> {
    verify_restore_path(
        &layout.runtime_live,
        marker.runtime,
        BackupMemberKind::Runtime,
        RUNTIME_MEMBER,
        true,
    )
    .await?;
    if let Some(private) = marker.private {
        verify_restore_path(
            &layout.private_live,
            private,
            BackupMemberKind::Protected,
            PRIVATE_MEMBER,
            false,
        )
        .await?;
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn verify_restore_path(
    path: &Path,
    expected: RestoreMemberExpectation,
    kind: BackupMemberKind,
    member_name: &'static str,
    runtime: bool,
) -> Result<(), Error> {
    let member = expected.member(member_name, kind)?;
    verify_member(path, &member, kind, member_name, runtime)
        .await
        .map_err(|_| Error::RestoreReplacementFailed {
            member: member_name,
        })
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn install_restore_member(
    live: &Path,
    staging: &Path,
    previous: &Path,
    expected: RestoreMemberExpectation,
    kind: BackupMemberKind,
    member_name: &'static str,
    runtime: bool,
) -> Result<(), Error> {
    let live_kind = restore_entry_kind(live)?;
    let staging_kind = restore_entry_kind(staging)?;
    let previous_kind = restore_entry_kind(previous)?;
    if [live_kind, staging_kind, previous_kind]
        .into_iter()
        .any(|entry| entry == RestoreEntryKind::Other)
    {
        return Err(Error::RestoreRecoveryConflict(live.to_path_buf()));
    }

    if live_kind == RestoreEntryKind::File && restore_member_matches(live, expected)? {
        verify_restore_path(live, expected, kind, member_name, runtime).await?;
        if staging_kind == RestoreEntryKind::File {
            verify_restore_path(staging, expected, kind, member_name, runtime).await?;
            remove_restore_file(staging, "remove redundant restore staging member")?;
        }
        return Ok(());
    }
    if staging_kind != RestoreEntryKind::File {
        return Err(Error::RestoreReplacementFailed {
            member: member_name,
        });
    }
    verify_restore_path(staging, expected, kind, member_name, runtime).await?;
    match (live_kind, previous_kind) {
        (RestoreEntryKind::File, RestoreEntryKind::Missing) => {
            fs::rename(live, previous).map_err(|source| Error::RestoreFilesystem {
                operation: "rename live database to previous restore sidecar",
                source,
            })?;
            sync_parent(live, "sync previous database rename")?;
        }
        (RestoreEntryKind::Missing, RestoreEntryKind::File) => {}
        _ => return Err(Error::RestoreRecoveryConflict(live.to_path_buf())),
    }
    fs::rename(staging, live).map_err(|source| Error::RestoreFilesystem {
        operation: "rename staged restore member into live path",
        source,
    })?;
    sync_parent(live, "sync live restore replacement")?;
    verify_restore_path(live, expected, kind, member_name, runtime).await
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn restore_member_matches(path: &Path, expected: RestoreMemberExpectation) -> Result<bool, Error> {
    let (length, digest) = fingerprint(path)?;
    Ok(length == expected.byte_length && digest == expected.sha256)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum RestoreEntryKind {
    Missing,
    File,
    Other,
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn restore_entry_kind(path: &Path) -> Result<RestoreEntryKind, Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            Ok(RestoreEntryKind::File)
        }
        Ok(_) => Ok(RestoreEntryKind::Other),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            Ok(RestoreEntryKind::Missing)
        }
        Err(source) => Err(Error::RestoreFilesystem {
            operation: "inspect restore path",
            source,
        }),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn remove_restore_file(path: &Path, operation: &'static str) -> Result<(), Error> {
    match restore_entry_kind(path)? {
        RestoreEntryKind::Missing => Ok(()),
        RestoreEntryKind::File => {
            fs::remove_file(path)
                .map_err(|source| Error::RestoreFilesystem { operation, source })?;
            sync_parent(path, "sync restore cleanup")
        }
        RestoreEntryKind::Other => Err(Error::RestoreRecoveryConflict(path.to_path_buf())),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn require_sqlite_sidecars_absent(paths: &crate::Paths) -> Result<(), Error> {
    for live in [paths.runtime(), paths.private()] {
        let name = live
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| Error::InvalidPath(live.to_path_buf()))?;
        for suffix in ["wal", "shm"] {
            let sidecar = live.with_file_name(format!("{name}-{suffix}"));
            if restore_entry_kind(&sidecar)? != RestoreEntryKind::Missing {
                return Err(Error::RestoreRecoveryConflict(sidecar));
            }
        }
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
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

#[cfg_attr(coverage_nightly, coverage(off))]
fn sync_parent(path: &Path, operation: &'static str) -> Result<(), Error> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::InvalidPath(path.to_path_buf()))?;
    sync_directory(parent, operation)
}

#[cfg_attr(coverage_nightly, coverage(off))]
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

#[cfg_attr(coverage_nightly, coverage(off))]
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

#[cfg_attr(coverage_nightly, coverage(off))]
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

#[cfg_attr(coverage_nightly, coverage(off))]
fn sync_directory(path: &Path, operation: &'static str) -> Result<(), Error> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| Error::BackupFilesystem { operation, source })
}

#[cfg_attr(coverage_nightly, coverage(off))]
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

#[cfg_attr(coverage_nightly, coverage(off))]
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

#[cfg_attr(coverage_nightly, coverage(off))]
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

#[cfg_attr(coverage_nightly, coverage(off))]
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

#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use radroots_storage::{
        backup::{
            BackupFormatVersion, BackupId, BackupMemberKind, BackupPlan, BackupSecretPolicy,
            MemberVerification, RestorePlan,
        },
        event::SourceGeneration,
        status::ShutdownState,
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
    const RESTORE_FINALIZE_POLICY: &str =
        include_str!("../../../contracts/storage/restore_finalize_policy_v1.toml");
    const FAILURE_POLICY: &str =
        include_str!("../../../contracts/storage/failure_injection_policy_v1.toml");

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

    #[derive(Deserialize)]
    struct RestoreFinalizePolicy {
        schema_version: u32,
        authority: String,
        quiescence: String,
        wal_sidecars: String,
        marker: String,
        marker_encoding: String,
        marker_durability: Vec<String>,
        previous_runtime: String,
        previous_protected: String,
        replacement: Vec<String>,
        recovery: String,
        read_only_recovery: String,
        cleanup_order: Vec<String>,
        backend_after_attempt: String,
    }

    #[derive(Deserialize)]
    struct FailurePolicy {
        schema_version: u32,
        strategy: String,
        runtime_global_hooks: bool,
        accepted_reopen_outcomes: Vec<String>,
        atomic_sql_points: Vec<String>,
        migration_points: Vec<String>,
        backup_points: Vec<String>,
        restore_points: Vec<String>,
        lock_close_points: Vec<String>,
        restore_recovery: String,
        failure_reporting: String,
    }

    #[derive(Clone, Copy, Debug)]
    #[repr(u8)]
    enum RestoreCrashPoint {
        MarkerPersisted = 0,
        RuntimePreviousRenamed = 1,
        RuntimeReplacementRenamed = 2,
        ProtectedPreviousRenamed = 3,
        ProtectedReplacementRenamed = 4,
        RuntimePreviousCleaned = 5,
        ProtectedPreviousCleaned = 6,
        MarkerCleaned = 7,
    }

    impl RestoreCrashPoint {
        const ALL: [Self; 8] = [
            Self::MarkerPersisted,
            Self::RuntimePreviousRenamed,
            Self::RuntimeReplacementRenamed,
            Self::ProtectedPreviousRenamed,
            Self::ProtectedReplacementRenamed,
            Self::RuntimePreviousCleaned,
            Self::ProtectedPreviousCleaned,
            Self::MarkerCleaned,
        ];

        const fn reached(self, point: Self) -> bool {
            self as u8 >= point as u8
        }
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

    #[tokio::test]
    async fn aggregate_reliability_state_is_idempotent_conflict_safe_and_close_aware() {
        let database_root = tempfile::tempdir().expect("database root");
        let (_paths, store) = create(database_root.path(), None).await;
        let backup = plan(44, BackupSecretPolicy::ExcludeProtectedStorage, 4_400);

        let planned = StorageReliability::begin_backup(&store, backup.clone())
            .await
            .expect("planned backup");
        assert_eq!(
            StorageReliability::begin_backup(&store, backup.clone())
                .await
                .expect("idempotent backup"),
            planned
        );
        let conflicting = plan(44, BackupSecretPolicy::IncludeProtectedStorage, 4_400);
        assert_eq!(
            StorageReliability::begin_backup(&store, conflicting).await,
            Err(StorageError::ReliabilityRevisionConflict)
        );

        let manifest = BackupManifest::new(
            backup.format_version(),
            backup.backup_id(),
            backup.requested_at_unix_ms(),
            backup.secret_policy(),
            vec![
                BackupMember::new(
                    RUNTIME_MEMBER,
                    BackupMemberKind::Runtime,
                    1,
                    MemberDigest::new([1; 32]),
                )
                .expect("runtime member"),
            ],
        )
        .expect("restore manifest");
        let restore = RestorePlan::new(
            manifest.clone(),
            BackupSecretPolicy::ExcludeProtectedStorage,
            4_401,
        )
        .expect("restore plan");
        let staging = StorageReliability::begin_restore(&store, restore.clone())
            .await
            .expect("staging restore");
        assert_eq!(
            StorageReliability::begin_restore(&store, restore)
                .await
                .expect("idempotent restore"),
            staging
        );
        let conflicting_restore =
            RestorePlan::new(manifest, BackupSecretPolicy::ExcludeProtectedStorage, 4_402)
                .expect("conflicting restore plan");
        assert_eq!(
            StorageReliability::begin_restore(&store, conflicting_restore).await,
            Err(StorageError::ReliabilityRevisionConflict)
        );

        let failed = StorageReliability::transition_backup(
            &store,
            backup.backup_id(),
            planned.revision(),
            BackupTransition::Fail,
            4_401,
        )
        .await
        .expect("failed transition");
        assert_eq!(
            failed.stage(),
            radroots_storage::backup::BackupStage::Failed
        );
        assert_eq!(
            StorageReliability::transition_backup(
                &store,
                backup.backup_id(),
                failed.revision(),
                BackupTransition::Fail,
                4_402,
            )
            .await,
            Err(StorageError::ReliabilityOperationTerminal)
        );

        let status = StorageReliability::status(&store)
            .await
            .expect("open status");
        assert_eq!(status.shutdown(), ShutdownState::Open);
        let closed = StorageReliability::close(&store)
            .await
            .expect("close storage");
        assert_eq!(closed.shutdown(), ShutdownState::Closed);
        assert_eq!(
            StorageReliability::begin_backup(
                &store,
                plan(45, BackupSecretPolicy::ExcludeProtectedStorage, 4_500)
            )
            .await,
            Err(StorageError::BackendUnavailable)
        );
    }

    async fn insert_private_artifact(store: &SqliteStorage, byte: u8) {
        sqlx::query(
            "INSERT INTO radroots_private_artifacts (
               artifact_id, artifact_kind, schema_id, commitment,
               protected_size_bytes, secret_provider, secret_reference,
               key_version, envelope_version, encrypted_envelope,
               delete_not_before_unix_ms, expires_at_unix_ms, revision, stage,
               created_at_unix_ms, updated_at_unix_ms, deleted_at_unix_ms,
               deletion_reason, tombstone_commitment
             ) VALUES (?, 'test', 'test.v1', ?, 1, 'test', 'ref', 1,
                       NULL, NULL, NULL, NULL, 1, 'active', 1, 1, NULL, NULL, NULL)",
        )
        .bind(vec![byte; 16])
        .bind(vec![byte; 32])
        .execute(&store.private_pool)
        .await
        .expect("insert private artifact");
    }

    fn construct_restore_crash_state(
        layout: &RestoreLayout,
        marker: &RestoreMarker,
        point: RestoreCrashPoint,
    ) {
        write_restore_marker(&layout.marker, marker).expect("persist interruption marker");
        if point.reached(RestoreCrashPoint::RuntimePreviousRenamed) {
            fs::rename(&layout.runtime_live, &layout.runtime_previous)
                .expect("rename runtime previous");
            sync_parent(&layout.runtime_live, "sync runtime previous")
                .expect("sync runtime previous");
        }
        if point.reached(RestoreCrashPoint::RuntimeReplacementRenamed) {
            fs::rename(&layout.runtime_staging, &layout.runtime_live)
                .expect("promote runtime replacement");
            sync_parent(&layout.runtime_live, "sync runtime replacement")
                .expect("sync runtime replacement");
        }
        if point.reached(RestoreCrashPoint::ProtectedPreviousRenamed) {
            fs::rename(&layout.private_live, &layout.private_previous)
                .expect("rename protected previous");
            sync_parent(&layout.private_live, "sync protected previous")
                .expect("sync protected previous");
        }
        if point.reached(RestoreCrashPoint::ProtectedReplacementRenamed) {
            fs::rename(&layout.private_staging, &layout.private_live)
                .expect("promote protected replacement");
            sync_parent(&layout.private_live, "sync protected replacement")
                .expect("sync protected replacement");
        }
        if point.reached(RestoreCrashPoint::RuntimePreviousCleaned) {
            remove_restore_file(&layout.runtime_previous, "inject runtime cleanup")
                .expect("clean runtime previous");
        }
        if point.reached(RestoreCrashPoint::ProtectedPreviousCleaned) {
            remove_restore_file(&layout.private_previous, "inject protected cleanup")
                .expect("clean protected previous");
        }
        if point.reached(RestoreCrashPoint::MarkerCleaned) {
            remove_restore_file(&layout.marker, "inject marker cleanup").expect("clean marker");
        }
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

    #[test]
    fn implementation_matches_the_governed_restore_finalize_policy() {
        let policy = toml::from_str::<RestoreFinalizePolicy>(RESTORE_FINALIZE_POLICY)
            .expect("restore finalize policy");
        assert_eq!(policy.schema_version, 1);
        assert_eq!(
            policy.authority,
            "open_writable_backend_with_exclusive_writer_lock"
        );
        assert_eq!(
            policy.quiescence,
            "close_all_owned_pools_before_live_rename"
        );
        assert_eq!(
            policy.wal_sidecars,
            "marker_precedes_and_fences_absence_before_live_rename"
        );
        assert_eq!(
            policy.marker,
            ".radroots-storage-restore-{backup_id_hex}.marker"
        );
        assert_eq!(
            policy.marker_encoding,
            "fixed_binary_v1_exact_member_lengths_and_sha256"
        );
        assert_eq!(
            policy.marker_durability,
            ["marker_file_fsync", "runtime_parent_fsync"]
        );
        assert_eq!(
            policy.previous_runtime,
            ".runtime.sqlite.restore-{backup_id_hex}.previous"
        );
        assert_eq!(
            policy.previous_protected,
            ".private.sqlite.restore-{backup_id_hex}.previous"
        );
        assert_eq!(
            policy.replacement,
            [
                "live_to_previous_atomic_rename",
                "staging_to_live_atomic_rename"
            ]
        );
        assert_eq!(
            policy.recovery,
            "marker_driven_idempotent_forward_completion_before_open"
        );
        assert_eq!(policy.read_only_recovery, "reject");
        assert_eq!(
            policy.cleanup_order,
            [
                "verify_all_live_members",
                "remove_previous_members",
                "remove_marker"
            ]
        );
        assert_eq!(policy.backend_after_attempt, "closed_reopen_required");
    }

    #[test]
    fn implementation_matches_the_governed_failure_injection_policy() {
        let policy = toml::from_str::<FailurePolicy>(FAILURE_POLICY).expect("failure policy");
        assert_eq!(policy.schema_version, 1);
        assert_eq!(
            policy.strategy,
            "deterministic_state_construction_and_sql_faults"
        );
        assert!(!policy.runtime_global_hooks);
        assert_eq!(
            policy.accepted_reopen_outcomes,
            [
                "fully_committed_replayable",
                "typed_recoverable_no_partial_success"
            ]
        );
        assert_eq!(
            policy.atomic_sql_points,
            [
                "source_sequence",
                "event",
                "provenance",
                "projection_checkpoint",
                "commit_receipt",
                "journal",
                "outbox_item",
                "outbox_target",
                "delivery_evidence"
            ]
        );
        assert_eq!(
            policy.migration_points,
            [
                "application_identity",
                "each_pending_step",
                "user_version",
                "exact_catalog",
                "transaction_commit"
            ]
        );
        assert_eq!(
            policy.backup_points,
            [
                "runtime_snapshot",
                "protected_snapshot",
                "member_hash",
                "manifest",
                "complete_verification",
                "final_directory_rename",
                "root_sync"
            ]
        );
        assert_eq!(
            policy.restore_points,
            [
                "runtime_staging",
                "protected_staging",
                "staged_validation",
                "marker",
                "runtime_previous_rename",
                "runtime_replacement_rename",
                "protected_previous_rename",
                "protected_replacement_rename",
                "installed_validation",
                "runtime_previous_cleanup",
                "protected_previous_cleanup",
                "marker_cleanup"
            ]
        );
        assert_eq!(
            policy.lock_close_points,
            [
                "lock_file_open",
                "exclusive_acquisition",
                "cross_process_contention",
                "pool_drain",
                "lock_release",
                "closed_status"
            ]
        );
        assert_eq!(
            policy.restore_recovery,
            "idempotent_forward_completion_before_connection_open"
        );
        assert_eq!(
            policy.failure_reporting,
            "stable_typed_error_without_backend_details"
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

    #[tokio::test]
    async fn restore_finalization_atomically_replaces_every_selected_member_and_closes() {
        let database_root = tempfile::tempdir().expect("database root");
        let backup_parent = tempfile::tempdir().expect("backup parent");
        let backup_root = backup_parent.path().join("backups");
        fs::create_dir(&backup_root).expect("backup root");
        let (paths, store) = create(database_root.path(), Some(&backup_root)).await;
        sqlx::query(
            "UPDATE radroots_runtime_source_generations SET sequence_head = 61 WHERE state = 'active'",
        )
        .execute(&store.pool)
        .await
        .expect("initial runtime state");
        insert_private_artifact(&store, 1).await;
        let backup_plan = plan(102, BackupSecretPolicy::IncludeProtectedStorage, 10_300);
        let manifest = store
            .capture_backup(&backup_plan)
            .await
            .expect("capture restore source");
        store
            .finalize_backup(&backup_plan, &manifest)
            .await
            .expect("finalize restore source");
        sqlx::query(
            "UPDATE radroots_runtime_source_generations SET sequence_head = 62 WHERE state = 'active'",
        )
        .execute(&store.pool)
        .await
        .expect("advance runtime state");
        insert_private_artifact(&store, 2).await;
        let restore = RestorePlan::new(
            manifest.clone(),
            BackupSecretPolicy::IncludeProtectedStorage,
            10_400,
        )
        .expect("restore plan");
        store.stage_restore(&restore).await.expect("stage restore");
        let layout = RestoreLayout::new(&paths, manifest.backup_id()).expect("restore layout");
        let held_connection = store.pool.acquire().await.expect("held connection");
        let restoring = store.clone();
        let restore_plan = restore.clone();
        let finalization = tokio::spawn(async move {
            restoring
                .finalize_restore(&restore_plan)
                .await
                .expect("finalize restore")
        });
        for _ in 0..10_000 {
            if store
                .storage_status()
                .await
                .expect("restoring status")
                .shutdown()
                == ShutdownState::Closing
            {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(!finalization.is_finished());
        assert_eq!(
            store
                .storage_status()
                .await
                .expect("restoring status")
                .shutdown(),
            ShutdownState::Closing
        );
        let concurrent_close_store = store.clone();
        let concurrent_close = tokio::spawn(async move {
            concurrent_close_store
                .close()
                .await
                .expect("concurrent close")
        });
        tokio::task::yield_now().await;
        assert!(!concurrent_close.is_finished());
        assert!(matches!(
            SqliteStorage::open(OpenOptions::new(paths.clone(), OpenMode::ReadWriteExisting)).await,
            Err(Error::WriterAlreadyActive { .. })
        ));
        drop(held_connection);
        let (finalization, concurrent_close) = tokio::join!(finalization, concurrent_close);
        finalization.expect("restore finalization task");
        let close_status = concurrent_close.expect("concurrent close task");
        assert!(matches!(
            close_status.shutdown(),
            ShutdownState::Closing | ShutdownState::Closed
        ));
        assert_eq!(
            store
                .storage_status()
                .await
                .expect("closed restore status")
                .shutdown(),
            ShutdownState::Closed
        );

        let reopened = SqliteStorage::open(
            OpenOptions::new(paths.clone(), OpenMode::ReadWriteExisting)
                .with_backup_root(&backup_root)
                .expect("backup root"),
        )
        .await
        .expect("reopen restored storage");
        assert_eq!(
            scalar(
                paths.runtime(),
                "SELECT sequence_head FROM radroots_runtime_source_generations WHERE state = 'active'"
            )
            .await,
            61
        );
        assert_eq!(
            scalar(
                paths.private(),
                "SELECT COUNT(*) FROM radroots_private_artifacts"
            )
            .await,
            1
        );
        for path in [
            layout.runtime_staging,
            layout.private_staging,
            layout.runtime_previous,
            layout.private_previous,
            layout.marker,
        ] {
            assert!(
                !path.exists(),
                "restore artifact remained: {}",
                path.display()
            );
        }
        reopened.close().await.expect("close restored storage");
    }

    #[tokio::test]
    async fn writable_open_completes_an_interrupted_restore_before_connections_open() {
        let database_root = tempfile::tempdir().expect("database root");
        let backup_parent = tempfile::tempdir().expect("backup parent");
        let backup_root = backup_parent.path().join("backups");
        fs::create_dir(&backup_root).expect("backup root");
        let (paths, store) = create(database_root.path(), Some(&backup_root)).await;
        sqlx::query(
            "UPDATE radroots_runtime_source_generations SET sequence_head = 71 WHERE state = 'active'",
        )
        .execute(&store.pool)
        .await
        .expect("initial runtime state");
        insert_private_artifact(&store, 3).await;
        let backup_plan = plan(103, BackupSecretPolicy::IncludeProtectedStorage, 10_500);
        let manifest = store
            .capture_backup(&backup_plan)
            .await
            .expect("capture restore source");
        store
            .finalize_backup(&backup_plan, &manifest)
            .await
            .expect("finalize restore source");
        sqlx::query(
            "UPDATE radroots_runtime_source_generations SET sequence_head = 72 WHERE state = 'active'",
        )
        .execute(&store.pool)
        .await
        .expect("advance runtime state");
        insert_private_artifact(&store, 4).await;
        let restore = RestorePlan::new(
            manifest.clone(),
            BackupSecretPolicy::IncludeProtectedStorage,
            10_600,
        )
        .expect("restore plan");
        store.stage_restore(&restore).await.expect("stage restore");
        store.close().await.expect("close before simulated crash");
        require_sqlite_sidecars_absent(&paths).expect("quiesced SQLite sidecars");
        let marker = RestoreMarker::from_manifest(&manifest).expect("restore marker");
        let layout = RestoreLayout::new(&paths, manifest.backup_id()).expect("restore layout");
        write_restore_marker(&layout.marker, &marker).expect("persist restore marker");
        fs::rename(&layout.runtime_live, &layout.runtime_previous)
            .expect("simulate interrupted previous rename");
        fs::rename(&layout.runtime_staging, &layout.runtime_live)
            .expect("simulate installed runtime member");
        sync_parent(&layout.runtime_live, "sync simulated interruption")
            .expect("sync simulated interruption");

        assert!(matches!(
            SqliteStorage::open(OpenOptions::new(paths.clone(), OpenMode::ReadOnly)).await,
            Err(Error::RestoreRequiresWritableStorage)
        ));
        let runtime_wal = paths.runtime().with_file_name("runtime.sqlite-wal");
        fs::write(&runtime_wal, b"simulated reader sidecar").expect("simulated WAL sidecar");
        assert!(matches!(
            SqliteStorage::open(OpenOptions::new(
                paths.clone(),
                OpenMode::ReadWriteExisting
            ))
            .await,
            Err(Error::RestoreRecoveryConflict(path)) if path == runtime_wal
        ));
        assert!(layout.marker.is_file());
        fs::remove_file(&runtime_wal).expect("remove simulated WAL sidecar");
        sync_parent(&runtime_wal, "sync simulated WAL cleanup")
            .expect("sync simulated WAL cleanup");
        let recovered =
            SqliteStorage::open(OpenOptions::new(paths.clone(), OpenMode::ReadWriteExisting))
                .await
                .expect("recover interrupted restore");
        assert_eq!(
            scalar(
                paths.runtime(),
                "SELECT sequence_head FROM radroots_runtime_source_generations WHERE state = 'active'"
            )
            .await,
            71
        );
        assert_eq!(
            scalar(
                paths.private(),
                "SELECT COUNT(*) FROM radroots_private_artifacts"
            )
            .await,
            1
        );
        for path in [
            layout.runtime_staging,
            layout.private_staging,
            layout.runtime_previous,
            layout.private_previous,
            layout.marker,
        ] {
            assert!(
                !path.exists(),
                "recovery artifact remained: {}",
                path.display()
            );
        }
        recovered.close().await.expect("close recovered storage");
    }

    #[tokio::test]
    async fn every_durable_restore_crash_point_recovers_to_one_complete_installation() {
        for (index, point) in RestoreCrashPoint::ALL.into_iter().enumerate() {
            let database_root = tempfile::tempdir().expect("database root");
            let backup_parent = tempfile::tempdir().expect("backup parent");
            let backup_root = backup_parent.path().join("backups");
            fs::create_dir(&backup_root).expect("backup root");
            let (paths, store) = create(database_root.path(), Some(&backup_root)).await;
            sqlx::query(
                "UPDATE radroots_runtime_source_generations SET sequence_head = 81 WHERE state = 'active'",
            )
            .execute(&store.pool)
            .await
            .expect("backup runtime state");
            insert_private_artifact(&store, 5).await;
            let backup_plan = plan(
                110 + u8::try_from(index).expect("crash index"),
                BackupSecretPolicy::IncludeProtectedStorage,
                11_000 + u64::try_from(index).expect("crash index"),
            );
            let manifest = store
                .capture_backup(&backup_plan)
                .await
                .expect("capture crash source");
            store
                .finalize_backup(&backup_plan, &manifest)
                .await
                .expect("finalize crash source");
            sqlx::query(
                "UPDATE radroots_runtime_source_generations SET sequence_head = 82 WHERE state = 'active'",
            )
            .execute(&store.pool)
            .await
            .expect("advance runtime state");
            insert_private_artifact(&store, 6).await;
            let restore = RestorePlan::new(
                manifest.clone(),
                BackupSecretPolicy::IncludeProtectedStorage,
                12_000 + u64::try_from(index).expect("crash index"),
            )
            .expect("restore plan");
            store.stage_restore(&restore).await.expect("stage restore");
            store.close().await.expect("quiesce crash state");
            require_sqlite_sidecars_absent(&paths).expect("quiesced SQLite sidecars");
            let marker = RestoreMarker::from_manifest(&manifest).expect("restore marker");
            let layout = RestoreLayout::new(&paths, manifest.backup_id()).expect("restore layout");
            construct_restore_crash_state(&layout, &marker, point);

            let recovered =
                SqliteStorage::open(OpenOptions::new(paths.clone(), OpenMode::ReadWriteExisting))
                    .await
                    .unwrap_or_else(|error| panic!("recover {point:?}: {error}"));
            assert_eq!(
                scalar(
                    paths.runtime(),
                    "SELECT sequence_head FROM radroots_runtime_source_generations WHERE state = 'active'"
                )
                .await,
                81,
                "runtime state after {point:?}"
            );
            assert_eq!(
                scalar(
                    paths.private(),
                    "SELECT COUNT(*) FROM radroots_private_artifacts"
                )
                .await,
                1,
                "protected state after {point:?}"
            );
            for artifact in [
                &layout.runtime_staging,
                &layout.private_staging,
                &layout.runtime_previous,
                &layout.private_previous,
                &layout.marker,
            ] {
                assert!(
                    !artifact.exists(),
                    "artifact after {point:?}: {}",
                    artifact.display()
                );
            }
            recovered.close().await.expect("close recovered state");
        }
    }

    #[tokio::test]
    async fn corrupt_restore_markers_fail_closed_without_opening_live_state() {
        let database_root = tempfile::tempdir().expect("database root");
        let (paths, store) = create(database_root.path(), None).await;
        store.close().await.expect("close storage");
        let backup_id = BackupId::new([104; 16]).expect("backup id");
        let layout = RestoreLayout::new(&paths, backup_id).expect("restore layout");
        fs::write(&layout.marker, [0_u8; RESTORE_MARKER_BYTES]).expect("corrupt marker");
        sync_parent(&layout.marker, "sync corrupt marker").expect("sync corrupt marker");
        assert!(matches!(
            SqliteStorage::open(OpenOptions::new(paths, OpenMode::ReadWriteExisting)).await,
            Err(Error::RestoreMarkerCorrupt(_))
        ));

        let marker_path = Path::new("restore.marker");
        for private in [
            None,
            Some(RestoreMemberExpectation {
                byte_length: 2,
                sha256: MemberDigest::new([2; 32]),
            }),
        ] {
            let marker = RestoreMarker {
                backup_id,
                secret_policy: if private.is_some() {
                    BackupSecretPolicy::IncludeProtectedStorage
                } else {
                    BackupSecretPolicy::ExcludeProtectedStorage
                },
                runtime: RestoreMemberExpectation {
                    byte_length: 1,
                    sha256: MemberDigest::new([1; 32]),
                },
                private,
            };
            let encoded = marker.encode();
            assert_eq!(
                RestoreMarker::decode(marker_path, &encoded)
                    .expect("decode marker")
                    .encode(),
                encoded
            );
            for end in 0..encoded.len() {
                let _ = RestoreMarker::decode(marker_path, &encoded[..end]);
            }
            for index in 0..encoded.len() {
                let mut corrupt = encoded;
                corrupt[index] ^= 0xff;
                let _ = RestoreMarker::decode(marker_path, &corrupt);
            }
        }

        let valid = RestoreMarker {
            backup_id,
            secret_policy: BackupSecretPolicy::ExcludeProtectedStorage,
            runtime: RestoreMemberExpectation {
                byte_length: 1,
                sha256: MemberDigest::new([1; 32]),
            },
            private: None,
        }
        .encode();
        let mut zero_runtime = valid;
        zero_runtime[25..33].copy_from_slice(&0_u64.to_be_bytes());
        assert!(RestoreMarker::decode(marker_path, &zero_runtime).is_err());
        let mut unexpected_private = valid;
        unexpected_private[65..73].copy_from_slice(&1_u64.to_be_bytes());
        assert!(RestoreMarker::decode(marker_path, &unexpected_private).is_err());
    }

    #[test]
    fn manifest_validation_rejects_each_governed_identity_mismatch() {
        fn manifest(
            id: u8,
            policy: BackupSecretPolicy,
            created_at: u64,
            runtime_path: &'static str,
        ) -> BackupManifest {
            let mut members = vec![
                BackupMember::new(
                    runtime_path,
                    BackupMemberKind::Runtime,
                    1,
                    MemberDigest::new([1; 32]),
                )
                .expect("runtime member"),
            ];
            if policy == BackupSecretPolicy::IncludeProtectedStorage {
                members.push(
                    BackupMember::new(
                        PRIVATE_MEMBER,
                        BackupMemberKind::Protected,
                        2,
                        MemberDigest::new([2; 32]),
                    )
                    .expect("private member"),
                );
            }
            BackupManifest::new(
                BackupFormatVersion::V1,
                BackupId::new([id; 16]).expect("backup id"),
                created_at,
                policy,
                members,
            )
            .expect("backup manifest")
        }

        let plan = plan(120, BackupSecretPolicy::ExcludeProtectedStorage, 12_000);
        let valid = manifest(
            120,
            BackupSecretPolicy::ExcludeProtectedStorage,
            12_000,
            RUNTIME_MEMBER,
        );
        assert!(validate_manifest(&plan, &valid).is_ok());
        for invalid in [
            manifest(
                121,
                BackupSecretPolicy::ExcludeProtectedStorage,
                12_000,
                RUNTIME_MEMBER,
            ),
            manifest(
                120,
                BackupSecretPolicy::IncludeProtectedStorage,
                12_000,
                RUNTIME_MEMBER,
            ),
            manifest(
                120,
                BackupSecretPolicy::ExcludeProtectedStorage,
                12_001,
                RUNTIME_MEMBER,
            ),
            manifest(
                120,
                BackupSecretPolicy::ExcludeProtectedStorage,
                12_000,
                "runtime/alternate.sqlite",
            ),
        ] {
            assert!(matches!(
                validate_manifest(&plan, &invalid),
                Err(Error::BackupVerificationFailed { member: "manifest" })
            ));
        }
    }
}

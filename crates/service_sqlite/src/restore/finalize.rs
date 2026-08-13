//! Atomic installation of one completely verified offline restore stage.

#[cfg(any(target_os = "linux", target_os = "macos"))]
use core::fmt;

use crate::{ServiceSqliteError, ServiceSqliteErrorKind, StagedServiceRestore};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use {
    super::{
        RestoreArtifactExpectation, RestoreMarkerBinding, RestoreRecoveryMarker,
        RestoreRecoveryPhase,
        marker::{BACKUP_FILE_NAME, LIVE_FILE_NAME, STAGED_FILE_NAME},
        stage::NativeStagedServiceRestore,
    },
    rustix::{
        fs::{FileType, Mode, OFlags, RenameFlags, fstat, openat, renameat_with},
        process::geteuid,
    },
    sha2::{Digest, Sha256},
    std::{
        error::Error,
        fs::File,
        os::unix::fs::FileExt,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU8, Ordering},
        },
    },
};

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
use super::marker::MARKER_NEXT_FILE_NAME;

#[cfg(any(target_os = "linux", target_os = "macos"))]
const HASH_BUFFER_BYTES: usize = 64 * 1_024;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const PHASE_BEFORE_PREPARED: u8 = 1;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const PHASE_COMMIT_OWNED: u8 = 2;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const PHASE_AFTER_PREPARED: u8 = 3;

#[cfg(any(target_os = "linux", target_os = "macos"))]
const CANCELLABLE: u8 = 0;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const CANCELLED: u8 = 1;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const COMMIT_OWNED: u8 = 2;

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
pub(crate) static TEST_FINALIZE_PHASE: AtomicU8 = AtomicU8::new(0);
#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
pub(crate) static TEST_FINALIZE_BLOCK_PHASE: AtomicU8 = AtomicU8::new(0);
#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
static TEST_FINALIZE_FAILURE: AtomicU8 = AtomicU8::new(0);

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
pub(crate) const TEST_PHASE_BEFORE_PREPARED: u8 = PHASE_BEFORE_PREPARED;
#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
pub(crate) const TEST_PHASE_COMMIT_OWNED: u8 = PHASE_COMMIT_OWNED;
#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
pub(crate) const TEST_PHASE_AFTER_PREPARED: u8 = PHASE_AFTER_PREPARED;

/// Atomically installs a completely verified adjacent restore stage.
///
/// Cancellation observed before the worker atomically claims commit ownership
/// leaves the live database untouched and attempts exact stage cleanup. Caller
/// loss after that in-memory handoff has an unknown immediate outcome, even if
/// the durable `prepared` marker has not appeared yet: the owned worker retains
/// writer authority until it either fails before durability or establishes
/// recovery evidence and continues. Once `prepared` is durable, the staged
/// artifact is retained unconditionally for recovery. A successful return
/// provides no open database handle; the next open must reconcile and retire
/// the retained marker and old live database.
pub async fn finalize_staged_restore(
    staged: StagedServiceRestore,
) -> Result<(), ServiceSqliteError> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let failpoints = crate::failpoint::DurabilityFailpoints::default();
        let cancellation = Arc::new(AtomicU8::new(CANCELLABLE));
        let cancellation_on_drop = CancellationOnDrop::new(Arc::clone(&cancellation));
        let native = staged.into_native();
        let result = tokio::task::spawn_blocking(move || {
            finalize_native(
                native,
                &cancellation,
                &SystemFinalizeOperations,
                &failpoints,
            )
        })
        .await
        .map_err(|source| finalize_source(FinalizeFailureKind::Join, source))?;
        cancellation_on_drop.disarm();
        result
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = staged;
        Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Restore))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn finalize_native(
    staged: NativeStagedServiceRestore,
    cancellation: &AtomicU8,
    operations: &dyn FinalizeOperations,
    failpoints: &crate::failpoint::DurabilityFailpoints,
) -> Result<(), ServiceSqliteError> {
    staged.validate()?;
    check_cancel(cancellation)?;
    let live = authority_checked(&staged, || open_live(staged.directory()))??;
    let live_artifact = staged.live_artifact();
    authority_checked(&staged, || {
        verify_named_artifact(
            staged.directory(),
            LIVE_FILE_NAME,
            &live,
            live_artifact,
            Some(cancellation),
        )
    })??;
    authority_checked(&staged, || {
        verify_named_artifact(
            staged.directory(),
            STAGED_FILE_NAME,
            staged.staged_file(),
            staged.artifact(),
            Some(cancellation),
        )
    })??;
    authority_checked(&staged, || {
        live.sync_all()
            .map_err(|source| finalize_source(FinalizeFailureKind::SyncLive, source))
    })??;
    authority_checked(&staged, || {
        staged
            .staged_file()
            .sync_all()
            .map_err(|source| finalize_source(FinalizeFailureKind::SyncStaged, source))
    })??;
    test_phase(PHASE_BEFORE_PREPARED, cancellation, true)?;
    staged.validate()?;
    claim_commit_ownership(cancellation)?;
    test_phase(PHASE_COMMIT_OWNED, cancellation, false)?;

    let marker = RestoreRecoveryMarker::prepared(
        staged.metadata(),
        staged.manifest_digest(),
        live_artifact,
        staged.artifact(),
    )
    .map_err(|source| ServiceSqliteError::with_source(ServiceSqliteErrorKind::Restore, source))?;
    let on_durable = || {
        staged.disarm_cleanup();
        let _ = test_phase(PHASE_AFTER_PREPARED, cancellation, false);
    };
    #[cfg(test)]
    let marker_result = if operations.drift_authority_during_marker_sync() {
        RestoreMarkerBinding::test_create_with_durable_authority_drift(
            staged.paths(),
            staged.authority(),
            &marker,
            on_durable,
        )
    } else {
        RestoreMarkerBinding::create_with_durable_callback_and_failpoints(
            staged.paths(),
            staged.authority(),
            &marker,
            failpoints,
            on_durable,
        )
    };
    #[cfg(not(test))]
    let marker_result = RestoreMarkerBinding::create_with_durable_callback_and_failpoints(
        staged.paths(),
        staged.authority(),
        &marker,
        failpoints,
        on_durable,
    );
    let mut marker = marker_result?;

    rename_and_sync(
        &staged,
        operations,
        RenameStep::RetainLive,
        RenameArtifact {
            source_name: LIVE_FILE_NAME,
            destination_name: BACKUP_FILE_NAME,
            held: &live,
            expected: live_artifact,
        },
        failpoints,
    )?;
    authority_checked(&staged, || {
        operations.after_directory_sync(staged.directory(), RenameStep::RetainLive)
    })??;
    marker = marker.advance_with_failpoints(
        staged.paths(),
        staged.authority(),
        RestoreRecoveryPhase::LiveRetained,
        failpoints,
    )?;

    rename_and_sync(
        &staged,
        operations,
        RenameStep::InstallStage,
        RenameArtifact {
            source_name: STAGED_FILE_NAME,
            destination_name: LIVE_FILE_NAME,
            held: staged.staged_file(),
            expected: staged.artifact(),
        },
        failpoints,
    )?;
    authority_checked(&staged, || {
        operations.after_directory_sync(staged.directory(), RenameStep::InstallStage)
    })??;
    marker = marker.advance_with_failpoints(
        staged.paths(),
        staged.authority(),
        RestoreRecoveryPhase::ReplacementInstalled,
        failpoints,
    )?;
    require_finalize_condition(
        marker.marker().phase() == RestoreRecoveryPhase::ReplacementInstalled,
        FinalizeFailureKind::Marker,
    )?;
    staged.validate_finalization_authority()?;
    drop(marker);
    drop(staged);
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn rename_and_sync(
    staged: &NativeStagedServiceRestore,
    operations: &dyn FinalizeOperations,
    step: RenameStep,
    artifact: RenameArtifact<'_>,
    failpoints: &crate::failpoint::DurabilityFailpoints,
) -> Result<(), ServiceSqliteError> {
    authority_checked(staged, || {
        verify_named_artifact(
            staged.directory(),
            artifact.source_name,
            artifact.held,
            artifact.expected,
            None,
        )
    })??;
    authority_checked(staged, || {
        hit(
            failpoints,
            step.before_rename_failpoint(),
            step.rename_failure(),
        )
    })??;
    let rename = authority_checked(staged, || {
        operations
            .rename(
                staged.directory(),
                artifact.source_name,
                artifact.destination_name,
                step,
            )
            .map_err(|source| finalize_source(step.rename_failure(), source))
    })?;
    rename?;
    authority_checked(staged, || {
        hit(
            failpoints,
            step.after_rename_failpoint(),
            step.rename_failure(),
        )
    })??;
    authority_checked(staged, || {
        verify_named_artifact(
            staged.directory(),
            artifact.destination_name,
            artifact.held,
            artifact.expected,
            None,
        )
    })??;
    authority_checked(staged, || {
        hit(
            failpoints,
            step.before_sync_failpoint(),
            step.sync_failure(),
        )
    })??;
    let sync = authority_checked(staged, || {
        operations
            .sync_directory(staged.directory(), step)
            .map_err(|source| finalize_source(step.sync_failure(), source))
    })?;
    sync?;
    authority_checked(staged, || {
        hit(failpoints, step.after_sync_failpoint(), step.sync_failure())
    })??;
    authority_checked(staged, || {
        verify_named_artifact(
            staged.directory(),
            artifact.destination_name,
            artifact.held,
            artifact.expected,
            None,
        )
    })??;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn authority_checked<T>(
    staged: &NativeStagedServiceRestore,
    operation: impl FnOnce() -> T,
) -> Result<T, ServiceSqliteError> {
    staged.validate_finalization_authority()?;
    let result = operation();
    staged.validate_finalization_authority()?;
    Ok(result)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn open_live(directory: &File) -> Result<File, ServiceSqliteError> {
    openat(
        directory,
        LIVE_FILE_NAME,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|source| finalize_source(FinalizeFailureKind::Live, source))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn verify_named_artifact(
    directory: &File,
    name: &str,
    held: &File,
    expected: RestoreArtifactExpectation,
    cancellation: Option<&AtomicU8>,
) -> Result<(), ServiceSqliteError> {
    let current = openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|source| finalize_source(FinalizeFailureKind::Artifact, source))?;
    let held_status =
        fstat(held).map_err(|source| finalize_source(FinalizeFailureKind::Artifact, source))?;
    let current_status =
        fstat(&current).map_err(|source| finalize_source(FinalizeFailureKind::Artifact, source))?;
    validate_status(&held_status, Some(expected.byte_length()))?;
    validate_status(&current_status, Some(expected.byte_length()))?;
    let held_identity = (
        crate::native_metadata::device(held_status.st_dev)
            .map_err(|_| finalize_error(FinalizeFailureKind::Artifact))?,
        held_status.st_ino,
    );
    let current_identity = (
        crate::native_metadata::device(current_status.st_dev)
            .map_err(|_| finalize_error(FinalizeFailureKind::Artifact))?,
        current_status.st_ino,
    );
    require_finalize_condition(
        crate::native_metadata::identity_pair_matches(
            held_identity.0,
            held_identity.1,
            current_identity.0,
            current_identity.1,
            expected.device(),
            expected.inode(),
        ),
        FinalizeFailureKind::Artifact,
    )?;
    require_finalize_condition(
        hash_exact(held, expected.byte_length(), cancellation)? == expected.sha256(),
        FinalizeFailureKind::Artifact,
    )?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_status(
    status: &rustix::fs::Stat,
    expected_length: Option<u64>,
) -> Result<(), ServiceSqliteError> {
    let length =
        u64::try_from(status.st_size).map_err(|_| finalize_error(FinalizeFailureKind::Artifact))?;
    require_finalize_condition(
        crate::all_constraints([
            crate::native_metadata::exact_regular_file(
                FileType::from_raw_mode(status.st_mode).is_file(),
                crate::native_metadata::link_count(status.st_nlink),
                status.st_uid,
                geteuid().as_raw(),
                crate::native_metadata::mode(status.st_mode),
            ),
            crate::native_metadata::valid_artifact_length(length, expected_length),
        ]),
        FinalizeFailureKind::Artifact,
    )?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn hash_exact(
    file: &File,
    expected_length: u64,
    cancellation: Option<&AtomicU8>,
) -> Result<[u8; 32], ServiceSqliteError> {
    let mut offset = 0_u64;
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    let mut hasher = Sha256::new();
    while offset < expected_length {
        if cancellation.is_some_and(|state| state.load(Ordering::Acquire) == CANCELLED) {
            return Err(finalize_error(FinalizeFailureKind::Cancelled));
        }
        let requested = usize::try_from((expected_length - offset).min(HASH_BUFFER_BYTES as u64))
            .map_err(|_| finalize_error(FinalizeFailureKind::Hash))?;
        let read = file
            .read_at(&mut buffer[..requested], offset)
            .map_err(|source| finalize_source(FinalizeFailureKind::Hash, source))?;
        if read == 0 {
            return Err(finalize_error(FinalizeFailureKind::Hash));
        }
        hasher.update(&buffer[..read]);
        offset = offset
            .checked_add(
                u64::try_from(read).map_err(|_| finalize_error(FinalizeFailureKind::Hash))?,
            )
            .ok_or_else(|| finalize_error(FinalizeFailureKind::Hash))?;
    }
    require_finalize_condition(
        !cancellation.is_some_and(|state| state.load(Ordering::Acquire) == CANCELLED),
        FinalizeFailureKind::Cancelled,
    )?;
    let mut extra = [0_u8; 1];
    if file
        .read_at(&mut extra, expected_length)
        .map_err(|source| finalize_source(FinalizeFailureKind::Hash, source))?
        != 0
    {
        return Err(finalize_error(FinalizeFailureKind::Hash));
    }
    Ok(hasher.finalize().into())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn check_cancel(cancellation: &AtomicU8) -> Result<(), ServiceSqliteError> {
    if cancellation.load(Ordering::Acquire) == CANCELLED {
        Err(finalize_error(FinalizeFailureKind::Cancelled))
    } else {
        Ok(())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_finalize_condition(
    condition: bool,
    kind: FinalizeFailureKind,
) -> Result<(), ServiceSqliteError> {
    if condition {
        Ok(())
    } else {
        Err(finalize_error(kind))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn claim_commit_ownership(cancellation: &AtomicU8) -> Result<(), ServiceSqliteError> {
    cancellation
        .compare_exchange(
            CANCELLABLE,
            COMMIT_OWNED,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .map(|_| ())
        .map_err(|_| finalize_error(FinalizeFailureKind::Cancelled))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn test_phase(
    phase: u8,
    cancellation: &AtomicU8,
    cancellable: bool,
) -> Result<(), ServiceSqliteError> {
    #[cfg(test)]
    {
        TEST_FINALIZE_PHASE.store(phase, Ordering::Release);
        while TEST_FINALIZE_BLOCK_PHASE.load(Ordering::Acquire) == phase {
            if cancellable && cancellation.load(Ordering::Acquire) == CANCELLED {
                return Err(finalize_error(FinalizeFailureKind::Cancelled));
            }
            std::thread::yield_now();
        }
    }
    #[cfg(not(test))]
    let _ = (phase, cancellation, cancellable);
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RenameStep {
    RetainLive,
    InstallStage,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct RenameArtifact<'a> {
    source_name: &'static str,
    destination_name: &'static str,
    held: &'a File,
    expected: RestoreArtifactExpectation,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl RenameStep {
    const fn rename_failure(self) -> FinalizeFailureKind {
        match self {
            Self::RetainLive => FinalizeFailureKind::RetainLive,
            Self::InstallStage => FinalizeFailureKind::InstallStage,
        }
    }

    const fn sync_failure(self) -> FinalizeFailureKind {
        match self {
            Self::RetainLive => FinalizeFailureKind::SyncRetained,
            Self::InstallStage => FinalizeFailureKind::SyncInstalled,
        }
    }

    const fn before_rename_failpoint(self) -> crate::failpoint::DurabilityFailpoint {
        match self {
            Self::RetainLive => {
                crate::failpoint::DurabilityFailpoint::RestoreBeforeRetainLiveRename
            }
            Self::InstallStage => {
                crate::failpoint::DurabilityFailpoint::RestoreBeforeInstallStageRename
            }
        }
    }

    const fn after_rename_failpoint(self) -> crate::failpoint::DurabilityFailpoint {
        match self {
            Self::RetainLive => crate::failpoint::DurabilityFailpoint::RestoreAfterRetainLiveRename,
            Self::InstallStage => {
                crate::failpoint::DurabilityFailpoint::RestoreAfterInstallStageRename
            }
        }
    }

    const fn before_sync_failpoint(self) -> crate::failpoint::DurabilityFailpoint {
        match self {
            Self::RetainLive => crate::failpoint::DurabilityFailpoint::RestoreBeforeRetainLiveSync,
            Self::InstallStage => {
                crate::failpoint::DurabilityFailpoint::RestoreBeforeInstallStageSync
            }
        }
    }

    const fn after_sync_failpoint(self) -> crate::failpoint::DurabilityFailpoint {
        match self {
            Self::RetainLive => crate::failpoint::DurabilityFailpoint::RestoreAfterRetainLiveSync,
            Self::InstallStage => {
                crate::failpoint::DurabilityFailpoint::RestoreAfterInstallStageSync
            }
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
trait FinalizeOperations: Send + Sync {
    fn rename(
        &self,
        directory: &File,
        source: &str,
        destination: &str,
        step: RenameStep,
    ) -> std::io::Result<()>;

    fn sync_directory(&self, directory: &File, step: RenameStep) -> std::io::Result<()>;

    fn after_directory_sync(
        &self,
        _directory: &File,
        _step: RenameStep,
    ) -> Result<(), ServiceSqliteError> {
        Ok(())
    }

    #[cfg(test)]
    fn drift_authority_during_marker_sync(&self) -> bool {
        false
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct SystemFinalizeOperations;

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl FinalizeOperations for SystemFinalizeOperations {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn rename(
        &self,
        directory: &File,
        source: &str,
        destination: &str,
        _step: RenameStep,
    ) -> std::io::Result<()> {
        renameat_with(
            directory,
            source,
            directory,
            destination,
            RenameFlags::NOREPLACE,
        )
        .map_err(std::io::Error::from)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn sync_directory(&self, directory: &File, _step: RenameStep) -> std::io::Result<()> {
        directory.sync_all()
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
struct FailingFinalizeOperations;

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
impl FinalizeOperations for FailingFinalizeOperations {
    fn drift_authority_during_marker_sync(&self) -> bool {
        TEST_FINALIZE_FAILURE.load(Ordering::Acquire) == 9
    }

    fn rename(
        &self,
        directory: &File,
        source: &str,
        destination: &str,
        step: RenameStep,
    ) -> std::io::Result<()> {
        let failure = TEST_FINALIZE_FAILURE.load(Ordering::Acquire);
        let (before, after) = match step {
            RenameStep::RetainLive => (1, 2),
            RenameStep::InstallStage => (5, 6),
        };
        if failure == before {
            return Err(std::io::Error::other("injected pre-rename failure"));
        }
        SystemFinalizeOperations.rename(directory, source, destination, step)?;
        if failure == after {
            return Err(std::io::Error::other("injected post-rename failure"));
        }
        Ok(())
    }

    fn sync_directory(&self, directory: &File, step: RenameStep) -> std::io::Result<()> {
        let failure = TEST_FINALIZE_FAILURE.load(Ordering::Acquire);
        let target = match step {
            RenameStep::RetainLive => 3,
            RenameStep::InstallStage => 7,
        };
        if failure == target {
            return Err(std::io::Error::other("injected directory sync failure"));
        }
        SystemFinalizeOperations.sync_directory(directory, step)
    }

    fn after_directory_sync(
        &self,
        directory: &File,
        step: RenameStep,
    ) -> Result<(), ServiceSqliteError> {
        let failure = TEST_FINALIZE_FAILURE.load(Ordering::Acquire);
        let target = match step {
            RenameStep::RetainLive => 4,
            RenameStep::InstallStage => 8,
        };
        if failure != target {
            return Ok(());
        }
        openat(
            directory,
            MARKER_NEXT_FILE_NAME,
            OFlags::RDWR
                | OFlags::CREATE
                | OFlags::EXCL
                | OFlags::NOFOLLOW
                | OFlags::CLOEXEC
                | OFlags::NONBLOCK,
            Mode::RUSR | Mode::WUSR,
        )
        .map(drop)
        .map_err(|source| finalize_source(FinalizeFailureKind::Marker, source))
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
pub(crate) async fn test_finalize_with_failure(
    staged: StagedServiceRestore,
    failure: u8,
) -> Result<(), ServiceSqliteError> {
    TEST_FINALIZE_FAILURE.store(failure, Ordering::Release);
    let native = staged.into_native();
    let result = tokio::task::spawn_blocking(move || {
        finalize_native(
            native,
            &AtomicU8::new(CANCELLABLE),
            &FailingFinalizeOperations,
            &crate::failpoint::DurabilityFailpoints::default(),
        )
    })
    .await
    .map_err(|source| finalize_source(FinalizeFailureKind::Join, source))?;
    TEST_FINALIZE_FAILURE.store(0, Ordering::Release);
    result
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
pub(crate) async fn test_finalize_with_failpoint(
    staged: StagedServiceRestore,
    failpoints: crate::failpoint::DurabilityFailpoints,
) -> Result<(), ServiceSqliteError> {
    let native = staged.into_native();
    tokio::task::spawn_blocking(move || {
        finalize_native(
            native,
            &AtomicU8::new(CANCELLABLE),
            &SystemFinalizeOperations,
            &failpoints,
        )
    })
    .await
    .map_err(|source| finalize_source(FinalizeFailureKind::Join, source))?
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
pub(crate) fn reset_test_controls() {
    TEST_FINALIZE_PHASE.store(0, Ordering::Release);
    TEST_FINALIZE_BLOCK_PHASE.store(0, Ordering::Release);
    TEST_FINALIZE_FAILURE.store(0, Ordering::Release);
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct CancellationOnDrop {
    cancellation: Arc<AtomicU8>,
    armed: AtomicBool,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl CancellationOnDrop {
    fn new(cancellation: Arc<AtomicU8>) -> Self {
        Self {
            cancellation,
            armed: AtomicBool::new(true),
        }
    }

    fn disarm(&self) {
        self.armed.store(false, Ordering::Release);
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for CancellationOnDrop {
    fn drop(&mut self) {
        if self.armed.load(Ordering::Acquire) {
            let _ = self.cancellation.compare_exchange(
                CANCELLABLE,
                CANCELLED,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FinalizeFailureKind {
    Live,
    Artifact,
    Hash,
    SyncLive,
    SyncStaged,
    Marker,
    RetainLive,
    SyncRetained,
    InstallStage,
    SyncInstalled,
    Cancelled,
    Join,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct FinalizeFailure {
    kind: FinalizeFailureKind,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl fmt::Debug for FinalizeFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FinalizeFailure")
            .field("kind", &self.kind)
            .field("source", &self.source.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl fmt::Display for FinalizeFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            FinalizeFailureKind::Live => "live restore source is invalid",
            FinalizeFailureKind::Artifact => "restore artifact binding changed",
            FinalizeFailureKind::Hash => "restore artifact hash failed",
            FinalizeFailureKind::SyncLive => "live restore source sync failed",
            FinalizeFailureKind::SyncStaged => "staged restore sync failed",
            FinalizeFailureKind::Marker => "restore marker transition failed",
            FinalizeFailureKind::RetainLive => "live restore retention failed",
            FinalizeFailureKind::SyncRetained => "retained restore sync failed",
            FinalizeFailureKind::InstallStage => "restore installation failed",
            FinalizeFailureKind::SyncInstalled => "installed restore sync failed",
            FinalizeFailureKind::Cancelled => "restore finalization was cancelled",
            FinalizeFailureKind::Join => "restore finalization worker failed",
        })
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Error for FinalizeFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn finalize_error(kind: FinalizeFailureKind) -> ServiceSqliteError {
    ServiceSqliteError::with_source(
        ServiceSqliteErrorKind::Restore,
        FinalizeFailure { kind, source: None },
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn finalize_source(
    kind: FinalizeFailureKind,
    source: impl Error + Send + Sync + 'static,
) -> ServiceSqliteError {
    ServiceSqliteError::with_source(
        ServiceSqliteErrorKind::Restore,
        FinalizeFailure {
            kind,
            source: Some(Box::new(source)),
        },
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn hit(
    failpoints: &crate::failpoint::DurabilityFailpoints,
    point: crate::failpoint::DurabilityFailpoint,
    kind: FinalizeFailureKind,
) -> Result<(), ServiceSqliteError> {
    failpoints
        .hit(point)
        .map_err(|source| finalize_source(kind, source))
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use std::io::Write;

    use sha2::{Digest, Sha256};

    use super::*;

    #[test]
    fn finalize_failure_inventory_is_complete_and_source_aware() {
        let cases = [
            (FinalizeFailureKind::Live, "live restore source is invalid"),
            (
                FinalizeFailureKind::Artifact,
                "restore artifact binding changed",
            ),
            (FinalizeFailureKind::Hash, "restore artifact hash failed"),
            (
                FinalizeFailureKind::SyncLive,
                "live restore source sync failed",
            ),
            (
                FinalizeFailureKind::SyncStaged,
                "staged restore sync failed",
            ),
            (
                FinalizeFailureKind::Marker,
                "restore marker transition failed",
            ),
            (
                FinalizeFailureKind::RetainLive,
                "live restore retention failed",
            ),
            (
                FinalizeFailureKind::SyncRetained,
                "retained restore sync failed",
            ),
            (
                FinalizeFailureKind::InstallStage,
                "restore installation failed",
            ),
            (
                FinalizeFailureKind::SyncInstalled,
                "installed restore sync failed",
            ),
            (
                FinalizeFailureKind::Cancelled,
                "restore finalization was cancelled",
            ),
            (
                FinalizeFailureKind::Join,
                "restore finalization worker failed",
            ),
        ];
        for (kind, message) in cases {
            let plain = FinalizeFailure { kind, source: None };
            assert_eq!(plain.to_string(), message);
            assert!(plain.source().is_none());
            let sourced = FinalizeFailure {
                kind,
                source: Some(Box::new(std::io::Error::other("private-cause"))),
            };
            assert_eq!(sourced.to_string(), message);
            assert!(sourced.source().is_some());
            assert!(format!("{sourced:?}").contains("[redacted]"));
            assert!(require_finalize_condition(true, kind).is_ok());
            assert_eq!(
                require_finalize_condition(false, kind)
                    .expect_err("false condition")
                    .kind(),
                ServiceSqliteErrorKind::Restore
            );
        }
    }

    #[test]
    fn hash_and_cancellation_boundaries_fail_closed() {
        let root = tempfile::tempdir().expect("root");
        let path = root.path().join("artifact.sqlite");
        let payload = b"finalize-artifact";
        std::fs::write(&path, payload).expect("artifact");
        let file = File::open(&path).expect("open artifact");
        let length = u64::try_from(payload.len()).expect("length");
        let digest: [u8; 32] = Sha256::digest(payload).into();
        assert_eq!(hash_exact(&file, length, None).expect("exact hash"), digest);
        assert_eq!(
            hash_exact(&file, length + 1, None)
                .expect_err("short artifact")
                .kind(),
            ServiceSqliteErrorKind::Restore
        );
        assert_eq!(
            hash_exact(&file, length - 1, None)
                .expect_err("long artifact")
                .kind(),
            ServiceSqliteErrorKind::Restore
        );

        let cancelled = AtomicU8::new(CANCELLED);
        assert_eq!(
            hash_exact(&file, length, Some(&cancelled))
                .expect_err("pre-read cancellation")
                .kind(),
            ServiceSqliteErrorKind::Restore
        );
        assert_eq!(
            check_cancel(&cancelled).expect_err("cancelled").kind(),
            ServiceSqliteErrorKind::Restore
        );
        assert_eq!(
            claim_commit_ownership(&cancelled)
                .expect_err("cancelled ownership handoff")
                .kind(),
            ServiceSqliteErrorKind::Restore
        );
        let active = AtomicU8::new(CANCELLABLE);
        check_cancel(&active).expect("active");
        claim_commit_ownership(&active).expect("commit ownership");
        assert_eq!(active.load(Ordering::Acquire), COMMIT_OWNED);

        let empty_path = root.path().join("empty.sqlite");
        let mut empty = File::create(&empty_path).expect("empty artifact");
        empty.flush().expect("flush empty artifact");
        let empty = File::open(empty_path).expect("open empty artifact");
        assert_eq!(
            hash_exact(&empty, 1, None)
                .expect_err("zero-byte read")
                .kind(),
            ServiceSqliteErrorKind::Restore
        );
    }
}

//! Deterministic writer-authorized reconciliation of interrupted restores.

#[cfg(any(target_os = "linux", target_os = "macos"))]
use core::fmt;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use {
    super::{
        RestoreArtifactExpectation, RestoreMarkerBinding, RestoreRecoveryPhase,
        marker::{
            BACKUP_FILE_NAME, LIVE_FILE_NAME, MARKER_FILE_NAME, MARKER_NEXT_FILE_NAME,
            STAGED_FILE_NAME,
        },
    },
    crate::{
        ServiceDatabaseIdentity, ServiceSqliteError, ServiceSqliteErrorKind, ServiceSqlitePaths,
        WriterAuthority,
    },
    rustix::{
        fs::{
            AtFlags, FileType, Mode, OFlags, RenameFlags, fstat, openat, renameat_with, statat,
            unlinkat,
        },
        io::Errno,
        process::geteuid,
    },
    sha2::{Digest, Sha256},
    std::{error::Error, fs::File, os::unix::fs::FileExt},
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
const HASH_BUFFER_BYTES: usize = 64 * 1_024;

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn recover_for_open(
    paths: &ServiceSqlitePaths,
    identity: &ServiceDatabaseIdentity,
    authority: &WriterAuthority,
) -> Result<(), ServiceSqliteError> {
    authority.validate_for(paths)?;
    let Some(mut marker) = RestoreMarkerBinding::load_for_recovery(paths, authority)? else {
        authority_checked(authority, paths, || {
            refuse_unresolved_recovery(authority.directory())
        })?;
        return Ok(());
    };
    if !marker.marker().matches_identity(identity) {
        return Err(recovery_error(RecoveryFailureKind::Intent));
    }

    let observed = authority_checked(authority, paths, || {
        observe_artifacts(authority.directory(), marker.marker())
    })?;
    if let Some(next) = marker.interrupted_transition(paths, authority)? {
        let topology_matches = match (marker.marker().phase(), next) {
            (RestoreRecoveryPhase::Prepared, RestoreRecoveryPhase::LiveRetained) => {
                observed.proves_live_retained()
            }
            (RestoreRecoveryPhase::LiveRetained, RestoreRecoveryPhase::ReplacementInstalled) => {
                observed.proves_replacement_installed()
            }
            _ => false,
        };
        if !topology_matches {
            return Err(recovery_error(RecoveryFailureKind::Topology));
        }
        marker = marker.promote_interrupted_transition(paths, authority, next)?;
    }

    loop {
        let observed = authority_checked(authority, paths, || {
            observe_artifacts(authority.directory(), marker.marker())
        })?;
        match marker.marker().phase() {
            RestoreRecoveryPhase::Prepared if observed.can_roll_back_prepared() => {
                if let Some(staged) = observed.staged.as_ref() {
                    authority_checked(authority, paths, || {
                        remove_exact_artifact(
                            authority.directory(),
                            STAGED_FILE_NAME,
                            staged,
                            marker.marker().staged(),
                        )
                    })?;
                }
                marker.retire(paths, authority)?;
                authority_checked(authority, paths, || {
                    refuse_unresolved_recovery(authority.directory())
                })?;
                return Ok(());
            }
            RestoreRecoveryPhase::Prepared if observed.proves_live_retained() => {
                authority_checked(authority, paths, || {
                    authority.directory().sync_all().map_err(|source| {
                        recovery_source(RecoveryFailureKind::DirectorySync, source)
                    })
                })?;
                marker = marker.advance_for_recovery(
                    paths,
                    authority,
                    RestoreRecoveryPhase::LiveRetained,
                )?;
            }
            RestoreRecoveryPhase::LiveRetained if observed.needs_replacement_install() => {
                let staged = observed
                    .staged
                    .as_ref()
                    .ok_or_else(|| recovery_error(RecoveryFailureKind::Topology))?;
                authority_checked(authority, paths, || {
                    verify_named_artifact(
                        authority.directory(),
                        STAGED_FILE_NAME,
                        staged,
                        marker.marker().staged(),
                    )
                })?;
                authority_checked(authority, paths, || {
                    renameat_with(
                        authority.directory(),
                        STAGED_FILE_NAME,
                        authority.directory(),
                        LIVE_FILE_NAME,
                        RenameFlags::NOREPLACE,
                    )
                    .map_err(|source| {
                        recovery_source(RecoveryFailureKind::InstallReplacement, source)
                    })
                })?;
                authority_checked(authority, paths, || {
                    verify_named_artifact(
                        authority.directory(),
                        LIVE_FILE_NAME,
                        staged,
                        marker.marker().staged(),
                    )
                })?;
                authority_checked(authority, paths, || {
                    authority.directory().sync_all().map_err(|source| {
                        recovery_source(RecoveryFailureKind::DirectorySync, source)
                    })
                })?;
                marker = marker.advance_for_recovery(
                    paths,
                    authority,
                    RestoreRecoveryPhase::ReplacementInstalled,
                )?;
            }
            RestoreRecoveryPhase::LiveRetained if observed.proves_replacement_installed() => {
                authority_checked(authority, paths, || {
                    authority.directory().sync_all().map_err(|source| {
                        recovery_source(RecoveryFailureKind::DirectorySync, source)
                    })
                })?;
                marker = marker.advance_for_recovery(
                    paths,
                    authority,
                    RestoreRecoveryPhase::ReplacementInstalled,
                )?;
            }
            RestoreRecoveryPhase::ReplacementInstalled
                if observed.proves_replacement_installed_or_cleanup() =>
            {
                let live = match observed.live {
                    LiveArtifact::Replacement(ref file) => file,
                    _ => return Err(recovery_error(RecoveryFailureKind::Topology)),
                };
                authority_checked(authority, paths, || {
                    verify_named_artifact(
                        authority.directory(),
                        LIVE_FILE_NAME,
                        live,
                        observed.marker_staged,
                    )
                })?;
                if let Some(backup) = observed.backup.as_ref() {
                    authority_checked(authority, paths, || {
                        remove_exact_artifact(
                            authority.directory(),
                            BACKUP_FILE_NAME,
                            backup,
                            observed.marker_live,
                        )
                    })?;
                }
                marker.retire(paths, authority)?;
                authority_checked(authority, paths, || {
                    refuse_unresolved_recovery(authority.directory())
                })?;
                authority_checked(authority, paths, || {
                    verify_named_artifact(
                        authority.directory(),
                        LIVE_FILE_NAME,
                        live,
                        observed.marker_staged,
                    )
                })?;
                return Ok(());
            }
            _ => return Err(recovery_error(RecoveryFailureKind::Topology)),
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn refuse_unresolved_recovery(
    directory: &impl std::os::fd::AsFd,
) -> Result<(), ServiceSqliteError> {
    for name in [
        STAGED_FILE_NAME,
        BACKUP_FILE_NAME,
        MARKER_FILE_NAME,
        MARKER_NEXT_FILE_NAME,
    ] {
        match statat(directory, name, AtFlags::SYMLINK_NOFOLLOW) {
            Err(Errno::NOENT) => {}
            Ok(_) | Err(_) => {
                return Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Recovery));
            }
        }
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct ObservedArtifacts {
    live: LiveArtifact,
    staged: Option<File>,
    backup: Option<File>,
    marker_live: RestoreArtifactExpectation,
    marker_staged: RestoreArtifactExpectation,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl ObservedArtifacts {
    fn can_roll_back_prepared(&self) -> bool {
        matches!(self.live, LiveArtifact::Original) && self.backup.is_none()
    }

    fn proves_live_retained(&self) -> bool {
        matches!(self.live, LiveArtifact::Absent) && self.staged.is_some() && self.backup.is_some()
    }

    fn needs_replacement_install(&self) -> bool {
        self.proves_live_retained()
    }

    fn proves_replacement_installed(&self) -> bool {
        matches!(self.live, LiveArtifact::Replacement(_))
            && self.staged.is_none()
            && self.backup.is_some()
    }

    fn proves_replacement_installed_or_cleanup(&self) -> bool {
        matches!(self.live, LiveArtifact::Replacement(_)) && self.staged.is_none()
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
enum LiveArtifact {
    Absent,
    Original,
    Replacement(File),
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn observe_artifacts(
    directory: &File,
    marker: &super::RestoreRecoveryMarker,
) -> Result<ObservedArtifacts, ServiceSqliteError> {
    for sidecar in [
        "state.sqlite-wal",
        "state.sqlite-shm",
        "state.sqlite-journal",
    ] {
        require_absent(directory, sidecar)?;
    }
    let live = match open_optional(directory, LIVE_FILE_NAME)? {
        None => LiveArtifact::Absent,
        Some(file) if artifact_has_identity(&file, marker.live())? => {
            verify_artifact(&file, marker.live())?;
            LiveArtifact::Original
        }
        Some(file) => {
            require_recovery_condition(
                artifact_has_identity(&file, marker.staged())?,
                RecoveryFailureKind::Artifact,
            )?;
            verify_artifact(&file, marker.staged())?;
            LiveArtifact::Replacement(file)
        }
    };
    let staged = observe_expected(directory, STAGED_FILE_NAME, marker.staged())?;
    let backup = observe_expected(directory, BACKUP_FILE_NAME, marker.backup())?;
    Ok(ObservedArtifacts {
        live,
        staged,
        backup,
        marker_live: marker.live(),
        marker_staged: marker.staged(),
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn observe_expected(
    directory: &File,
    name: &str,
    expected: RestoreArtifactExpectation,
) -> Result<Option<File>, ServiceSqliteError> {
    let Some(file) = open_optional(directory, name)? else {
        return Ok(None);
    };
    verify_artifact(&file, expected)?;
    Ok(Some(file))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn open_optional(directory: &File, name: &str) -> Result<Option<File>, ServiceSqliteError> {
    match openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    ) {
        Ok(file) => Ok(Some(File::from(file))),
        Err(Errno::NOENT) => Ok(None),
        Err(source) => Err(recovery_source(RecoveryFailureKind::Artifact, source)),
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn artifact_has_identity(
    file: &File,
    expected: RestoreArtifactExpectation,
) -> Result<bool, ServiceSqliteError> {
    let status =
        fstat(file).map_err(|source| recovery_source(RecoveryFailureKind::Artifact, source))?;
    let device = crate::native_metadata::device(status.st_dev)
        .map_err(|_| recovery_error(RecoveryFailureKind::Artifact))?;
    Ok((device, status.st_ino) == (expected.device(), expected.inode()))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn verify_artifact(
    file: &File,
    expected: RestoreArtifactExpectation,
) -> Result<(), ServiceSqliteError> {
    let status =
        fstat(file).map_err(|source| recovery_source(RecoveryFailureKind::Artifact, source))?;
    let device = crate::native_metadata::device(status.st_dev)
        .map_err(|_| recovery_error(RecoveryFailureKind::Artifact))?;
    let length =
        u64::try_from(status.st_size).map_err(|_| recovery_error(RecoveryFailureKind::Artifact))?;
    require_recovery_condition(
        crate::all_constraints([
            crate::native_metadata::exact_regular_file(
                FileType::from_raw_mode(status.st_mode).is_file(),
                crate::native_metadata::link_count(status.st_nlink),
                status.st_uid,
                geteuid().as_raw(),
                crate::native_metadata::mode(status.st_mode),
            ),
            (device, status.st_ino) == (expected.device(), expected.inode()),
            crate::native_metadata::valid_artifact_length(length, Some(expected.byte_length())),
        ]),
        RecoveryFailureKind::Artifact,
    )?;
    require_recovery_condition(
        hash_exact(file, expected.byte_length())? == expected.sha256(),
        RecoveryFailureKind::Artifact,
    )?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn verify_named_artifact(
    directory: &File,
    name: &str,
    held: &File,
    expected: RestoreArtifactExpectation,
) -> Result<(), ServiceSqliteError> {
    verify_artifact(held, expected)?;
    let current = open_optional(directory, name)?
        .ok_or_else(|| recovery_error(RecoveryFailureKind::Artifact))?;
    verify_artifact(&current, expected)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn remove_exact_artifact(
    directory: &File,
    name: &str,
    held: &File,
    expected: RestoreArtifactExpectation,
) -> Result<(), ServiceSqliteError> {
    verify_named_artifact(directory, name, held, expected)?;
    unlinkat(directory, name, AtFlags::empty())
        .map_err(|source| recovery_source(RecoveryFailureKind::Cleanup, source))?;
    directory
        .sync_all()
        .map_err(|source| recovery_source(RecoveryFailureKind::DirectorySync, source))?;
    require_absent(directory, name)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_absent(directory: &File, name: &str) -> Result<(), ServiceSqliteError> {
    match statat(directory, name, AtFlags::SYMLINK_NOFOLLOW) {
        Err(Errno::NOENT) => Ok(()),
        Ok(_) | Err(_) => Err(recovery_error(RecoveryFailureKind::Topology)),
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_recovery_condition(
    condition: bool,
    kind: RecoveryFailureKind,
) -> Result<(), ServiceSqliteError> {
    if condition {
        Ok(())
    } else {
        Err(recovery_error(kind))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn hash_exact(file: &File, expected_length: u64) -> Result<[u8; 32], ServiceSqliteError> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    let mut offset = 0_u64;
    while offset < expected_length {
        let requested = usize::try_from((expected_length - offset).min(HASH_BUFFER_BYTES as u64))
            .map_err(|_| recovery_error(RecoveryFailureKind::Hash))?;
        let read = file
            .read_at(&mut buffer[..requested], offset)
            .map_err(|source| recovery_source(RecoveryFailureKind::Hash, source))?;
        require_recovery_condition(read != 0, RecoveryFailureKind::Hash)?;
        hasher.update(&buffer[..read]);
        offset = offset
            .checked_add(
                u64::try_from(read).map_err(|_| recovery_error(RecoveryFailureKind::Hash))?,
            )
            .ok_or_else(|| recovery_error(RecoveryFailureKind::Hash))?;
    }
    let mut extra = [0_u8; 1];
    require_recovery_condition(
        file.read_at(&mut extra, expected_length)
            .map_err(|source| recovery_source(RecoveryFailureKind::Hash, source))?
            == 0,
        RecoveryFailureKind::Hash,
    )?;
    Ok(hasher.finalize().into())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn authority_checked<T>(
    authority: &WriterAuthority,
    paths: &ServiceSqlitePaths,
    operation: impl FnOnce() -> Result<T, ServiceSqliteError>,
) -> Result<T, ServiceSqliteError> {
    authority.validate_for(paths)?;
    let result = operation();
    authority.validate_for(paths)?;
    result
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecoveryFailureKind {
    Intent,
    Topology,
    Artifact,
    Hash,
    InstallReplacement,
    DirectorySync,
    Cleanup,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct RecoveryFailure {
    kind: RecoveryFailureKind,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl fmt::Debug for RecoveryFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecoveryFailure")
            .field("kind", &self.kind)
            .field("source", &self.source.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl fmt::Display for RecoveryFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            RecoveryFailureKind::Intent => "restore recovery intent does not match",
            RecoveryFailureKind::Topology => "restore recovery topology is ambiguous",
            RecoveryFailureKind::Artifact => "restore recovery artifact is invalid",
            RecoveryFailureKind::Hash => "restore recovery artifact hash failed",
            RecoveryFailureKind::InstallReplacement => {
                "restore recovery replacement installation failed"
            }
            RecoveryFailureKind::DirectorySync => "restore recovery directory durability failed",
            RecoveryFailureKind::Cleanup => "restore recovery cleanup failed",
        })
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Error for RecoveryFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn recovery_error(kind: RecoveryFailureKind) -> ServiceSqliteError {
    ServiceSqliteError::with_source(
        ServiceSqliteErrorKind::Recovery,
        RecoveryFailure { kind, source: None },
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn recovery_source(
    kind: RecoveryFailureKind,
    source: impl Error + Send + Sync + 'static,
) -> ServiceSqliteError {
    ServiceSqliteError::with_source(
        ServiceSqliteErrorKind::Recovery,
        RecoveryFailure {
            kind,
            source: Some(Box::new(source)),
        },
    )
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use std::{
        fs::{self, File, OpenOptions},
        io::Write,
        num::NonZeroU32,
        os::unix::fs::{FileTypeExt, OpenOptionsExt, PermissionsExt},
        path::{Path, PathBuf},
        process::Command,
    };

    use radroots_runtime_paths::{
        InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource, ServiceId,
    };
    use radroots_storage::event::SourceGeneration;

    use super::*;
    use crate::restore::RestoreRecoveryMarker;
    use crate::{
        BackupManifestSha256, OpenMode, ServiceDatabaseMetadata, ServiceSqliteApplicationId,
    };

    const OLD_BYTES: &[u8] = b"old-live-state";
    const NEW_BYTES: &[u8] = b"new-restored-state";

    #[test]
    fn recovery_failure_inventory_is_complete_and_source_aware() {
        let cases = [
            (
                RecoveryFailureKind::Intent,
                "restore recovery intent does not match",
            ),
            (
                RecoveryFailureKind::Topology,
                "restore recovery topology is ambiguous",
            ),
            (
                RecoveryFailureKind::Artifact,
                "restore recovery artifact is invalid",
            ),
            (
                RecoveryFailureKind::Hash,
                "restore recovery artifact hash failed",
            ),
            (
                RecoveryFailureKind::InstallReplacement,
                "restore recovery replacement installation failed",
            ),
            (
                RecoveryFailureKind::DirectorySync,
                "restore recovery directory durability failed",
            ),
            (
                RecoveryFailureKind::Cleanup,
                "restore recovery cleanup failed",
            ),
        ];
        for (kind, message) in cases {
            let plain = RecoveryFailure { kind, source: None };
            assert_eq!(plain.to_string(), message);
            assert!(plain.source().is_none());
            let sourced = RecoveryFailure {
                kind,
                source: Some(Box::new(std::io::Error::other("private-cause"))),
            };
            assert_eq!(sourced.to_string(), message);
            assert!(sourced.source().is_some());
            assert!(format!("{sourced:?}").contains("[redacted]"));
            assert!(require_recovery_condition(true, kind).is_ok());
            assert_eq!(
                require_recovery_condition(false, kind)
                    .expect_err("false condition")
                    .kind(),
                ServiceSqliteErrorKind::Recovery
            );
        }
    }

    #[test]
    fn observed_artifact_topology_predicates_cover_every_boolean_combination() {
        fn observed(live: u8, staged: bool, backup: bool) -> ObservedArtifacts {
            let live = match live {
                0 => LiveArtifact::Absent,
                1 => LiveArtifact::Original,
                2 => LiveArtifact::Replacement(File::open("/dev/null").expect("replacement")),
                _ => unreachable!("test topology is closed"),
            };
            let artifact =
                RestoreArtifactExpectation::new(1, 2, 1, [3; 32]).expect("artifact expectation");
            ObservedArtifacts {
                live,
                staged: staged.then(|| File::open("/dev/null").expect("staged")),
                backup: backup.then(|| File::open("/dev/null").expect("backup")),
                marker_live: artifact,
                marker_staged: artifact,
            }
        }

        for live in 0..=2 {
            for staged in [false, true] {
                for backup in [false, true] {
                    let observed = observed(live, staged, backup);
                    assert_eq!(observed.can_roll_back_prepared(), live == 1 && !backup);
                    assert_eq!(
                        observed.proves_live_retained(),
                        live == 0 && staged && backup
                    );
                    assert_eq!(
                        observed.needs_replacement_install(),
                        live == 0 && staged && backup
                    );
                    assert_eq!(
                        observed.proves_replacement_installed(),
                        live == 2 && !staged && backup
                    );
                    assert_eq!(
                        observed.proves_replacement_installed_or_cleanup(),
                        live == 2 && !staged
                    );
                }
            }
        }
    }

    struct Fixture {
        _root: tempfile::TempDir,
        paths: ServiceSqlitePaths,
        identity: ServiceDatabaseIdentity,
        authority: WriterAuthority,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().expect("temporary root");
            let paths = service_paths(root.path());
            let state_directory = paths.state_database().parent().expect("state directory");
            fs::create_dir_all(state_directory).expect("create state directory");
            fs::set_permissions(state_directory, fs::Permissions::from_mode(0o700))
                .expect("restrict state directory");
            write_new(paths.state_database(), OLD_BYTES);
            write_new(&artifact_path(&paths, STAGED_FILE_NAME), NEW_BYTES);
            let authority = WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting)
                .expect("writer authority")
                .expect("writable mode retains authority");
            let metadata = database_metadata(&paths);
            let marker = RestoreRecoveryMarker::prepared(
                &metadata,
                BackupManifestSha256::from_bytes([19; 32]),
                expectation(paths.state_database()),
                expectation(&artifact_path(&paths, STAGED_FILE_NAME)),
            )
            .expect("prepared marker");
            RestoreMarkerBinding::create(&paths, &authority, &marker)
                .expect("persist prepared marker");
            Self {
                _root: root,
                paths,
                identity: metadata.identity(),
                authority,
            }
        }

        fn load_marker(&self) -> RestoreMarkerBinding {
            RestoreMarkerBinding::load_for_recovery(&self.paths, &self.authority)
                .expect("load marker")
                .expect("marker exists")
        }

        fn retain_live(&self, advance: bool) {
            fs::rename(
                self.paths.state_database(),
                artifact_path(&self.paths, BACKUP_FILE_NAME),
            )
            .expect("retain live");
            sync_state_directory(&self.paths);
            if advance {
                self.load_marker()
                    .advance(
                        &self.paths,
                        &self.authority,
                        RestoreRecoveryPhase::LiveRetained,
                    )
                    .expect("advance live-retained marker");
            }
        }

        fn install_stage(&self, advance: bool) {
            fs::rename(
                artifact_path(&self.paths, STAGED_FILE_NAME),
                self.paths.state_database(),
            )
            .expect("install stage");
            sync_state_directory(&self.paths);
            if advance {
                self.load_marker()
                    .advance(
                        &self.paths,
                        &self.authority,
                        RestoreRecoveryPhase::ReplacementInstalled,
                    )
                    .expect("advance replacement marker");
            }
        }

        fn write_scratch(&self, next: RestoreRecoveryPhase) {
            let marker = self.load_marker();
            let next = marker
                .marker()
                .transitioned_to(next)
                .expect("legal next phase");
            write_new(
                &artifact_path(&self.paths, MARKER_NEXT_FILE_NAME),
                next.canonical_bytes(),
            );
            sync_state_directory(&self.paths);
        }

        fn recover(&self) -> Result<(), ServiceSqliteError> {
            recover_for_open(&self.paths, &self.identity, &self.authority)
        }
    }

    #[test]
    fn prepared_with_old_live_rolls_back_and_retries_idempotently() {
        let fixture = Fixture::new();
        fixture.recover().expect("rollback prepared restore");
        assert_eq!(fs::read(fixture.paths.state_database()).unwrap(), OLD_BYTES);
        assert_no_recovery_evidence(&fixture.paths);
        fixture.recover().expect("repeated recovery is a no-op");
    }

    #[test]
    fn interrupted_prepared_rollback_without_stage_finishes_marker_cleanup() {
        let fixture = Fixture::new();
        fs::remove_file(artifact_path(&fixture.paths, STAGED_FILE_NAME)).expect("remove stage");
        sync_state_directory(&fixture.paths);
        fixture.recover().expect("finish interrupted rollback");
        assert_eq!(fs::read(fixture.paths.state_database()).unwrap(), OLD_BYTES);
        assert_no_recovery_evidence(&fixture.paths);
    }

    #[test]
    fn prepared_with_proven_first_rename_rolls_forward() {
        let fixture = Fixture::new();
        fixture.retain_live(false);
        fixture.recover().expect("recover after first rename");
        assert_eq!(fs::read(fixture.paths.state_database()).unwrap(), NEW_BYTES);
        assert_no_recovery_evidence(&fixture.paths);
    }

    #[test]
    fn live_retained_with_proven_second_rename_rolls_forward() {
        let fixture = Fixture::new();
        fixture.retain_live(true);
        fixture.install_stage(false);
        fixture.recover().expect("recover after second rename");
        assert_eq!(fs::read(fixture.paths.state_database()).unwrap(), NEW_BYTES);
        assert_no_recovery_evidence(&fixture.paths);
    }

    #[test]
    fn replacement_installed_retires_backup_and_marker() {
        let fixture = Fixture::new();
        fixture.retain_live(true);
        fixture.install_stage(true);
        fixture.recover().expect("retire completed recovery");
        assert_eq!(fs::read(fixture.paths.state_database()).unwrap(), NEW_BYTES);
        assert_no_recovery_evidence(&fixture.paths);
    }

    #[test]
    fn replacement_cleanup_without_backup_finishes_idempotently() {
        let fixture = Fixture::new();
        fixture.retain_live(true);
        fixture.install_stage(true);
        fs::remove_file(artifact_path(&fixture.paths, BACKUP_FILE_NAME)).expect("remove backup");
        sync_state_directory(&fixture.paths);
        fixture.recover().expect("finish interrupted cleanup");
        assert_eq!(fs::read(fixture.paths.state_database()).unwrap(), NEW_BYTES);
        assert_no_recovery_evidence(&fixture.paths);
        fixture.recover().expect("repeated cleanup is a no-op");
    }

    #[test]
    fn topology_consistent_marker_scratch_is_promoted_at_both_edges() {
        let first = Fixture::new();
        first.retain_live(false);
        first.write_scratch(RestoreRecoveryPhase::LiveRetained);
        first.recover().expect("promote first scratch");
        assert_eq!(fs::read(first.paths.state_database()).unwrap(), NEW_BYTES);
        assert_no_recovery_evidence(&first.paths);

        let second = Fixture::new();
        second.retain_live(true);
        second.install_stage(false);
        second.write_scratch(RestoreRecoveryPhase::ReplacementInstalled);
        second.recover().expect("promote second scratch");
        assert_eq!(fs::read(second.paths.state_database()).unwrap(), NEW_BYTES);
        assert_no_recovery_evidence(&second.paths);
    }

    #[test]
    fn inferred_advance_failures_are_recovery_and_authority_keeps_precedence() {
        let recovery = Fixture::new();
        let error = recovery
            .load_marker()
            .test_advance_for_recovery_with_failure(
                &recovery.paths,
                &recovery.authority,
                RestoreRecoveryPhase::LiveRetained,
                crate::restore::marker::TestStoreFailure::ScratchSync,
            )
            .expect_err("recovery marker sync failure must be classified");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Recovery);
        assert!(artifact_path(&recovery.paths, MARKER_FILE_NAME).exists());
        assert!(!artifact_path(&recovery.paths, MARKER_NEXT_FILE_NAME).exists());

        let authority = Fixture::new();
        let state_directory = authority.paths.state_database().parent().unwrap();
        let error = authority
            .load_marker()
            .test_advance_for_recovery_with_failure(
                &authority.paths,
                &authority.authority,
                RestoreRecoveryPhase::LiveRetained,
                crate::restore::marker::TestStoreFailure::AuthorityDriftAndScratchSync,
            )
            .expect_err("authority drift must dominate the marker failure");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Authority);
        assert!(artifact_path(&authority.paths, MARKER_FILE_NAME).exists());
        fs::set_permissions(state_directory, fs::Permissions::from_mode(0o700))
            .expect("restore state directory mode");
    }

    #[test]
    fn replaced_interrupted_scratch_never_overwrites_the_valid_marker() {
        let fixture = Fixture::new();
        fixture.retain_live(false);
        fixture.write_scratch(RestoreRecoveryPhase::LiveRetained);
        let marker = fixture.load_marker();
        let scratch = artifact_path(&fixture.paths, MARKER_NEXT_FILE_NAME);
        let retained = scratch.with_file_name("retained-valid-marker-next");
        let replacement_bytes = b"foreign-marker-next";
        let scratch_for_hook = scratch.clone();
        let error = marker
            .test_promote_interrupted_transition_after_hook(
                &fixture.paths,
                &fixture.authority,
                RestoreRecoveryPhase::LiveRetained,
                move || {
                    fs::rename(&scratch_for_hook, &retained).expect("retain exact scratch");
                    write_new(&scratch_for_hook, replacement_bytes);
                },
            )
            .expect_err("replaced scratch must fail without marker replacement");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Recovery);
        assert_eq!(fs::read(&scratch).unwrap(), replacement_bytes);
        assert!(
            scratch
                .with_file_name("retained-valid-marker-next")
                .exists()
        );
        let current = fixture.load_marker();
        assert_eq!(current.marker().phase(), RestoreRecoveryPhase::Prepared);
    }

    #[test]
    fn topology_inconsistent_scratch_and_tampered_artifacts_fail_without_cleanup() {
        let scratch = Fixture::new();
        scratch.write_scratch(RestoreRecoveryPhase::LiveRetained);
        let error = scratch
            .recover()
            .expect_err("scratch before the first rename is ambiguous");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Recovery);
        assert!(artifact_path(&scratch.paths, MARKER_FILE_NAME).exists());
        assert!(artifact_path(&scratch.paths, MARKER_NEXT_FILE_NAME).exists());
        assert!(artifact_path(&scratch.paths, STAGED_FILE_NAME).exists());

        let tampered = Fixture::new();
        fs::write(
            artifact_path(&tampered.paths, STAGED_FILE_NAME),
            b"tampered-restored",
        )
        .expect("tamper stage");
        let before = fs::read_dir(
            tampered
                .paths
                .state_database()
                .parent()
                .expect("state directory"),
        )
        .unwrap()
        .count();
        let error = tampered
            .recover()
            .expect_err("tampered stage must fail closed");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Recovery);
        assert_eq!(
            fs::read_dir(
                tampered
                    .paths
                    .state_database()
                    .parent()
                    .expect("state directory")
            )
            .unwrap()
            .count(),
            before
        );
        assert!(artifact_path(&tampered.paths, MARKER_FILE_NAME).exists());
    }

    #[test]
    fn sidecar_mode_and_identity_mismatch_preserve_recovery_evidence() {
        let sidecar = Fixture::new();
        write_new(&artifact_path(&sidecar.paths, "state.sqlite-wal"), b"wal");
        let error = sidecar
            .recover()
            .expect_err("restore recovery rejects sidecars");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Recovery);
        assert!(artifact_path(&sidecar.paths, MARKER_FILE_NAME).exists());

        let mode = Fixture::new();
        fs::set_permissions(
            artifact_path(&mode.paths, STAGED_FILE_NAME),
            fs::Permissions::from_mode(0o640),
        )
        .expect("change stage mode");
        let error = mode
            .recover()
            .expect_err("insecure artifact mode fails closed");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Recovery);
        assert!(artifact_path(&mode.paths, MARKER_FILE_NAME).exists());

        let mismatch = Fixture::new();
        let wrong = ServiceDatabaseIdentity::new(
            &mismatch.paths,
            SourceGeneration::new([99; 32]).expect("different generation"),
            NonZeroU32::new(1).expect("schema version"),
            mismatch.identity.application_id(),
        );
        let error = recover_for_open(&mismatch.paths, &wrong, &mismatch.authority)
            .expect_err("marker intent mismatch fails closed");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Recovery);
        assert!(artifact_path(&mismatch.paths, MARKER_FILE_NAME).exists());
    }

    #[test]
    fn symlink_hardlink_fifo_and_foreign_replacement_fail_without_deletion() {
        use std::os::unix::fs::symlink;

        let symlinked = Fixture::new();
        let staged = artifact_path(&symlinked.paths, STAGED_FILE_NAME);
        let held = staged.with_file_name("held-stage");
        fs::rename(&staged, &held).expect("retain original stage");
        symlink(symlinked.paths.state_database(), &staged).expect("replace with symlink");
        let error = symlinked
            .recover()
            .expect_err("symlink replacement fails closed");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Recovery);
        assert!(
            fs::symlink_metadata(&staged)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(held.exists());

        let hardlinked = Fixture::new();
        let staged = artifact_path(&hardlinked.paths, STAGED_FILE_NAME);
        let link = staged.with_file_name("stage-hardlink");
        fs::hard_link(&staged, &link).expect("create hard link");
        let error = hardlinked
            .recover()
            .expect_err("multiple-link stage fails closed");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Recovery);
        assert!(staged.exists());
        assert!(link.exists());

        let fifo = Fixture::new();
        let staged = artifact_path(&fifo.paths, STAGED_FILE_NAME);
        fs::remove_file(&staged).expect("remove original stage");
        assert!(
            Command::new("mkfifo")
                .arg(&staged)
                .status()
                .expect("run mkfifo")
                .success()
        );
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o600)).expect("restrict FIFO");
        let error = fifo
            .recover()
            .expect_err("nonblocking FIFO admission fails closed");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Recovery);
        assert!(fs::symlink_metadata(&staged).unwrap().file_type().is_fifo());

        let replaced = Fixture::new();
        let staged = artifact_path(&replaced.paths, STAGED_FILE_NAME);
        let original = staged.with_file_name("original-stage");
        fs::rename(&staged, &original).expect("retain original stage");
        write_new(&staged, b"foreign-stage");
        let foreign = fs::read(&staged).unwrap();
        let error = replaced
            .recover()
            .expect_err("foreign replacement fails closed");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Recovery);
        assert_eq!(fs::read(&staged).unwrap(), foreign);
        assert_eq!(fs::read(&original).unwrap(), NEW_BYTES);
        assert!(artifact_path(&replaced.paths, MARKER_FILE_NAME).exists());
    }

    #[test]
    fn authority_drift_precedes_recovery_and_keeps_evidence() {
        let fixture = Fixture::new();
        let state_directory = fixture.paths.state_database().parent().unwrap();
        fs::set_permissions(state_directory, fs::Permissions::from_mode(0o770))
            .expect("drift state directory mode");
        let error = fixture
            .recover()
            .expect_err("authority drift must stop recovery");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Authority);
        assert!(artifact_path(&fixture.paths, MARKER_FILE_NAME).exists());
        assert!(artifact_path(&fixture.paths, STAGED_FILE_NAME).exists());
        fs::set_permissions(state_directory, fs::Permissions::from_mode(0o700))
            .expect("restore state directory mode");
    }

    #[test]
    fn orphan_stage_backup_or_scratch_without_marker_is_never_repaired() {
        for name in [STAGED_FILE_NAME, BACKUP_FILE_NAME, MARKER_NEXT_FILE_NAME] {
            let fixture = Fixture::new();
            fs::remove_file(artifact_path(&fixture.paths, MARKER_FILE_NAME))
                .expect("remove marker");
            if name != STAGED_FILE_NAME {
                fs::remove_file(artifact_path(&fixture.paths, STAGED_FILE_NAME))
                    .expect("remove default stage");
                write_new(&artifact_path(&fixture.paths, name), b"orphan");
            }
            sync_state_directory(&fixture.paths);
            let error = fixture
                .recover()
                .expect_err("orphan recovery evidence must fail closed");
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Recovery);
            assert!(artifact_path(&fixture.paths, name).exists());
        }
    }

    fn service_paths(root: &Path) -> ServiceSqlitePaths {
        let context = RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                Some(root.to_path_buf()),
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("bootstrap"),
            ServiceId::new("myc").expect("service"),
            InstanceId::new("recovery").expect("instance"),
        )
        .expect("runtime context");
        ServiceSqlitePaths::from_runtime_context(&context).expect("SQLite paths")
    }

    fn database_metadata(paths: &ServiceSqlitePaths) -> ServiceDatabaseMetadata {
        ServiceDatabaseMetadata::new(
            paths,
            SourceGeneration::new([7; 32]).expect("source generation"),
            NonZeroU32::new(1).expect("schema version"),
            1_700_000_000_000,
            ServiceSqliteApplicationId::new(0x5244_5351).expect("application ID"),
        )
        .expect("metadata")
    }

    fn write_new(path: &Path, bytes: &[u8]) -> File {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .expect("create artifact");
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .expect("set artifact mode");
        file.write_all(bytes).expect("write artifact");
        file.sync_all().expect("sync artifact");
        file
    }

    fn expectation(path: &Path) -> RestoreArtifactExpectation {
        let file = File::open(path).expect("open artifact");
        let status = fstat(&file).expect("artifact status");
        RestoreArtifactExpectation::new(
            crate::native_metadata::device(status.st_dev).expect("device"),
            status.st_ino,
            u64::try_from(status.st_size).expect("length"),
            hash_exact(
                &file,
                u64::try_from(status.st_size).expect("positive artifact length"),
            )
            .expect("artifact digest"),
        )
        .expect("artifact expectation")
    }

    fn artifact_path(paths: &ServiceSqlitePaths, name: &str) -> PathBuf {
        paths.state_database().with_file_name(name)
    }

    fn sync_state_directory(paths: &ServiceSqlitePaths) {
        File::open(paths.state_database().parent().expect("state directory"))
            .expect("open state directory")
            .sync_all()
            .expect("sync state directory");
    }

    fn assert_no_recovery_evidence(paths: &ServiceSqlitePaths) {
        for name in [
            STAGED_FILE_NAME,
            BACKUP_FILE_NAME,
            MARKER_FILE_NAME,
            MARKER_NEXT_FILE_NAME,
        ] {
            assert!(!artifact_path(paths, name).exists(), "unexpected {name}");
        }
    }
}

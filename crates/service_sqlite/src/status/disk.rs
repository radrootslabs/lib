//! Explicit state-filesystem capacity inspection and admission classification.

use core::fmt;
use std::error::Error;

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

use crate::{ServiceSqliteErrorKind, ServiceSqlitePaths};

const MAXIMUM_MINIMUM_FREE_BYTES: u64 = i64::MAX as u64;

/// Explicit minimum free-space policy for authoritative persistence admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct MinimumFreeBytes(u64);

impl MinimumFreeBytes {
    /// Validates a positive threshold representable by the governed TOML integer.
    pub const fn new(value: u64) -> Result<Self, StateFilesystemCapacityError> {
        if value == 0 {
            return Err(StateFilesystemCapacityError::InvalidMinimum);
        }
        if value > MAXIMUM_MINIMUM_FREE_BYTES {
            return Err(StateFilesystemCapacityError::MinimumTooLarge);
        }
        Ok(Self(value))
    }

    /// Returns the exact configured byte threshold.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for MinimumFreeBytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u64::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

/// Closed readiness result derived from one advisory filesystem snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StateFilesystemCapacityReadiness {
    Ready,
    LowDisk,
}

/// Immutable capacity snapshot safe to cache for later readiness projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct StateFilesystemCapacity {
    available_bytes: u64,
    minimum_free_bytes: MinimumFreeBytes,
    readiness: StateFilesystemCapacityReadiness,
}

impl StateFilesystemCapacity {
    fn new(available_bytes: u64, minimum_free_bytes: MinimumFreeBytes) -> Self {
        let readiness = if available_bytes >= minimum_free_bytes.get() {
            StateFilesystemCapacityReadiness::Ready
        } else {
            StateFilesystemCapacityReadiness::LowDisk
        };
        Self {
            available_bytes,
            minimum_free_bytes,
            readiness,
        }
    }

    /// Returns bytes available to the unprivileged service user.
    #[must_use]
    pub const fn available_bytes(self) -> u64 {
        self.available_bytes
    }

    /// Returns the exact policy used to classify this snapshot.
    #[must_use]
    pub const fn minimum_free_bytes(self) -> MinimumFreeBytes {
        self.minimum_free_bytes
    }

    /// Returns the closed ready or low-disk classification.
    #[must_use]
    pub const fn readiness(self) -> StateFilesystemCapacityReadiness {
        self.readiness
    }

    /// Returns whether this snapshot permits new authoritative admission.
    #[must_use]
    pub const fn allows_authoritative_admission(self) -> bool {
        matches!(self.readiness, StateFilesystemCapacityReadiness::Ready)
    }
}

/// Injected source for one synchronous state-filesystem capacity snapshot.
pub trait StateFilesystemCapacitySource {
    /// Returns bytes available to the unprivileged service user.
    fn available_bytes(
        &self,
        paths: &ServiceSqlitePaths,
    ) -> Result<u64, StateFilesystemCapacityError>;
}

/// Production Linux/macOS source backed by retained-directory `fstatvfs`.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlatformStateFilesystemCapacitySource;

impl StateFilesystemCapacitySource for PlatformStateFilesystemCapacitySource {
    fn available_bytes(
        &self,
        paths: &ServiceSqlitePaths,
    ) -> Result<u64, StateFilesystemCapacityError> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            available_bytes_native(paths)
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = paths;
            Err(StateFilesystemCapacityError::UnsupportedPlatform)
        }
    }
}

/// Runs one explicit capacity measurement and applies the supplied policy.
pub fn inspect_state_filesystem_capacity<S: StateFilesystemCapacitySource + ?Sized>(
    paths: &ServiceSqlitePaths,
    minimum_free_bytes: MinimumFreeBytes,
    source: &S,
) -> Result<StateFilesystemCapacity, StateFilesystemCapacityError> {
    source
        .available_bytes(paths)
        .map(|available| StateFilesystemCapacity::new(available, minimum_free_bytes))
}

/// Stable source-free failures for policy and filesystem capacity inspection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateFilesystemCapacityError {
    InvalidMinimum,
    MinimumTooLarge,
    MeasurementUnavailable,
    MeasurementOverflow,
    UnsupportedPlatform,
}

impl fmt::Display for StateFilesystemCapacityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidMinimum => "minimum free bytes must be positive",
            Self::MinimumTooLarge => "minimum free bytes exceed the supported integer range",
            Self::MeasurementUnavailable => "state filesystem capacity is unavailable",
            Self::MeasurementOverflow => "state filesystem capacity is not representable",
            Self::UnsupportedPlatform => {
                "state filesystem capacity inspection is unsupported on this platform"
            }
        })
    }
}

impl Error for StateFilesystemCapacityError {}

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
fn checked_available_bytes(
    available_blocks: u64,
    fragment_size: u64,
) -> Result<u64, StateFilesystemCapacityError> {
    if fragment_size == 0 {
        return Err(StateFilesystemCapacityError::MeasurementUnavailable);
    }
    available_blocks
        .checked_mul(fragment_size)
        .ok_or(StateFilesystemCapacityError::MeasurementOverflow)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn available_bytes_native(paths: &ServiceSqlitePaths) -> Result<u64, StateFilesystemCapacityError> {
    use rustix::fs::{Mode, OFlags, fstat, fstatvfs, open};

    let state_directory = paths
        .state_database()
        .parent()
        .ok_or(StateFilesystemCapacityError::MeasurementUnavailable)?;
    let held = open(
        state_directory,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| StateFilesystemCapacityError::MeasurementUnavailable)?;
    let held_status =
        fstat(&held).map_err(|_| StateFilesystemCapacityError::MeasurementUnavailable)?;
    validate_directory_status(&held_status)?;
    let capacity =
        fstatvfs(&held).map_err(|_| StateFilesystemCapacityError::MeasurementUnavailable)?;
    let available = checked_available_bytes(capacity.f_bavail, capacity.f_frsize)?;

    let current = open(
        state_directory,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| StateFilesystemCapacityError::MeasurementUnavailable)?;
    let current_status =
        fstat(&current).map_err(|_| StateFilesystemCapacityError::MeasurementUnavailable)?;
    validate_directory_status(&current_status)?;
    let final_held_status =
        fstat(&held).map_err(|_| StateFilesystemCapacityError::MeasurementUnavailable)?;
    validate_directory_status(&final_held_status)?;
    let expected_device = crate::native_metadata::device(held_status.st_dev)
        .map_err(|_| StateFilesystemCapacityError::MeasurementUnavailable)?;
    crate::require_condition(
        crate::native_metadata::identity_pair_matches(
            crate::native_metadata::device(final_held_status.st_dev)
                .map_err(|_| StateFilesystemCapacityError::MeasurementUnavailable)?,
            final_held_status.st_ino,
            crate::native_metadata::device(current_status.st_dev)
                .map_err(|_| StateFilesystemCapacityError::MeasurementUnavailable)?,
            current_status.st_ino,
            expected_device,
            held_status.st_ino,
        ),
        ServiceSqliteErrorKind::Authority,
    )
    .map_err(|_| StateFilesystemCapacityError::MeasurementUnavailable)?;
    Ok(available)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_directory_status(
    status: &rustix::fs::Stat,
) -> Result<(), StateFilesystemCapacityError> {
    use rustix::fs::FileType;
    use rustix::process::geteuid;

    if !crate::native_metadata::secure_directory(
        FileType::from_raw_mode(status.st_mode).is_dir(),
        status.st_uid,
        geteuid().as_raw(),
        crate::native_metadata::mode(status.st_mode),
    ) {
        return Err(StateFilesystemCapacityError::MeasurementUnavailable);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeSource(Result<u64, StateFilesystemCapacityError>);

    impl StateFilesystemCapacitySource for FakeSource {
        fn available_bytes(
            &self,
            _paths: &ServiceSqlitePaths,
        ) -> Result<u64, StateFilesystemCapacityError> {
            self.0
        }
    }

    fn unused_paths() -> ServiceSqlitePaths {
        use radroots_runtime_paths::{
            InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
            RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource,
            ServiceId,
        };

        let root = std::path::PathBuf::from("/unused/capacity-test-root");
        let context = RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                Some(root),
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("bootstrap"),
            ServiceId::new("myc").expect("service"),
            InstanceId::new("capacity").expect("instance"),
        )
        .expect("context");
        ServiceSqlitePaths::from_runtime_context(&context).expect("paths")
    }

    #[test]
    fn minimum_policy_and_strict_numeric_serde_are_bounded() {
        assert_eq!(
            MinimumFreeBytes::new(0),
            Err(StateFilesystemCapacityError::InvalidMinimum)
        );
        for value in [1, 268_435_456, i64::MAX as u64] {
            let policy = MinimumFreeBytes::new(value).expect("valid policy");
            assert_eq!(policy.get(), value);
            let wire = value.to_string();
            assert_eq!(serde_json::to_string(&policy).unwrap(), wire);
            assert_eq!(
                serde_json::from_str::<MinimumFreeBytes>(&wire).unwrap(),
                policy
            );
        }
        for value in [i64::MAX as u64 + 1, u64::MAX] {
            assert_eq!(
                MinimumFreeBytes::new(value),
                Err(StateFilesystemCapacityError::MinimumTooLarge)
            );
            assert!(serde_json::from_str::<MinimumFreeBytes>(&value.to_string()).is_err());
        }
        for wire in ["0", "-1", "1.0", "\"1\"", "null", "true", "{}", "[]"] {
            assert!(serde_json::from_str::<MinimumFreeBytes>(wire).is_err());
        }
    }

    #[test]
    fn injected_values_classify_exact_boundary_and_propagate_failure() {
        let paths = unused_paths();
        let minimum = MinimumFreeBytes::new(268_435_456).unwrap();
        for (available, readiness, allowed) in [
            (0, StateFilesystemCapacityReadiness::LowDisk, false),
            (
                minimum.get() - 1,
                StateFilesystemCapacityReadiness::LowDisk,
                false,
            ),
            (minimum.get(), StateFilesystemCapacityReadiness::Ready, true),
            (
                minimum.get() + 1,
                StateFilesystemCapacityReadiness::Ready,
                true,
            ),
            (u64::MAX, StateFilesystemCapacityReadiness::Ready, true),
        ] {
            let report =
                inspect_state_filesystem_capacity(&paths, minimum, &FakeSource(Ok(available)))
                    .expect("injected measurement");
            assert_eq!(report.available_bytes(), available);
            assert_eq!(report.minimum_free_bytes(), minimum);
            assert_eq!(report.readiness(), readiness);
            assert_eq!(report.allows_authoritative_admission(), allowed);
        }
        assert_eq!(
            inspect_state_filesystem_capacity(
                &paths,
                minimum,
                &FakeSource(Err(StateFilesystemCapacityError::MeasurementUnavailable,)),
            ),
            Err(StateFilesystemCapacityError::MeasurementUnavailable)
        );
    }

    #[test]
    fn arithmetic_and_wire_projection_are_exact() {
        assert_eq!(checked_available_bytes(7, 4), Ok(28));
        assert_eq!(checked_available_bytes(0, 4), Ok(0));
        assert_eq!(
            checked_available_bytes(1, 0),
            Err(StateFilesystemCapacityError::MeasurementUnavailable)
        );
        assert_eq!(
            checked_available_bytes(u64::MAX, 2),
            Err(StateFilesystemCapacityError::MeasurementOverflow)
        );
        let report = inspect_state_filesystem_capacity(
            &unused_paths(),
            MinimumFreeBytes::new(10).unwrap(),
            &FakeSource(Ok(10)),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_string(&report).unwrap(),
            r#"{"available_bytes":10,"minimum_free_bytes":10,"readiness":"ready"}"#
        );
    }

    #[test]
    fn errors_are_stable_source_free_and_redacted() {
        use std::error::Error as _;

        let sensitive = "/private/secret-state/state.sqlite";
        for error in [
            StateFilesystemCapacityError::InvalidMinimum,
            StateFilesystemCapacityError::MinimumTooLarge,
            StateFilesystemCapacityError::MeasurementUnavailable,
            StateFilesystemCapacityError::MeasurementOverflow,
            StateFilesystemCapacityError::UnsupportedPlatform,
        ] {
            assert!(error.source().is_none());
            assert!(!error.to_string().contains(sensitive));
            assert!(!format!("{error:?}").contains(sensitive));
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn native_adapter_is_descriptor_bound_nonmutating_and_rejects_unsafe_shapes() {
        use std::{
            fs,
            os::unix::fs::{MetadataExt, PermissionsExt, symlink},
        };

        fn paths(root: &std::path::Path, instance: &str) -> ServiceSqlitePaths {
            use radroots_runtime_paths::{
                InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
                RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource,
                ServiceId,
            };

            let context = RuntimeContext::resolve(
                &RadrootsPathResolver::new(
                    RadrootsPlatform::Linux,
                    RadrootsHostEnvironment::default(),
                ),
                RuntimeContextBootstrap::new(
                    RadrootsPathProfile::RepoLocal,
                    Some(root.to_path_buf()),
                    RuntimeContextSource::BootstrapCli,
                    RuntimeContextSource::BootstrapCli,
                )
                .expect("bootstrap"),
                ServiceId::new("myc").expect("service"),
                InstanceId::new(instance).expect("instance"),
            )
            .expect("context");
            ServiceSqlitePaths::from_runtime_context(&context).expect("paths")
        }

        let root = tempfile::tempdir().expect("root");
        let valid = paths(root.path(), "valid");
        let valid_directory = valid.state_database().parent().unwrap();
        fs::create_dir_all(valid_directory).expect("state directory");
        fs::set_permissions(valid_directory, fs::Permissions::from_mode(0o700)).unwrap();
        let before = fs::metadata(valid_directory).unwrap();
        let report = inspect_state_filesystem_capacity(
            &valid,
            MinimumFreeBytes::new(1).unwrap(),
            &PlatformStateFilesystemCapacitySource,
        )
        .expect("native measurement");
        assert!(report.available_bytes() > 0);
        assert!(report.allows_authoritative_admission());
        let after = fs::metadata(valid_directory).unwrap();
        assert_eq!(before.dev(), after.dev());
        assert_eq!(before.ino(), after.ino());
        assert_eq!(before.permissions().mode(), after.permissions().mode());
        assert!(fs::read_dir(valid_directory).unwrap().next().is_none());
        for mode in [0o750, 0o755] {
            fs::set_permissions(valid_directory, fs::Permissions::from_mode(mode)).unwrap();
            let report = inspect_state_filesystem_capacity(
                &valid,
                MinimumFreeBytes::new(1).unwrap(),
                &PlatformStateFilesystemCapacitySource,
            )
            .expect("non-writable group/other mode remains admissible");
            assert!(report.allows_authoritative_admission());
        }

        let missing = paths(root.path(), "missing");
        assert_eq!(
            inspect_state_filesystem_capacity(
                &missing,
                MinimumFreeBytes::new(1).unwrap(),
                &PlatformStateFilesystemCapacitySource,
            ),
            Err(StateFilesystemCapacityError::MeasurementUnavailable)
        );

        let file = paths(root.path(), "file");
        let file_directory = file.state_database().parent().unwrap();
        fs::create_dir_all(file_directory.parent().unwrap()).unwrap();
        fs::write(file_directory, b"not a directory").unwrap();
        assert_eq!(
            inspect_state_filesystem_capacity(
                &file,
                MinimumFreeBytes::new(1).unwrap(),
                &PlatformStateFilesystemCapacitySource,
            ),
            Err(StateFilesystemCapacityError::MeasurementUnavailable)
        );

        let linked = paths(root.path(), "linked");
        let linked_directory = linked.state_database().parent().unwrap();
        fs::create_dir_all(linked_directory.parent().unwrap()).unwrap();
        let target = root.path().join("linked-target");
        fs::create_dir(&target).unwrap();
        symlink(&target, linked_directory).unwrap();
        assert_eq!(
            inspect_state_filesystem_capacity(
                &linked,
                MinimumFreeBytes::new(1).unwrap(),
                &PlatformStateFilesystemCapacitySource,
            ),
            Err(StateFilesystemCapacityError::MeasurementUnavailable)
        );

        let insecure = paths(root.path(), "insecure");
        let insecure_directory = insecure.state_database().parent().unwrap();
        fs::create_dir_all(insecure_directory).unwrap();
        for mode in [0o720, 0o702, 0o722] {
            fs::set_permissions(insecure_directory, fs::Permissions::from_mode(mode)).unwrap();
            assert_eq!(
                inspect_state_filesystem_capacity(
                    &insecure,
                    MinimumFreeBytes::new(1).unwrap(),
                    &PlatformStateFilesystemCapacitySource,
                ),
                Err(StateFilesystemCapacityError::MeasurementUnavailable)
            );
        }
    }
}

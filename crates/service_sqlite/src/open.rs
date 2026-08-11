//! Instance-bound SQLite paths and declarative open modes.

use core::fmt;
use std::{
    error::Error,
    path::{Path, PathBuf},
};

use radroots_runtime_paths::{
    InstanceId, RuntimeContext, ServiceId, default_service_instance_artifacts,
};
use serde::Serialize;

/// Canonical database and writer-lock paths for one validated service instance.
///
/// Callers cannot forge paths or rebind the service and instance independently:
///
/// ```compile_fail
/// use std::path::PathBuf;
/// use radroots_runtime_paths::{InstanceId, ServiceId};
/// use radroots_service_sqlite::ServiceSqlitePaths;
///
/// let _ = ServiceSqlitePaths {
///     service: ServiceId::new("myc").unwrap(),
///     instance: InstanceId::new("primary").unwrap(),
///     state_database: PathBuf::from("/tmp/alternate.sqlite"),
///     state_lock: PathBuf::from("/tmp/alternate.lock"),
/// };
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct ServiceSqlitePaths {
    service: ServiceId,
    instance: InstanceId,
    state_database: PathBuf,
    state_lock: PathBuf,
}

impl ServiceSqlitePaths {
    /// Derives the fixed SQLite artifacts from one immutable runtime context.
    pub fn from_runtime_context(context: &RuntimeContext) -> Result<Self, ServiceSqlitePathError> {
        validate_state_directory(context.paths().state())?;
        let artifacts = default_service_instance_artifacts(context.paths());
        Ok(Self {
            service: context.service().clone(),
            instance: context.instance().clone(),
            state_database: artifacts.state_database().to_path_buf(),
            state_lock: artifacts.state_lock().to_path_buf(),
        })
    }

    /// Returns the validated service identity bound to these paths.
    #[must_use]
    pub fn service(&self) -> &ServiceId {
        &self.service
    }

    /// Returns the validated instance identity bound to these paths.
    #[must_use]
    pub fn instance(&self) -> &InstanceId {
        &self.instance
    }

    /// Returns the canonical `state.sqlite` path.
    #[must_use]
    pub fn state_database(&self) -> &Path {
        &self.state_database
    }

    /// Returns the canonical retained `state.lock` path.
    #[must_use]
    pub fn state_lock(&self) -> &Path {
        &self.state_lock
    }
}

impl fmt::Debug for ServiceSqlitePaths {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceSqlitePaths")
            .field("service", &self.service)
            .field("instance", &self.instance)
            .field("state_database", &"[redacted]")
            .field("state_lock", &"[redacted]")
            .finish()
    }
}

/// Path-shape failure detected before any filesystem or SQLite operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceSqlitePathError {
    RelativeStateDirectory,
    MissingStateDirectoryParent,
}

impl fmt::Display for ServiceSqlitePathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RelativeStateDirectory => {
                formatter.write_str("SQLite state directory must be absolute")
            }
            Self::MissingStateDirectoryParent => {
                formatter.write_str("SQLite state directory must have a parent")
            }
        }
    }
}

impl Error for ServiceSqlitePathError {}

fn validate_state_directory(path: &Path) -> Result<(), ServiceSqlitePathError> {
    if !path.is_absolute() {
        return Err(ServiceSqlitePathError::RelativeStateDirectory);
    }
    if path.parent().is_none() {
        return Err(ServiceSqlitePathError::MissingStateDirectoryParent);
    }
    Ok(())
}

/// Declarative behavior for opening one service-owned SQLite database.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenMode {
    Initialize,
    ReadWriteExisting,
    ReadOnlyInspection,
}

impl OpenMode {
    /// Returns whether this mode permits creating missing state.
    #[must_use]
    pub const fn may_create(self) -> bool {
        matches!(self, Self::Initialize)
    }

    /// Returns whether state must already exist before opening.
    #[must_use]
    pub const fn requires_existing(self) -> bool {
        !matches!(self, Self::Initialize)
    }

    /// Returns whether exclusive writer authority is required.
    #[must_use]
    pub const fn requires_writer_authority(self) -> bool {
        !matches!(self, Self::ReadOnlyInspection)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use radroots_runtime_paths::{
        RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver, RadrootsPlatform,
        RuntimeContextBootstrap, RuntimeContextSource,
    };

    use super::*;

    fn runtime_context(
        profile: RadrootsPathProfile,
        repo_local_root: Option<PathBuf>,
        service: &str,
        instance: &str,
    ) -> RuntimeContext {
        let profile_source = if matches!(profile, RadrootsPathProfile::RepoLocal) {
            RuntimeContextSource::BootstrapCli
        } else {
            RuntimeContextSource::SafeDefault
        };
        RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                profile,
                repo_local_root,
                profile_source,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("valid bootstrap"),
            ServiceId::new(service).expect("valid service"),
            InstanceId::new(instance).expect("valid instance"),
        )
        .expect("valid runtime context")
    }

    #[test]
    fn paths_bind_exact_service_host_and_repo_local_artifacts() {
        let myc = ServiceSqlitePaths::from_runtime_context(&runtime_context(
            RadrootsPathProfile::ServiceHost,
            None,
            "myc",
            "primary",
        ))
        .expect("Myc paths");
        assert_eq!(myc.service().as_str(), "myc");
        assert_eq!(myc.instance().as_str(), "primary");
        assert_eq!(
            myc.state_database(),
            Path::new("/var/lib/radroots/services/myc/primary/state.sqlite")
        );
        assert_eq!(
            myc.state_lock(),
            Path::new("/var/lib/radroots/services/myc/primary/state.lock")
        );

        let rhi = ServiceSqlitePaths::from_runtime_context(&runtime_context(
            RadrootsPathProfile::RepoLocal,
            Some(PathBuf::from("/repo/.local/radroots")),
            "rhi",
            "north-01",
        ))
        .expect("RHI paths");
        assert_eq!(rhi.service().as_str(), "rhi");
        assert_eq!(rhi.instance().as_str(), "north-01");
        assert_eq!(
            rhi.state_database(),
            Path::new("/repo/.local/radroots/data/services/rhi/north-01/state.sqlite")
        );
        assert_eq!(
            rhi.state_lock(),
            Path::new("/repo/.local/radroots/data/services/rhi/north-01/state.lock")
        );

        let second = ServiceSqlitePaths::from_runtime_context(&runtime_context(
            RadrootsPathProfile::RepoLocal,
            Some(PathBuf::from("/repo/.local/radroots")),
            "rhi",
            "south-02",
        ))
        .expect("second RHI paths");
        assert_ne!(rhi, second);
        assert_ne!(rhi.state_database(), second.state_database());
        assert_ne!(rhi.state_lock(), second.state_lock());
    }

    #[test]
    fn path_shape_failures_are_typed_path_free_and_debug_is_redacted() {
        assert_eq!(
            validate_state_directory(Path::new("relative/state")),
            Err(ServiceSqlitePathError::RelativeStateDirectory)
        );
        assert_eq!(
            validate_state_directory(Path::new("/")),
            Err(ServiceSqlitePathError::MissingStateDirectoryParent)
        );

        let error = ServiceSqlitePathError::RelativeStateDirectory;
        assert_eq!(error.to_string(), "SQLite state directory must be absolute");
        assert_eq!(format!("{error:?}"), "RelativeStateDirectory");

        let paths = ServiceSqlitePaths::from_runtime_context(&runtime_context(
            RadrootsPathProfile::RepoLocal,
            Some(PathBuf::from("/sensitive/project-root")),
            "myc",
            "private-instance",
        ))
        .expect("redacted paths");
        let debug = format!("{paths:?}");
        assert!(debug.contains("service: ServiceId(\"myc\")"));
        assert!(debug.contains("instance: InstanceId(\"private-instance\")"));
        assert!(debug.contains("state_database: \"[redacted]\""));
        assert!(debug.contains("state_lock: \"[redacted]\""));
        assert!(!debug.contains("sensitive"));
        assert!(!debug.contains("project-root"));
        assert!(!debug.contains("state.sqlite"));
        assert!(!debug.contains("state.lock"));
    }

    #[test]
    fn open_mode_wire_inventory_and_semantics_are_exact() {
        let inventory = [
            (OpenMode::Initialize, "initialize", true, false, true),
            (
                OpenMode::ReadWriteExisting,
                "read_write_existing",
                false,
                true,
                true,
            ),
            (
                OpenMode::ReadOnlyInspection,
                "read_only_inspection",
                false,
                true,
                false,
            ),
        ];
        for (mode, wire, may_create, requires_existing, requires_writer) in inventory {
            assert_eq!(
                serde_json::to_string(&mode).unwrap(),
                format!(r#""{wire}""#)
            );
            assert_eq!(mode.may_create(), may_create);
            assert_eq!(mode.requires_existing(), requires_existing);
            assert_eq!(mode.requires_writer_authority(), requires_writer);
        }
    }
}

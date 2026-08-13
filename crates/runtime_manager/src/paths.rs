use core::fmt;
use std::path::{Path, PathBuf};

use radroots_runtime_paths::{RuntimeContext, default_service_instance_artifacts};

use crate::error::RadrootsRuntimeManagerError;
use crate::model::RadrootsRuntimeManagementContract;

/// Manager-owned paths derived from the manager's validated service context.
///
/// External callers cannot forge another root set:
///
/// ```compile_fail
/// use std::path::PathBuf;
/// use radroots_runtime_manager::ManagedRuntimeSharedPaths;
///
/// let _ = ManagedRuntimeSharedPaths {
///     instance_registry_path: PathBuf::from("/tmp/escape"),
///     artifact_cache_dir: PathBuf::from("/tmp/escape"),
///     install_root: PathBuf::from("/tmp/escape"),
/// };
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct ManagedRuntimeSharedPaths {
    instance_registry_path: PathBuf,
    artifact_cache_dir: PathBuf,
    install_root: PathBuf,
    logs_root: PathBuf,
    run_root: PathBuf,
}

impl ManagedRuntimeSharedPaths {
    #[must_use]
    pub fn instance_registry_path(&self) -> &Path {
        &self.instance_registry_path
    }

    #[must_use]
    pub fn artifact_cache_dir(&self) -> &Path {
        &self.artifact_cache_dir
    }

    #[must_use]
    pub fn install_root(&self) -> &Path {
        &self.install_root
    }

    #[must_use]
    pub fn logs_root(&self) -> &Path {
        &self.logs_root
    }

    #[must_use]
    pub fn run_root(&self) -> &Path {
        &self.run_root
    }
}

impl fmt::Debug for ManagedRuntimeSharedPaths {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ManagedRuntimeSharedPaths([redacted])")
    }
}

/// Operational paths for one validated service instance.
///
/// Service-owned directories and fixed artifacts come only from the supplied
/// [`RuntimeContext`]. The manager-owned install directory comes only from its
/// own validated context. Process tracking and captured stdout/stderr remain
/// under manager-owned roots, so lifecycle removal cannot delete canonical
/// service state or secrets.
///
/// ```compile_fail
/// use std::path::PathBuf;
/// use radroots_runtime_manager::ManagedRuntimeInstancePaths;
///
/// let _ = ManagedRuntimeInstancePaths {
///     install_dir: PathBuf::from("/tmp/escape"),
/// };
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct ManagedRuntimeInstancePaths {
    context: RuntimeContext,
    install_dir: PathBuf,
    logs_dir: PathBuf,
    run_dir: PathBuf,
    pid_file_path: PathBuf,
    stdout_log_path: PathBuf,
    stderr_log_path: PathBuf,
}

impl ManagedRuntimeInstancePaths {
    #[must_use]
    pub fn context(&self) -> &RuntimeContext {
        &self.context
    }

    #[must_use]
    pub fn install_dir(&self) -> &Path {
        &self.install_dir
    }

    #[must_use]
    pub fn config_dir(&self) -> &Path {
        self.context.paths().config()
    }

    #[must_use]
    pub fn state_dir(&self) -> &Path {
        self.context.paths().state()
    }

    #[must_use]
    pub fn logs_dir(&self) -> &Path {
        &self.logs_dir
    }

    #[must_use]
    pub fn run_dir(&self) -> &Path {
        &self.run_dir
    }

    #[must_use]
    pub fn secrets_dir(&self) -> &Path {
        self.context.paths().secrets()
    }

    #[must_use]
    pub fn config_path(&self) -> PathBuf {
        default_service_instance_artifacts(self.context.paths())
            .config()
            .to_path_buf()
    }

    #[must_use]
    pub fn state_database_path(&self) -> PathBuf {
        default_service_instance_artifacts(self.context.paths())
            .state_database()
            .to_path_buf()
    }

    #[must_use]
    pub fn state_lock_path(&self) -> PathBuf {
        default_service_instance_artifacts(self.context.paths())
            .state_lock()
            .to_path_buf()
    }

    #[must_use]
    pub fn admin_socket_path(&self) -> PathBuf {
        default_service_instance_artifacts(self.context.paths())
            .admin_socket()
            .to_path_buf()
    }

    #[must_use]
    pub fn pid_file_path(&self) -> &Path {
        &self.pid_file_path
    }

    #[must_use]
    pub fn stdout_log_path(&self) -> &Path {
        &self.stdout_log_path
    }

    #[must_use]
    pub fn stderr_log_path(&self) -> &Path {
        &self.stderr_log_path
    }
}

impl fmt::Debug for ManagedRuntimeInstancePaths {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ManagedRuntimeInstancePaths([redacted])")
    }
}

#[must_use]
pub(crate) fn resolve_shared_paths(context: &RuntimeContext) -> ManagedRuntimeSharedPaths {
    ManagedRuntimeSharedPaths {
        instance_registry_path: context.paths().config().join("instances.toml"),
        artifact_cache_dir: context.paths().cache().join("artifacts"),
        install_root: context.paths().state().join("installs"),
        logs_root: context.paths().logs().join("instances"),
        run_root: context.paths().run().join("instances"),
    }
}

#[must_use]
#[cfg(test)]
pub(crate) fn resolve_instance_paths(
    shared: &ManagedRuntimeSharedPaths,
    context: &RuntimeContext,
) -> ManagedRuntimeInstancePaths {
    let suffix = PathBuf::from(context.service().as_str()).join(context.instance().as_str());
    let logs_dir = shared.logs_root.join(&suffix);
    let run_dir = shared.run_root.join(&suffix);

    ManagedRuntimeInstancePaths {
        context: context.clone(),
        install_dir: shared.install_root.join(suffix),
        logs_dir: logs_dir.clone(),
        run_dir: run_dir.clone(),
        pid_file_path: run_dir.join("runtime.pid"),
        stdout_log_path: logs_dir.join("stdout.log"),
        stderr_log_path: logs_dir.join("stderr.log"),
    }
}

pub fn bootstrap_runtime<'a>(
    contract: &'a RadrootsRuntimeManagementContract,
    runtime_id: &str,
) -> Result<&'a crate::model::BootstrapRuntimeContract, RadrootsRuntimeManagerError> {
    contract
        .bootstrap
        .get(runtime_id)
        .ok_or(RadrootsRuntimeManagerError::UnknownBootstrapRuntime)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use radroots_runtime_paths::{
        InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource, ServiceId,
    };

    use super::{resolve_instance_paths, resolve_shared_paths};

    fn repo_context(service: &str, instance: &str, root: &str) -> RuntimeContext {
        RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                Some(PathBuf::from(root)),
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("bootstrap"),
            ServiceId::new(service).expect("service"),
            InstanceId::new(instance).expect("instance"),
        )
        .expect("context")
    }

    fn service_host_context(service: &str, instance: &str) -> RuntimeContext {
        RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::ServiceHost,
                None,
                RuntimeContextSource::SafeDefault,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("bootstrap"),
            ServiceId::new(service).expect("service"),
            InstanceId::new(instance).expect("instance"),
        )
        .expect("context")
    }

    #[test]
    fn shared_paths_derive_only_from_the_manager_context() {
        let manager = repo_context("runtime-manager", "default", "/repo/.radroots");
        let paths = resolve_shared_paths(&manager);

        assert_eq!(
            paths.instance_registry_path(),
            PathBuf::from("/repo/.radroots/config/services/runtime-manager/default/instances.toml")
        );
        assert_eq!(
            paths.artifact_cache_dir(),
            PathBuf::from("/repo/.radroots/cache/services/runtime-manager/default/artifacts")
        );
        assert_eq!(
            paths.install_root(),
            PathBuf::from("/repo/.radroots/data/services/runtime-manager/default/installs")
        );
        assert_eq!(
            format!("{paths:?}"),
            "ManagedRuntimeSharedPaths([redacted])"
        );
    }

    #[test]
    fn instance_paths_use_the_exact_service_context_and_fixed_artifacts() {
        let shared = resolve_shared_paths(&repo_context(
            "runtime-manager",
            "default",
            "/repo/.radroots",
        ));
        let service = repo_context("myc", "north", "/repo/.radroots");
        let paths = resolve_instance_paths(&shared, &service);

        assert_eq!(
            paths.install_dir(),
            PathBuf::from(
                "/repo/.radroots/data/services/runtime-manager/default/installs/myc/north"
            )
        );
        assert_eq!(
            paths.config_path(),
            PathBuf::from("/repo/.radroots/config/services/myc/north/config.toml")
        );
        assert_eq!(
            paths.state_database_path(),
            PathBuf::from("/repo/.radroots/data/services/myc/north/state.sqlite")
        );
        assert_eq!(
            paths.state_lock_path(),
            PathBuf::from("/repo/.radroots/data/services/myc/north/state.lock")
        );
        assert_eq!(
            paths.admin_socket_path(),
            PathBuf::from("/repo/.radroots/run/services/myc/north/admin.sock")
        );
        assert_eq!(
            paths.stdout_log_path(),
            PathBuf::from(
                "/repo/.radroots/logs/services/runtime-manager/default/instances/myc/north/stdout.log"
            )
        );
        assert_eq!(
            format!("{paths:?}"),
            "ManagedRuntimeInstancePaths([redacted])"
        );
    }

    #[test]
    fn multi_instance_paths_cannot_cross_service_contexts() {
        let shared = resolve_shared_paths(&repo_context(
            "runtime-manager",
            "default",
            "/repo/.radroots",
        ));
        let north =
            resolve_instance_paths(&shared, &repo_context("rhi", "north", "/repo/.radroots"));
        let south =
            resolve_instance_paths(&shared, &repo_context("rhi", "south", "/repo/.radroots"));

        assert_ne!(north, south);
        assert!(north.state_dir().ends_with("services/rhi/north"));
        assert!(south.state_dir().ends_with("services/rhi/south"));
        assert!(!north.state_dir().starts_with(south.state_dir()));
        assert!(!south.state_dir().starts_with(north.state_dir()));
    }

    #[test]
    fn linux_service_host_paths_preserve_the_canonical_service_layout() {
        let manager = service_host_context("runtime-manager", "default");
        let shared = resolve_shared_paths(&manager);
        let service = service_host_context("myc", "primary");
        let paths = resolve_instance_paths(&shared, &service);

        assert_eq!(
            shared.instance_registry_path(),
            PathBuf::from("/etc/radroots/services/runtime-manager/default/instances.toml")
        );
        assert_eq!(
            paths.config_path(),
            PathBuf::from("/etc/radroots/services/myc/primary/config.toml")
        );
        assert_eq!(
            paths.state_database_path(),
            PathBuf::from("/var/lib/radroots/services/myc/primary/state.sqlite")
        );
        assert_eq!(
            paths.admin_socket_path(),
            PathBuf::from("/run/radroots/services/myc/primary/admin.sock")
        );
    }
}

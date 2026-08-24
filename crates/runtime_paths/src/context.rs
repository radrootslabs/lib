//! Immutable resolved bootstrap context for one service instance.

use core::fmt;
use std::path::{Path, PathBuf};

use serde::{Serialize, Serializer, ser::SerializeStruct};
use thiserror::Error;

use crate::{
    InstanceId, RadrootsPathProfile, RadrootsPathResolver, RadrootsServiceInstancePaths, ServiceId,
};

/// Closed provenance vocabulary for effective runtime configuration.
///
/// Arbitrary strings, secrets, and high-cardinality labels cannot be converted
/// into this vocabulary:
///
/// ```compile_fail
/// use radroots_runtime_paths::RuntimeContextSource;
///
/// let _ = RuntimeContextSource::from("secret:caller-controlled-value");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeContextSource {
    BootstrapCli,
    Toml,
    SafeDefault,
    DerivedPath,
}

/// Sealed validated bootstrap input for one runtime context.
#[derive(Clone, PartialEq, Eq)]
pub struct RuntimeContextBootstrap {
    profile: RadrootsPathProfile,
    repo_local_root: Option<PathBuf>,
    profile_source: RuntimeContextSource,
    instance_source: RuntimeContextSource,
}

impl fmt::Debug for RuntimeContextBootstrap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeContextBootstrap")
            .field("profile", &self.profile)
            .field(
                "repo_local_root",
                &self.repo_local_root.as_ref().map(|_| "[redacted]"),
            )
            .field("profile_source", &self.profile_source)
            .field("instance_source", &self.instance_source)
            .finish()
    }
}

impl RuntimeContextBootstrap {
    pub fn new(
        profile: RadrootsPathProfile,
        repo_local_root: Option<PathBuf>,
        profile_source: RuntimeContextSource,
        instance_source: RuntimeContextSource,
    ) -> Result<Self, RuntimeContextError> {
        if matches!(profile, RadrootsPathProfile::MobileNative)
            || (matches!(profile, RadrootsPathProfile::RepoLocal)
                && !matches!(profile_source, RuntimeContextSource::BootstrapCli))
            || !matches!(
                profile_source,
                RuntimeContextSource::BootstrapCli | RuntimeContextSource::SafeDefault
            )
            || !matches!(
                instance_source,
                RuntimeContextSource::BootstrapCli | RuntimeContextSource::SafeDefault
            )
            || (matches!(profile, RadrootsPathProfile::RepoLocal) != repo_local_root.is_some())
        {
            return Err(RuntimeContextError::InvalidBootstrapBinding);
        }
        Ok(Self {
            profile,
            repo_local_root,
            profile_source,
            instance_source,
        })
    }

    #[must_use]
    pub fn profile(&self) -> RadrootsPathProfile {
        self.profile
    }
}

/// Closed provenance bound to every runtime-context field class.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RuntimeContextSources {
    service: RuntimeContextSource,
    instance: RuntimeContextSource,
    profile: RuntimeContextSource,
    repo_local_root: Option<RuntimeContextSource>,
    paths: RuntimeContextSource,
}

impl RuntimeContextSources {
    #[must_use]
    pub fn service(&self) -> RuntimeContextSource {
        self.service
    }

    #[must_use]
    pub fn instance(&self) -> RuntimeContextSource {
        self.instance
    }

    #[must_use]
    pub fn profile(&self) -> RuntimeContextSource {
        self.profile
    }

    #[must_use]
    pub fn repo_local_root(&self) -> Option<RuntimeContextSource> {
        self.repo_local_root
    }

    #[must_use]
    pub fn paths(&self) -> RuntimeContextSource {
        self.paths
    }
}

/// Immutable resolved bootstrap identity and canonical paths.
///
/// External callers cannot forge or mutate a context:
///
/// ```compile_fail
/// use radroots_runtime_paths::RuntimeContext;
///
/// let _ = RuntimeContext {
///     service: todo!(),
///     instance: todo!(),
///     profile: todo!(),
///     repo_local_root: todo!(),
///     paths: todo!(),
///     sources: todo!(),
/// };
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct RuntimeContext {
    service: ServiceId,
    instance: InstanceId,
    profile: RadrootsPathProfile,
    repo_local_root: Option<PathBuf>,
    paths: RadrootsServiceInstancePaths,
    sources: RuntimeContextSources,
}

impl RuntimeContext {
    pub fn resolve(
        resolver: &RadrootsPathResolver,
        bootstrap: RuntimeContextBootstrap,
        service: ServiceId,
        instance: InstanceId,
    ) -> Result<Self, RuntimeContextError> {
        let RuntimeContextBootstrap {
            profile,
            repo_local_root,
            profile_source,
            instance_source,
        } = bootstrap;
        let roots = resolver
            .resolve(profile, repo_local_root.as_deref())
            .map_err(|_| RuntimeContextError::PathSelection)?;
        let paths = RadrootsServiceInstancePaths::from_resolved_roots(&roots, &service, &instance);
        let sources = RuntimeContextSources {
            service: RuntimeContextSource::SafeDefault,
            instance: instance_source,
            profile: profile_source,
            repo_local_root: repo_local_root
                .as_ref()
                .map(|_| RuntimeContextSource::BootstrapCli),
            paths: RuntimeContextSource::DerivedPath,
        };

        Ok(Self {
            service,
            instance,
            profile,
            repo_local_root,
            paths,
            sources,
        })
    }

    #[must_use]
    pub fn service(&self) -> &ServiceId {
        &self.service
    }

    #[must_use]
    pub fn instance(&self) -> &InstanceId {
        &self.instance
    }

    #[must_use]
    pub fn profile(&self) -> RadrootsPathProfile {
        self.profile
    }

    /// Returns the validated explicit repo-local base when that profile is active.
    #[must_use]
    pub fn repo_local_root(&self) -> Option<&Path> {
        self.repo_local_root.as_deref()
    }

    #[must_use]
    pub fn paths(&self) -> &RadrootsServiceInstancePaths {
        &self.paths
    }

    #[must_use]
    pub fn sources(&self) -> &RuntimeContextSources {
        &self.sources
    }
}

impl fmt::Debug for RuntimeContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeContext")
            .field("service", &self.service)
            .field("instance", &self.instance)
            .field("profile", &self.profile)
            .field(
                "repo_local_root",
                &self.repo_local_root.as_ref().map(|_| "[redacted]"),
            )
            .field("paths", &"[redacted]")
            .field("sources", &self.sources)
            .finish()
    }
}

impl Serialize for RuntimeContext {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("RuntimeContext", 5)?;
        state.serialize_field("service", &self.service)?;
        state.serialize_field("instance", &self.instance)?;
        state.serialize_field("profile", &self.profile.to_string())?;
        state.serialize_field("paths", "[redacted]")?;
        state.serialize_field("sources", &self.sources)?;
        state.end()
    }
}

/// Safe construction failures for [`RuntimeContext`].
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum RuntimeContextError {
    #[error("runtime context bootstrap provenance does not match its selectors")]
    InvalidBootstrapBinding,
    #[error("runtime context path selection failed")]
    PathSelection,
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::SystemTime};

    use serde_json::json;

    use super::{
        RuntimeContext, RuntimeContextBootstrap, RuntimeContextError, RuntimeContextSource,
    };
    use crate::{
        InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform, ServiceId,
    };

    fn repo_local_context(base: PathBuf) -> RuntimeContext {
        repo_local_context_for(base, "myc", "primary")
    }

    fn repo_local_context_for(base: PathBuf, service: &str, instance: &str) -> RuntimeContext {
        RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                Some(base),
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("bootstrap"),
            ServiceId::new(service).expect("service"),
            InstanceId::new(instance).expect("instance"),
        )
        .expect("runtime context")
    }

    #[test]
    fn one_repo_local_base_derives_exact_noncolliding_service_instances() {
        let base = PathBuf::from("/repo/.local/radroots");
        let mut configs = Vec::new();

        for (service, instance) in [("myc", "primary"), ("myc", "secondary"), ("rhi", "default")] {
            let context = repo_local_context_for(base.clone(), service, instance);
            assert_eq!(context.repo_local_root(), Some(base.as_path()));
            let suffix = PathBuf::from("services").join(service).join(instance);
            assert_eq!(context.paths().config(), base.join("config").join(&suffix));
            assert_eq!(context.paths().state(), base.join("data").join(&suffix));
            assert_eq!(context.paths().cache(), base.join("cache").join(&suffix));
            assert_eq!(context.paths().logs(), base.join("logs").join(&suffix));
            assert_eq!(context.paths().run(), base.join("run").join(&suffix));
            assert_eq!(
                context.paths().secrets(),
                base.join("secrets").join(&suffix)
            );
            configs.push(context.paths().config().to_path_buf());
        }

        configs.sort();
        configs.dedup();
        assert_eq!(configs.len(), 3);
    }

    #[test]
    fn retained_macos_and_windows_profiles_derive_exact_context_paths() {
        let macos = RuntimeContext::resolve(
            &RadrootsPathResolver::new(
                RadrootsPlatform::Macos,
                RadrootsHostEnvironment {
                    home_dir: Some(PathBuf::from("/Users/treesap")),
                    ..RadrootsHostEnvironment::default()
                },
            ),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::InteractiveUser,
                None,
                RuntimeContextSource::SafeDefault,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("macOS bootstrap"),
            ServiceId::new("myc").expect("service"),
            InstanceId::new("primary").expect("instance"),
        )
        .expect("macOS context");
        assert_eq!(
            macos.paths().config(),
            PathBuf::from(
                "/Users/treesap/Library/Application Support/Radroots/config/services/myc/primary"
            )
        );
        assert_eq!(
            macos.paths().state(),
            PathBuf::from(
                "/Users/treesap/Library/Application Support/Radroots/data/services/myc/primary"
            )
        );
        assert_eq!(
            macos.paths().cache(),
            PathBuf::from("/Users/treesap/Library/Caches/Radroots/services/myc/primary")
        );
        assert_eq!(
            macos.paths().logs(),
            PathBuf::from("/Users/treesap/Library/Logs/Radroots/services/myc/primary")
        );
        assert_eq!(
            macos.paths().run(),
            PathBuf::from(
                "/Users/treesap/Library/Application Support/Radroots/run/services/myc/primary"
            )
        );
        assert_eq!(
            macos.paths().secrets(),
            PathBuf::from(
                "/Users/treesap/Library/Application Support/Radroots/secrets/services/myc/primary"
            )
        );

        let appdata = PathBuf::from(r"C:\Users\treesap\AppData\Roaming");
        let localappdata = PathBuf::from(r"C:\Users\treesap\AppData\Local");
        let windows = RuntimeContext::resolve(
            &RadrootsPathResolver::new(
                RadrootsPlatform::Windows,
                RadrootsHostEnvironment {
                    appdata_dir: Some(appdata.clone()),
                    localappdata_dir: Some(localappdata.clone()),
                    ..RadrootsHostEnvironment::default()
                },
            ),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::InteractiveUser,
                None,
                RuntimeContextSource::SafeDefault,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("Windows bootstrap"),
            ServiceId::new("rhi").expect("service"),
            InstanceId::new("default").expect("instance"),
        )
        .expect("Windows context");
        let suffix = PathBuf::from("services/rhi/default");
        assert_eq!(
            windows.paths().config(),
            appdata.join("Radroots/config").join(&suffix)
        );
        assert_eq!(
            windows.paths().state(),
            localappdata.join("Radroots/data").join(&suffix)
        );
        assert_eq!(
            windows.paths().cache(),
            localappdata.join("Radroots/cache").join(&suffix)
        );
        assert_eq!(
            windows.paths().logs(),
            localappdata.join("Radroots/logs").join(&suffix)
        );
        assert_eq!(
            windows.paths().run(),
            localappdata.join("Radroots/run").join(&suffix)
        );
        assert_eq!(
            windows.paths().secrets(),
            appdata.join("Radroots/secrets").join(&suffix)
        );

        assert_eq!(
            RuntimeContext::resolve(
                &RadrootsPathResolver::new(
                    RadrootsPlatform::Windows,
                    RadrootsHostEnvironment::default(),
                ),
                RuntimeContextBootstrap::new(
                    RadrootsPathProfile::InteractiveUser,
                    None,
                    RuntimeContextSource::SafeDefault,
                    RuntimeContextSource::BootstrapCli,
                )
                .expect("missing-directory bootstrap"),
                ServiceId::new("myc").expect("service"),
                InstanceId::new("primary").expect("instance"),
            ),
            Err(RuntimeContextError::PathSelection)
        );
    }

    #[test]
    fn retained_linux_xdg_vectors_are_exact_and_ignore_invalid_optional_values() {
        fn resolve(
            environment: RadrootsHostEnvironment,
        ) -> Result<RuntimeContext, RuntimeContextError> {
            RuntimeContext::resolve(
                &RadrootsPathResolver::new(RadrootsPlatform::Linux, environment),
                RuntimeContextBootstrap::new(
                    RadrootsPathProfile::InteractiveUser,
                    None,
                    RuntimeContextSource::SafeDefault,
                    RuntimeContextSource::BootstrapCli,
                )
                .expect("Linux bootstrap"),
                ServiceId::new("myc").expect("service"),
                InstanceId::new("primary").expect("instance"),
            )
        }

        let configured = resolve(RadrootsHostEnvironment {
            home_dir: Some(PathBuf::from("/home/treesap")),
            xdg_config_home: Some(PathBuf::from("/xdg/config")),
            xdg_data_home: Some(PathBuf::from("/xdg/data")),
            xdg_state_home: Some(PathBuf::from("/xdg/state")),
            xdg_cache_home: Some(PathBuf::from("/xdg/cache")),
            xdg_runtime_dir: Some(PathBuf::from("/xdg/run")),
            ..RadrootsHostEnvironment::default()
        })
        .expect("configured XDG context");
        assert_eq!(
            configured.paths().config(),
            PathBuf::from("/xdg/config/radroots/services/myc/primary")
        );
        assert_eq!(
            configured.paths().state(),
            PathBuf::from("/xdg/data/radroots/services/myc/primary")
        );
        assert_eq!(
            configured.paths().cache(),
            PathBuf::from("/xdg/cache/radroots/services/myc/primary")
        );
        assert_eq!(
            configured.paths().logs(),
            PathBuf::from("/xdg/state/radroots/logs/services/myc/primary")
        );
        assert_eq!(
            configured.paths().run(),
            PathBuf::from("/xdg/run/radroots/services/myc/primary")
        );
        assert_eq!(
            configured.paths().secrets(),
            PathBuf::from("/xdg/config/radroots/secrets/services/myc/primary")
        );

        for invalid in ["", "relative"] {
            let config = resolve(RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/home/treesap")),
                xdg_config_home: Some(PathBuf::from(invalid)),
                xdg_runtime_dir: Some(PathBuf::from("/run/user/1000")),
                ..RadrootsHostEnvironment::default()
            })
            .expect("invalid config override is ignored");
            assert_eq!(
                config.paths().config(),
                PathBuf::from("/home/treesap/.config/radroots/services/myc/primary")
            );

            let data = resolve(RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/home/treesap")),
                xdg_data_home: Some(PathBuf::from(invalid)),
                xdg_runtime_dir: Some(PathBuf::from("/run/user/1000")),
                ..RadrootsHostEnvironment::default()
            })
            .expect("invalid data override is ignored");
            assert_eq!(
                data.paths().state(),
                PathBuf::from("/home/treesap/.local/share/radroots/services/myc/primary")
            );

            let state = resolve(RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/home/treesap")),
                xdg_state_home: Some(PathBuf::from(invalid)),
                xdg_runtime_dir: Some(PathBuf::from("/run/user/1000")),
                ..RadrootsHostEnvironment::default()
            })
            .expect("invalid state override is ignored");
            assert_eq!(
                state.paths().logs(),
                PathBuf::from("/home/treesap/.local/state/radroots/logs/services/myc/primary")
            );

            let cache = resolve(RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/home/treesap")),
                xdg_cache_home: Some(PathBuf::from(invalid)),
                xdg_runtime_dir: Some(PathBuf::from("/run/user/1000")),
                ..RadrootsHostEnvironment::default()
            })
            .expect("invalid cache override is ignored");
            assert_eq!(
                cache.paths().cache(),
                PathBuf::from("/home/treesap/.cache/radroots/services/myc/primary")
            );

            assert_eq!(
                resolve(RadrootsHostEnvironment {
                    home_dir: Some(PathBuf::from("/home/treesap")),
                    xdg_runtime_dir: Some(PathBuf::from(invalid)),
                    ..RadrootsHostEnvironment::default()
                }),
                Err(RuntimeContextError::PathSelection)
            );
        }
    }

    #[test]
    fn context_is_equal_immutable_and_preserves_exact_typed_sources() {
        let first = repo_local_context(PathBuf::from("/repo/.local/radroots"));
        let second = repo_local_context(PathBuf::from("/repo/.local/radroots"));
        assert_eq!(first, second);
        assert_eq!(first.service().as_str(), "myc");
        assert_eq!(first.instance().as_str(), "primary");
        assert_eq!(first.profile(), RadrootsPathProfile::RepoLocal);
        assert_eq!(first.sources().service(), RuntimeContextSource::SafeDefault);
        assert_eq!(
            first.sources().instance(),
            RuntimeContextSource::BootstrapCli
        );
        assert_eq!(
            first.sources().profile(),
            RuntimeContextSource::BootstrapCli
        );
        assert_eq!(
            first.sources().repo_local_root(),
            Some(RuntimeContextSource::BootstrapCli)
        );
        assert_eq!(first.sources().paths(), RuntimeContextSource::DerivedPath);
        assert_eq!(
            first.paths().config(),
            PathBuf::from("/repo/.local/radroots/config/services/myc/primary")
        );

        let defaulted = RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::ServiceHost,
                None,
                RuntimeContextSource::SafeDefault,
                RuntimeContextSource::SafeDefault,
            )
            .expect("default bootstrap"),
            ServiceId::new("rhi").expect("service"),
            InstanceId::new("default").expect("instance"),
        )
        .expect("default runtime context");
        assert_eq!(
            defaulted.sources().profile(),
            RuntimeContextSource::SafeDefault
        );
        assert_eq!(
            defaulted.sources().instance(),
            RuntimeContextSource::SafeDefault
        );
        assert_eq!(defaulted.sources().repo_local_root(), None);
        assert_eq!(defaulted.repo_local_root(), None);
    }

    #[test]
    fn serialization_and_debug_redact_paths_and_use_only_closed_sources() {
        let bootstrap = RuntimeContextBootstrap::new(
            RadrootsPathProfile::RepoLocal,
            Some(PathBuf::from("/sensitive/project-root")),
            RuntimeContextSource::BootstrapCli,
            RuntimeContextSource::BootstrapCli,
        )
        .expect("bootstrap");
        assert_eq!(bootstrap.profile(), RadrootsPathProfile::RepoLocal);
        let bootstrap_debug = format!("{bootstrap:?}");
        assert!(bootstrap_debug.contains("repo_local_root: Some(\"[redacted]\")"));
        let context = repo_local_context(PathBuf::from("/sensitive/project-root"));
        let serialized = serde_json::to_value(&context).expect("serialize context");
        assert_eq!(
            serialized,
            json!({
                "service": "myc",
                "instance": "primary",
                "profile": "repo_local",
                "paths": "[redacted]",
                "sources": {
                    "service": "safe_default",
                    "instance": "bootstrap_cli",
                    "profile": "bootstrap_cli",
                    "repo_local_root": "bootstrap_cli",
                    "paths": "derived_path"
                }
            })
        );
        assert_eq!(
            serde_json::to_value([
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::Toml,
                RuntimeContextSource::SafeDefault,
                RuntimeContextSource::DerivedPath,
            ])
            .expect("source inventory"),
            json!(["bootstrap_cli", "toml", "safe_default", "derived_path"])
        );
        let debug = format!("{context:?}");
        assert!(debug.contains("paths: \"[redacted]\""));
        for forbidden in [
            "/sensitive",
            "project-root",
            "/config/",
            "/run/",
            "secret:caller-controlled-value",
            "0123456789abcdef0123456789abcdef",
        ] {
            assert!(!serialized.to_string().contains(forbidden));
            assert!(!debug.contains(forbidden));
            assert!(!bootstrap_debug.contains(forbidden));
        }
    }

    #[test]
    fn construction_performs_no_directory_file_or_ambient_bootstrap_io() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "radroots-runtime-context-no-io-{}-{unique}",
            std::process::id()
        ));
        assert!(!base.exists(), "unique test base unexpectedly exists");
        let context = repo_local_context(base.clone());
        assert_eq!(
            context.paths().state(),
            base.join("data/services/myc/primary")
        );
        assert!(!base.exists(), "context construction created the base");

        let production = include_str!("context.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production source");
        for forbidden in [
            "std::fs",
            "create_dir",
            "create_file",
            "OpenOptions",
            "std::env",
        ] {
            assert!(
                !production.contains(forbidden),
                "context production source contains forbidden I/O `{forbidden}`"
            );
        }
    }

    #[test]
    fn typed_bootstrap_rejects_every_mismatched_provenance_combination() {
        for source in [
            RuntimeContextSource::Toml,
            RuntimeContextSource::DerivedPath,
        ] {
            assert_eq!(
                RuntimeContextBootstrap::new(
                    RadrootsPathProfile::ServiceHost,
                    None,
                    source,
                    RuntimeContextSource::BootstrapCli,
                ),
                Err(RuntimeContextError::InvalidBootstrapBinding)
            );
            assert_eq!(
                RuntimeContextBootstrap::new(
                    RadrootsPathProfile::ServiceHost,
                    None,
                    RuntimeContextSource::SafeDefault,
                    source,
                ),
                Err(RuntimeContextError::InvalidBootstrapBinding)
            );
        }
        assert_eq!(
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                None,
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            ),
            Err(RuntimeContextError::InvalidBootstrapBinding)
        );
        assert_eq!(
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                Some(PathBuf::from("/repo/.local/radroots")),
                RuntimeContextSource::SafeDefault,
                RuntimeContextSource::SafeDefault,
            ),
            Err(RuntimeContextError::InvalidBootstrapBinding)
        );
        assert_eq!(
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::ServiceHost,
                Some(PathBuf::from("/repo/.local/radroots")),
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            ),
            Err(RuntimeContextError::InvalidBootstrapBinding)
        );
        assert_eq!(
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::MobileNative,
                None,
                RuntimeContextSource::SafeDefault,
                RuntimeContextSource::SafeDefault,
            ),
            Err(RuntimeContextError::InvalidBootstrapBinding)
        );
    }
}

//! Immutable resolved bootstrap context for one service instance.

use core::fmt;
use std::path::PathBuf;

use serde::{Serialize, Serializer, ser::SerializeStruct};
use thiserror::Error;

use crate::{
    InstanceId, RadrootsPathOverrides, RadrootsPathProfile, RadrootsPathResolver,
    RadrootsServiceInstancePaths, ServiceId, default_service_instance_paths,
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
///     paths: todo!(),
///     sources: todo!(),
/// };
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct RuntimeContext {
    service: ServiceId,
    instance: InstanceId,
    profile: RadrootsPathProfile,
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
        let overrides = match bootstrap.repo_local_root.as_ref() {
            Some(root) => RadrootsPathOverrides::repo_local(root),
            None => RadrootsPathOverrides::default(),
        };
        let paths = default_service_instance_paths(
            resolver,
            bootstrap.profile,
            &overrides,
            &service,
            &instance,
        )
        .map_err(|_| RuntimeContextError::PathSelection)?;
        let sources = RuntimeContextSources {
            service: RuntimeContextSource::SafeDefault,
            instance: bootstrap.instance_source,
            profile: bootstrap.profile_source,
            repo_local_root: bootstrap
                .repo_local_root
                .as_ref()
                .map(|_| RuntimeContextSource::BootstrapCli),
            paths: RuntimeContextSource::DerivedPath,
        };

        Ok(Self {
            service,
            instance,
            profile: bootstrap.profile,
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
        RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                Some(base),
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("bootstrap"),
            ServiceId::new("myc").expect("service"),
            InstanceId::new("primary").expect("instance"),
        )
        .expect("runtime context")
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

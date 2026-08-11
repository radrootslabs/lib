use std::{
    fmt,
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::{
    InstanceId, RadrootsPathOverrides, RadrootsPathProfile, RadrootsPathResolver,
    RadrootsRuntimeNamespace, RadrootsRuntimePathsError, RadrootsServiceInstanceNamespace,
    RadrootsServiceInstancePaths, ServiceId,
};

pub const DEFAULT_CONFIG_FILE_NAME: &str = "config.toml";
pub const SERVICE_STATE_DATABASE_FILE_NAME: &str = "state.sqlite";
pub const SERVICE_STATE_LOCK_FILE_NAME: &str = "state.lock";
pub const SERVICE_ADMIN_SOCKET_FILE_NAME: &str = "admin.sock";
pub const SERVICE_CREDENTIAL_ARTIFACT_NAME_MAX_BYTES: usize = 128;
pub const DEFAULT_SERVICE_IDENTITY_FILE_NAME: &str = "identity.secret.json";
pub const DEFAULT_SHARED_IDENTITY_FILE_NAME: &str = "default.json";
pub const DEFAULT_SHARED_GEONAMES_NAMESPACE_KIND: &str = "shared";
pub const DEFAULT_SHARED_GEONAMES_NAMESPACE_VALUE: &str = "geonames";
pub const DEFAULT_SHARED_GEONAMES_NAMESPACE: &str = "shared/geonames";
pub const DEFAULT_SHARED_RUNTIME_STORE_NAMESPACE_KIND: &str = "shared";
pub const DEFAULT_SHARED_RUNTIME_STORE_NAMESPACE_VALUE: &str = "runtime_store";
pub const DEFAULT_SHARED_RUNTIME_STORE_NAMESPACE: &str = "shared/runtime_store";
pub const DEFAULT_SHARED_RUNTIME_STORE_DB_FILE_NAME: &str = "runtime_store.sqlite";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsBootstrapPaths {
    pub config_path: PathBuf,
    pub logs_dir: PathBuf,
    pub identity_path: PathBuf,
}

/// A validated service-owned credential artifact filename.
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServiceCredentialArtifactName(String);

impl ServiceCredentialArtifactName {
    pub fn new(value: impl Into<String>) -> Result<Self, ServiceCredentialArtifactNameError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ServiceCredentialArtifactNameError::Empty);
        }
        if value.len() > SERVICE_CREDENTIAL_ARTIFACT_NAME_MAX_BYTES {
            return Err(ServiceCredentialArtifactNameError::TooLong {
                maximum: SERVICE_CREDENTIAL_ARTIFACT_NAME_MAX_BYTES,
            });
        }
        let bytes = value.as_bytes();
        let is_alphanumeric = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
        if !is_alphanumeric(bytes[0]) || !is_alphanumeric(bytes[bytes.len() - 1]) {
            return Err(ServiceCredentialArtifactNameError::InvalidBoundary);
        }
        if !bytes
            .iter()
            .all(|byte| is_alphanumeric(*byte) || matches!(*byte, b'.' | b'-' | b'_'))
        {
            return Err(ServiceCredentialArtifactNameError::InvalidCharacter);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<str> for ServiceCredentialArtifactName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Debug for ServiceCredentialArtifactName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ServiceCredentialArtifactName([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ServiceCredentialArtifactNameError {
    #[error("service credential artifact name must not be empty")]
    Empty,
    #[error("service credential artifact name exceeds its {maximum}-byte limit")]
    TooLong { maximum: usize },
    #[error(
        "service credential artifact name must start and end with a lowercase ASCII letter or digit"
    )]
    InvalidBoundary,
    #[error("service credential artifact name contains a forbidden character")]
    InvalidCharacter,
}

/// Fixed common artifacts derived for one validated service instance.
///
/// Callers cannot override the fixed artifact filenames or roots:
///
/// ```compile_fail
/// use radroots_runtime_paths::RadrootsServiceInstanceArtifacts;
///
/// let _ = RadrootsServiceInstanceArtifacts {
///     config: "/tmp/alternate.toml".into(),
///     state_database: "/tmp/alternate.sqlite".into(),
///     state_lock: "/tmp/alternate.lock".into(),
///     admin_socket: "/tmp/alternate.sock".into(),
/// };
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct RadrootsServiceInstanceArtifacts {
    config: PathBuf,
    state_database: PathBuf,
    state_lock: PathBuf,
    admin_socket: PathBuf,
}

impl fmt::Debug for RadrootsServiceInstanceArtifacts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RadrootsServiceInstanceArtifacts([redacted])")
    }
}

impl RadrootsServiceInstanceArtifacts {
    fn from_instance_paths(paths: &RadrootsServiceInstancePaths) -> Self {
        Self {
            config: paths.config().join(DEFAULT_CONFIG_FILE_NAME),
            state_database: paths.state().join(SERVICE_STATE_DATABASE_FILE_NAME),
            state_lock: paths.state().join(SERVICE_STATE_LOCK_FILE_NAME),
            admin_socket: paths.run().join(SERVICE_ADMIN_SOCKET_FILE_NAME),
        }
    }

    #[must_use]
    pub fn config(&self) -> &Path {
        &self.config
    }

    #[must_use]
    pub fn state_database(&self) -> &Path {
        &self.state_database
    }

    #[must_use]
    pub fn state_lock(&self) -> &Path {
        &self.state_lock
    }

    #[must_use]
    pub fn admin_socket(&self) -> &Path {
        &self.admin_socket
    }
}

#[must_use]
pub fn default_service_instance_artifacts(
    paths: &RadrootsServiceInstancePaths,
) -> RadrootsServiceInstanceArtifacts {
    RadrootsServiceInstanceArtifacts::from_instance_paths(paths)
}

#[must_use]
pub fn service_credential_artifact_path(
    paths: &RadrootsServiceInstancePaths,
    name: &ServiceCredentialArtifactName,
) -> PathBuf {
    paths.secrets().join(name.as_str())
}

pub fn default_service_instance_paths(
    resolver: &RadrootsPathResolver,
    profile: RadrootsPathProfile,
    overrides: &RadrootsPathOverrides,
    service_id: &ServiceId,
    instance_id: &InstanceId,
) -> Result<RadrootsServiceInstancePaths, RadrootsRuntimePathsError> {
    let namespace = RadrootsServiceInstanceNamespace::new(service_id.clone(), instance_id.clone());
    let roots = resolver.resolve(profile, overrides)?;
    Ok(RadrootsServiceInstancePaths::from_resolved_roots(
        &roots, &namespace,
    ))
}

pub fn default_namespaced_bootstrap_paths(
    resolver: &RadrootsPathResolver,
    profile: RadrootsPathProfile,
    overrides: &RadrootsPathOverrides,
    namespace: &RadrootsRuntimeNamespace,
    identity_file_name: &str,
) -> Result<RadrootsBootstrapPaths, RadrootsRuntimePathsError> {
    let namespaced = resolver.resolve(profile, overrides)?.namespaced(namespace);
    Ok(RadrootsBootstrapPaths {
        config_path: namespaced.config.join(DEFAULT_CONFIG_FILE_NAME),
        logs_dir: namespaced.logs,
        identity_path: namespaced.secrets.join(identity_file_name),
    })
}

pub fn default_shared_identity_path(
    resolver: &RadrootsPathResolver,
    profile: RadrootsPathProfile,
    overrides: &RadrootsPathOverrides,
) -> Result<PathBuf, RadrootsRuntimePathsError> {
    let namespace = RadrootsRuntimeNamespace::shared("identities")?;
    let namespaced = resolver.resolve(profile, overrides)?.namespaced(&namespace);
    Ok(namespaced.secrets.join(DEFAULT_SHARED_IDENTITY_FILE_NAME))
}

pub fn default_shared_runtime_logs_dir(
    resolver: &RadrootsPathResolver,
    profile: RadrootsPathProfile,
    overrides: &RadrootsPathOverrides,
) -> Result<PathBuf, RadrootsRuntimePathsError> {
    let namespace = RadrootsRuntimeNamespace::shared("runtime")?;
    let namespaced = resolver.resolve(profile, overrides)?.namespaced(&namespace);
    Ok(namespaced.logs)
}

#[must_use]
pub fn default_shared_runtime_store_root_from_data_root(data_root: impl AsRef<Path>) -> PathBuf {
    data_root
        .as_ref()
        .join(DEFAULT_SHARED_RUNTIME_STORE_NAMESPACE_KIND)
        .join(DEFAULT_SHARED_RUNTIME_STORE_NAMESPACE_VALUE)
}

#[must_use]
pub fn default_shared_runtime_store_database_path_from_data_root(
    data_root: impl AsRef<Path>,
) -> PathBuf {
    default_shared_runtime_store_root_from_data_root(data_root)
        .join(DEFAULT_SHARED_RUNTIME_STORE_DB_FILE_NAME)
}

#[must_use]
pub fn default_shared_geonames_root_from_cache_root(cache_root: impl AsRef<Path>) -> PathBuf {
    cache_root
        .as_ref()
        .join(DEFAULT_SHARED_GEONAMES_NAMESPACE_KIND)
        .join(DEFAULT_SHARED_GEONAMES_NAMESPACE_VALUE)
}

#[must_use]
pub fn default_shared_geonames_database_file_name(version: &str) -> String {
    format!("geonames-{version}.db")
}

#[must_use]
pub fn default_shared_geonames_database_path_from_cache_root(
    cache_root: impl AsRef<Path>,
    version: &str,
) -> PathBuf {
    default_shared_geonames_root_from_cache_root(cache_root)
        .join(default_shared_geonames_database_file_name(version))
}

pub fn default_shared_runtime_store_root_from_shared_accounts_data_root(
    shared_accounts_data_root: impl AsRef<Path>,
) -> Result<PathBuf, RadrootsRuntimePathsError> {
    let shared_accounts_data_root = shared_accounts_data_root.as_ref();
    let shared_data_root = shared_accounts_data_root.parent().ok_or_else(|| {
        RadrootsRuntimePathsError::SharedAccountsDataRootMissingParent {
            path: shared_accounts_data_root.to_path_buf(),
        }
    })?;
    Ok(shared_data_root.join(DEFAULT_SHARED_RUNTIME_STORE_NAMESPACE_VALUE))
}

pub fn default_shared_runtime_store_database_path_from_shared_accounts_data_root(
    shared_accounts_data_root: impl AsRef<Path>,
) -> Result<PathBuf, RadrootsRuntimePathsError> {
    default_shared_runtime_store_root_from_shared_accounts_data_root(shared_accounts_data_root)
        .map(|root| root.join(DEFAULT_SHARED_RUNTIME_STORE_DB_FILE_NAME))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{
        RadrootsHostEnvironment, RadrootsPlatform, RadrootsRuntimeNamespace,
        RadrootsRuntimePathsError,
    };

    use super::{
        DEFAULT_CONFIG_FILE_NAME, DEFAULT_SERVICE_IDENTITY_FILE_NAME,
        DEFAULT_SHARED_GEONAMES_NAMESPACE, DEFAULT_SHARED_IDENTITY_FILE_NAME,
        DEFAULT_SHARED_RUNTIME_STORE_DB_FILE_NAME, DEFAULT_SHARED_RUNTIME_STORE_NAMESPACE,
        SERVICE_ADMIN_SOCKET_FILE_NAME, SERVICE_CREDENTIAL_ARTIFACT_NAME_MAX_BYTES,
        SERVICE_STATE_DATABASE_FILE_NAME, SERVICE_STATE_LOCK_FILE_NAME,
        ServiceCredentialArtifactName, ServiceCredentialArtifactNameError,
        default_namespaced_bootstrap_paths, default_service_instance_artifacts,
        default_service_instance_paths, default_shared_geonames_database_file_name,
        default_shared_geonames_database_path_from_cache_root,
        default_shared_geonames_root_from_cache_root, default_shared_identity_path,
        default_shared_runtime_logs_dir, default_shared_runtime_store_database_path_from_data_root,
        default_shared_runtime_store_database_path_from_shared_accounts_data_root,
        default_shared_runtime_store_root_from_data_root,
        default_shared_runtime_store_root_from_shared_accounts_data_root,
        service_credential_artifact_path,
    };
    use crate::{InstanceId, ServiceId};

    #[test]
    fn service_instance_paths_are_exact_for_linux_and_xdg_profiles() {
        let service_id = ServiceId::new("myc").expect("service id");
        let instance_id = InstanceId::new("primary").expect("instance id");
        let service_host = crate::RadrootsPathResolver::new(
            RadrootsPlatform::Linux,
            RadrootsHostEnvironment::default(),
        );
        let service_paths = default_service_instance_paths(
            &service_host,
            crate::RadrootsPathProfile::ServiceHost,
            &crate::RadrootsPathOverrides::default(),
            &service_id,
            &instance_id,
        )
        .expect("service-host paths");
        assert_eq!(
            service_paths.config(),
            PathBuf::from("/etc/radroots/services/myc/primary")
        );
        assert_eq!(
            service_paths.state(),
            PathBuf::from("/var/lib/radroots/services/myc/primary")
        );
        assert_eq!(
            service_paths.cache(),
            PathBuf::from("/var/cache/radroots/services/myc/primary")
        );
        assert_eq!(
            service_paths.logs(),
            PathBuf::from("/var/log/radroots/services/myc/primary")
        );
        assert_eq!(
            service_paths.run(),
            PathBuf::from("/run/radroots/services/myc/primary")
        );
        assert_eq!(
            service_paths.secrets(),
            PathBuf::from("/etc/radroots/secrets/services/myc/primary")
        );

        let interactive = crate::RadrootsPathResolver::new(
            RadrootsPlatform::Linux,
            RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/home/treesap")),
                xdg_runtime_dir: Some(PathBuf::from("/run/user/1000")),
                ..RadrootsHostEnvironment::default()
            },
        );
        let interactive_paths = default_service_instance_paths(
            &interactive,
            crate::RadrootsPathProfile::InteractiveUser,
            &crate::RadrootsPathOverrides::default(),
            &service_id,
            &instance_id,
        )
        .expect("XDG paths");
        assert_eq!(
            interactive_paths.config(),
            PathBuf::from("/home/treesap/.config/radroots/services/myc/primary")
        );
        assert_eq!(
            interactive_paths.state(),
            PathBuf::from("/home/treesap/.local/share/radroots/services/myc/primary")
        );
        assert_eq!(
            interactive_paths.cache(),
            PathBuf::from("/home/treesap/.cache/radroots/services/myc/primary")
        );
        assert_eq!(
            interactive_paths.logs(),
            PathBuf::from("/home/treesap/.local/state/radroots/logs/services/myc/primary")
        );
        assert_eq!(
            interactive_paths.run(),
            PathBuf::from("/run/user/1000/radroots/services/myc/primary")
        );
        assert_eq!(
            interactive_paths.secrets(),
            PathBuf::from("/home/treesap/.config/radroots/secrets/services/myc/primary")
        );
    }

    #[test]
    fn service_instance_artifacts_use_only_fixed_common_filenames() {
        assert_eq!(DEFAULT_CONFIG_FILE_NAME, "config.toml");
        assert_eq!(SERVICE_STATE_DATABASE_FILE_NAME, "state.sqlite");
        assert_eq!(SERVICE_STATE_LOCK_FILE_NAME, "state.lock");
        assert_eq!(SERVICE_ADMIN_SOCKET_FILE_NAME, "admin.sock");

        let resolver = crate::RadrootsPathResolver::new(
            RadrootsPlatform::Linux,
            RadrootsHostEnvironment::default(),
        );
        let paths = default_service_instance_paths(
            &resolver,
            crate::RadrootsPathProfile::ServiceHost,
            &crate::RadrootsPathOverrides::default(),
            &ServiceId::new("myc").expect("service id"),
            &InstanceId::new("primary").expect("instance id"),
        )
        .expect("instance paths");
        let artifacts = default_service_instance_artifacts(&paths);

        assert_eq!(
            artifacts.config(),
            PathBuf::from("/etc/radroots/services/myc/primary/config.toml")
        );
        assert_eq!(
            artifacts.state_database(),
            PathBuf::from("/var/lib/radroots/services/myc/primary/state.sqlite")
        );
        assert_eq!(
            artifacts.state_lock(),
            PathBuf::from("/var/lib/radroots/services/myc/primary/state.lock")
        );
        assert_eq!(
            artifacts.admin_socket(),
            PathBuf::from("/run/radroots/services/myc/primary/admin.sock")
        );
    }

    #[test]
    fn credential_artifact_names_are_validated_and_remain_outside_state() {
        let resolver = crate::RadrootsPathResolver::new(
            RadrootsPlatform::Linux,
            RadrootsHostEnvironment::default(),
        );
        let paths = default_service_instance_paths(
            &resolver,
            crate::RadrootsPathProfile::ServiceHost,
            &crate::RadrootsPathOverrides::default(),
            &ServiceId::new("rhi").expect("service id"),
            &InstanceId::new("default").expect("instance id"),
        )
        .expect("instance paths");
        let name =
            ServiceCredentialArtifactName::new("identity.secret.json").expect("credential name");
        assert_eq!(
            format!("{name:?}"),
            "ServiceCredentialArtifactName([redacted])"
        );
        assert!(!format!("{name:?}").contains("identity.secret.json"));
        let credential = service_credential_artifact_path(&paths, &name);
        assert_eq!(
            credential,
            PathBuf::from("/etc/radroots/secrets/services/rhi/default/identity.secret.json")
        );
        assert!(!credential.starts_with(paths.state()));

        let artifacts = default_service_instance_artifacts(&paths);
        let artifacts_debug = format!("{artifacts:?}");
        assert_eq!(
            artifacts_debug,
            "RadrootsServiceInstanceArtifacts([redacted])"
        );
        assert!(!artifacts_debug.contains("/etc/radroots"));
        assert!(!artifacts_debug.contains("/var/lib/radroots"));
        assert!(!artifacts_debug.contains("/run/radroots"));

        let maximum_name = "a".repeat(SERVICE_CREDENTIAL_ARTIFACT_NAME_MAX_BYTES);
        assert_eq!(
            ServiceCredentialArtifactName::new(maximum_name.clone())
                .expect("maximum credential name")
                .as_str(),
            maximum_name
        );

        assert_eq!(
            ServiceCredentialArtifactName::new(""),
            Err(ServiceCredentialArtifactNameError::Empty)
        );
        assert_eq!(
            ServiceCredentialArtifactName::new(
                "a".repeat(SERVICE_CREDENTIAL_ARTIFACT_NAME_MAX_BYTES + 1)
            ),
            Err(ServiceCredentialArtifactNameError::TooLong {
                maximum: SERVICE_CREDENTIAL_ARTIFACT_NAME_MAX_BYTES,
            })
        );
        for invalid in [
            ".",
            "..",
            "../identity",
            "/identity",
            "identity/secret",
            r"identity\secret",
            "Identity",
            "identity secret",
            "identity-秘密",
        ] {
            assert!(
                ServiceCredentialArtifactName::new(invalid).is_err(),
                "accepted invalid credential artifact name `{invalid}`"
            );
        }
    }

    #[test]
    fn namespaced_bootstrap_paths_use_canonical_interactive_roots() {
        let resolver = crate::RadrootsPathResolver::new(
            RadrootsPlatform::Linux,
            RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/home/treesap")),
                xdg_runtime_dir: Some(PathBuf::from("/run/user/1000")),
                ..RadrootsHostEnvironment::default()
            },
        );
        let namespace =
            RadrootsRuntimeNamespace::service("radrootsd").expect("service namespace should parse");

        let paths = default_namespaced_bootstrap_paths(
            &resolver,
            crate::RadrootsPathProfile::InteractiveUser,
            &crate::RadrootsPathOverrides::default(),
            &namespace,
            DEFAULT_SERVICE_IDENTITY_FILE_NAME,
        )
        .expect("service bootstrap paths should resolve");

        assert_eq!(
            paths.config_path,
            PathBuf::from("/home/treesap/.config/radroots/services/radrootsd/config.toml")
        );
        assert_eq!(
            paths.logs_dir,
            PathBuf::from("/home/treesap/.local/state/radroots/logs/services/radrootsd")
        );
        assert_eq!(
            paths.identity_path,
            PathBuf::from(
                "/home/treesap/.config/radroots/secrets/services/radrootsd/identity.secret.json"
            )
        );
    }

    #[test]
    fn shared_defaults_use_shared_namespaces() {
        let resolver = crate::RadrootsPathResolver::new(
            RadrootsPlatform::Macos,
            RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/Users/treesap")),
                ..RadrootsHostEnvironment::default()
            },
        );

        let identity_path = default_shared_identity_path(
            &resolver,
            crate::RadrootsPathProfile::InteractiveUser,
            &crate::RadrootsPathOverrides::default(),
        )
        .expect("shared identity path should resolve");
        assert_eq!(
            identity_path,
            PathBuf::from(
                "/Users/treesap/Library/Application Support/Radroots/secrets/shared/identities"
            )
            .join(DEFAULT_SHARED_IDENTITY_FILE_NAME)
        );

        let logs_dir = default_shared_runtime_logs_dir(
            &resolver,
            crate::RadrootsPathProfile::InteractiveUser,
            &crate::RadrootsPathOverrides::default(),
        )
        .expect("shared runtime logs dir should resolve");
        assert_eq!(
            logs_dir,
            PathBuf::from("/Users/treesap/Library/Logs/Radroots/shared/runtime")
        );
    }

    #[test]
    fn shared_runtime_store_paths_use_canonical_shared_namespace() {
        let data_root = PathBuf::from("/repo/infra/local/runtime/radroots/data");

        assert_eq!(
            default_shared_runtime_store_root_from_data_root(&data_root),
            data_root.join(DEFAULT_SHARED_RUNTIME_STORE_NAMESPACE)
        );
        assert_eq!(
            default_shared_runtime_store_database_path_from_data_root(&data_root),
            data_root
                .join(DEFAULT_SHARED_RUNTIME_STORE_NAMESPACE)
                .join(DEFAULT_SHARED_RUNTIME_STORE_DB_FILE_NAME)
        );
    }

    #[test]
    fn shared_geonames_paths_use_canonical_shared_cache_namespace() {
        let cache_root = PathBuf::from("/repo/infra/local/runtime/radroots/cache");

        assert_eq!(
            default_shared_geonames_root_from_cache_root(&cache_root),
            cache_root.join(DEFAULT_SHARED_GEONAMES_NAMESPACE)
        );
        assert_eq!(
            default_shared_geonames_database_file_name("1.0"),
            "geonames-1.0.db"
        );
        assert_eq!(
            default_shared_geonames_database_path_from_cache_root(&cache_root, "1.0"),
            cache_root
                .join(DEFAULT_SHARED_GEONAMES_NAMESPACE)
                .join("geonames-1.0.db")
        );
    }

    #[test]
    fn shared_runtime_store_paths_derive_from_shared_accounts_data_root() {
        let shared_accounts_data_root =
            PathBuf::from("/repo/infra/local/runtime/radroots/data/shared/accounts");

        assert_eq!(
            default_shared_runtime_store_root_from_shared_accounts_data_root(
                &shared_accounts_data_root
            )
            .expect("shared runtime-store root"),
            PathBuf::from("/repo/infra/local/runtime/radroots/data/shared/runtime_store")
        );
        assert_eq!(
            default_shared_runtime_store_root_from_shared_accounts_data_root(
                shared_accounts_data_root.clone()
            )
            .expect("shared runtime-store root from owned path"),
            PathBuf::from("/repo/infra/local/runtime/radroots/data/shared/runtime_store")
        );
        assert_eq!(
            default_shared_runtime_store_database_path_from_shared_accounts_data_root(
                &shared_accounts_data_root
            )
            .expect("shared runtime-store database path"),
            PathBuf::from(
                "/repo/infra/local/runtime/radroots/data/shared/runtime_store/runtime_store.sqlite"
            )
        );

        let err =
            default_shared_runtime_store_root_from_shared_accounts_data_root(PathBuf::from("/"))
                .expect_err("root path has no parent shared data root");
        assert_eq!(
            err,
            RadrootsRuntimePathsError::SharedAccountsDataRootMissingParent {
                path: PathBuf::from("/")
            }
        );
    }

    #[test]
    fn namespaced_bootstrap_paths_propagate_resolver_errors() {
        let resolver =
            crate::RadrootsPathResolver::new(RadrootsPlatform::Linux, Default::default());
        let namespace =
            RadrootsRuntimeNamespace::service("radrootsd").expect("service namespace should parse");

        let err = default_namespaced_bootstrap_paths(
            &resolver,
            crate::RadrootsPathProfile::InteractiveUser,
            &crate::RadrootsPathOverrides::default(),
            &namespace,
            DEFAULT_SERVICE_IDENTITY_FILE_NAME,
        )
        .expect_err("interactive user should require a home dir");

        assert_eq!(
            err,
            crate::RadrootsRuntimePathsError::MissingHomeDir {
                platform: RadrootsPlatform::Linux,
            }
        );
    }

    #[test]
    fn shared_defaults_propagate_profile_errors() {
        let resolver =
            crate::RadrootsPathResolver::new(RadrootsPlatform::Android, Default::default());

        let identity_err = default_shared_identity_path(
            &resolver,
            crate::RadrootsPathProfile::InteractiveUser,
            &crate::RadrootsPathOverrides::default(),
        )
        .expect_err("interactive_user should be unsupported on android");
        assert_eq!(
            identity_err,
            crate::RadrootsRuntimePathsError::UnsupportedProfilePlatform {
                profile: crate::RadrootsPathProfile::InteractiveUser,
                platform: RadrootsPlatform::Android,
            }
        );

        let logs_err = default_shared_runtime_logs_dir(
            &resolver,
            crate::RadrootsPathProfile::ServiceHost,
            &crate::RadrootsPathOverrides::default(),
        )
        .expect_err("service_host should be unsupported on android");
        assert_eq!(
            logs_err,
            crate::RadrootsRuntimePathsError::UnsupportedProfilePlatform {
                profile: crate::RadrootsPathProfile::ServiceHost,
                platform: RadrootsPlatform::Android,
            }
        );
    }
}

use std::path::PathBuf;

use crate::{
    RadrootsPathOverrides, RadrootsPathProfile, RadrootsPathResolver, RadrootsRuntimeNamespace,
    RadrootsRuntimePathsError,
};

pub const DEFAULT_CONFIG_FILE_NAME: &str = "config.toml";
pub const DEFAULT_SERVICE_IDENTITY_FILE_NAME: &str = "identity.secret.json";
pub const DEFAULT_SHARED_IDENTITY_FILE_NAME: &str = "default.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsBootstrapPaths {
    pub config_path: PathBuf,
    pub logs_dir: PathBuf,
    pub identity_path: PathBuf,
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{RadrootsHostEnvironment, RadrootsPlatform, RadrootsRuntimeNamespace};

    use super::{
        DEFAULT_SERVICE_IDENTITY_FILE_NAME, DEFAULT_SHARED_IDENTITY_FILE_NAME,
        default_namespaced_bootstrap_paths, default_shared_identity_path,
        default_shared_runtime_logs_dir,
    };

    #[test]
    fn namespaced_bootstrap_paths_use_canonical_interactive_roots() {
        let resolver = crate::RadrootsPathResolver::new(
            RadrootsPlatform::Linux,
            RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/home/treesap")),
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
            PathBuf::from("/home/treesap/.radroots/config/services/radrootsd/config.toml")
        );
        assert_eq!(
            paths.logs_dir,
            PathBuf::from("/home/treesap/.radroots/logs/services/radrootsd")
        );
        assert_eq!(
            paths.identity_path,
            PathBuf::from(
                "/home/treesap/.radroots/secrets/services/radrootsd/identity.secret.json"
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
            PathBuf::from("/Users/treesap/.radroots/secrets/shared/identities")
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
            PathBuf::from("/Users/treesap/.radroots/logs/shared/runtime")
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

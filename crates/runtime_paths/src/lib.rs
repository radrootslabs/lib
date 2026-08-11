#![forbid(unsafe_code)]

pub mod conventions;
pub mod error;
pub mod identifier;
pub mod namespace;
pub mod platform;
pub mod roots;
pub mod service;

pub use conventions::{
    DEFAULT_CONFIG_FILE_NAME, DEFAULT_SERVICE_IDENTITY_FILE_NAME,
    DEFAULT_SHARED_GEONAMES_NAMESPACE, DEFAULT_SHARED_GEONAMES_NAMESPACE_KIND,
    DEFAULT_SHARED_GEONAMES_NAMESPACE_VALUE, DEFAULT_SHARED_IDENTITY_FILE_NAME,
    DEFAULT_SHARED_RUNTIME_STORE_DB_FILE_NAME, DEFAULT_SHARED_RUNTIME_STORE_NAMESPACE,
    DEFAULT_SHARED_RUNTIME_STORE_NAMESPACE_KIND, DEFAULT_SHARED_RUNTIME_STORE_NAMESPACE_VALUE,
    RadrootsBootstrapPaths, RadrootsServiceInstanceArtifacts, SERVICE_ADMIN_SOCKET_FILE_NAME,
    SERVICE_CREDENTIAL_ARTIFACT_NAME_MAX_BYTES, SERVICE_STATE_DATABASE_FILE_NAME,
    SERVICE_STATE_LOCK_FILE_NAME, ServiceCredentialArtifactName,
    ServiceCredentialArtifactNameError, default_namespaced_bootstrap_paths,
    default_service_instance_artifacts, default_service_instance_paths,
    default_shared_geonames_database_file_name,
    default_shared_geonames_database_path_from_cache_root,
    default_shared_geonames_root_from_cache_root, default_shared_identity_path,
    default_shared_runtime_logs_dir, default_shared_runtime_store_database_path_from_data_root,
    default_shared_runtime_store_database_path_from_shared_accounts_data_root,
    default_shared_runtime_store_root_from_data_root,
    default_shared_runtime_store_root_from_shared_accounts_data_root,
    service_credential_artifact_path,
};
pub use error::RadrootsRuntimePathsError;
pub use identifier::{
    INSTANCE_ID_MAX_BYTES, InstanceId, SERVICE_ID_MAX_BYTES, ServiceId, ServiceIdentityError,
    ServiceIdentityKind,
};
pub use namespace::{
    RadrootsRuntimeNamespace, RadrootsRuntimeNamespaceKind, RadrootsServiceInstanceNamespace,
};
pub use platform::{RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPlatform};
pub use roots::{RadrootsPathOverrides, RadrootsPathResolver, RadrootsPaths};
pub use service::{
    RadrootsRuntimePathConfigEntry, RadrootsRuntimePathPolicyContract,
    RadrootsRuntimePathSelection, RadrootsRuntimePathSelectionError,
    RadrootsRuntimeSelectionContract, RadrootsRuntimeSelectionOverrideContract,
    RadrootsServiceInstancePaths,
};

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        RadrootsHostEnvironment, RadrootsPathOverrides, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPaths, RadrootsPlatform, RadrootsRuntimeNamespace, RadrootsRuntimePathsError,
    };

    #[test]
    fn interactive_user_linux_uses_xdg_defaults_and_explicit_runtime() {
        let resolver = RadrootsPathResolver::new(
            RadrootsPlatform::Linux,
            RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/home/treesap")),
                xdg_runtime_dir: Some(PathBuf::from("/run/user/1000")),
                ..RadrootsHostEnvironment::default()
            },
        );

        let roots = resolver
            .resolve(
                RadrootsPathProfile::InteractiveUser,
                &RadrootsPathOverrides::default(),
            )
            .expect("resolve linux interactive roots");

        assert_eq!(
            roots.config,
            PathBuf::from("/home/treesap/.config/radroots")
        );
        assert_eq!(
            roots.data,
            PathBuf::from("/home/treesap/.local/share/radroots")
        );
        assert_eq!(
            roots.logs,
            PathBuf::from("/home/treesap/.local/state/radroots/logs")
        );
        assert_eq!(roots.run, PathBuf::from("/run/user/1000/radroots"));
        assert_eq!(
            roots.secrets,
            PathBuf::from("/home/treesap/.config/radroots/secrets")
        );
    }

    #[test]
    fn interactive_user_macos_uses_native_library_roots() {
        let resolver = RadrootsPathResolver::new(
            RadrootsPlatform::Macos,
            RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/Users/treesap")),
                ..RadrootsHostEnvironment::default()
            },
        );

        let roots = resolver
            .resolve(
                RadrootsPathProfile::InteractiveUser,
                &RadrootsPathOverrides::default(),
            )
            .expect("resolve macos interactive roots");

        assert_eq!(
            roots,
            RadrootsPaths {
                config: PathBuf::from("/Users/treesap/Library/Application Support/Radroots/config",),
                data: PathBuf::from("/Users/treesap/Library/Application Support/Radroots/data",),
                cache: PathBuf::from("/Users/treesap/Library/Caches/Radroots"),
                logs: PathBuf::from("/Users/treesap/Library/Logs/Radroots"),
                run: PathBuf::from("/Users/treesap/Library/Application Support/Radroots/run",),
                secrets: PathBuf::from(
                    "/Users/treesap/Library/Application Support/Radroots/secrets",
                ),
            }
        );
    }

    #[test]
    fn interactive_user_windows_uses_native_user_roots() {
        let resolver = RadrootsPathResolver::new(
            RadrootsPlatform::Windows,
            RadrootsHostEnvironment {
                appdata_dir: Some(PathBuf::from(r"C:\Users\treesap\AppData\Roaming")),
                localappdata_dir: Some(PathBuf::from(r"C:\Users\treesap\AppData\Local")),
                ..RadrootsHostEnvironment::default()
            },
        );

        let roots = resolver
            .resolve(
                RadrootsPathProfile::InteractiveUser,
                &RadrootsPathOverrides::default(),
            )
            .expect("resolve windows interactive roots");

        assert_eq!(
            roots,
            RadrootsPaths {
                config: PathBuf::from(r"C:\Users\treesap\AppData\Roaming")
                    .join("Radroots")
                    .join("config"),
                data: PathBuf::from(r"C:\Users\treesap\AppData\Local")
                    .join("Radroots")
                    .join("data"),
                cache: PathBuf::from(r"C:\Users\treesap\AppData\Local")
                    .join("Radroots")
                    .join("cache"),
                logs: PathBuf::from(r"C:\Users\treesap\AppData\Local")
                    .join("Radroots")
                    .join("logs"),
                run: PathBuf::from(r"C:\Users\treesap\AppData\Local")
                    .join("Radroots")
                    .join("run"),
                secrets: PathBuf::from(r"C:\Users\treesap\AppData\Roaming")
                    .join("Radroots")
                    .join("secrets"),
            }
        );
    }

    #[test]
    fn service_host_linux_uses_canonical_service_roots() {
        let resolver =
            RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default());

        let roots = resolver
            .resolve(
                RadrootsPathProfile::ServiceHost,
                &RadrootsPathOverrides::default(),
            )
            .expect("resolve service_host roots");

        assert_eq!(
            roots,
            RadrootsPaths {
                config: PathBuf::from("/etc/radroots"),
                data: PathBuf::from("/var/lib/radroots"),
                cache: PathBuf::from("/var/cache/radroots"),
                logs: PathBuf::from("/var/log/radroots"),
                run: PathBuf::from("/run/radroots"),
                secrets: PathBuf::from("/etc/radroots/secrets"),
            }
        );
    }

    #[test]
    fn service_host_is_unsupported_outside_linux() {
        for platform in [
            RadrootsPlatform::Macos,
            RadrootsPlatform::Windows,
            RadrootsPlatform::Android,
            RadrootsPlatform::Ios,
            RadrootsPlatform::Other,
        ] {
            let resolver = RadrootsPathResolver::new(platform, RadrootsHostEnvironment::default());
            assert_eq!(
                resolver
                    .resolve(
                        RadrootsPathProfile::ServiceHost,
                        &RadrootsPathOverrides::default(),
                    )
                    .expect_err("service_host must be unsupported outside linux"),
                RadrootsRuntimePathsError::UnsupportedProfilePlatform {
                    profile: RadrootsPathProfile::ServiceHost,
                    platform,
                }
            );
        }
    }

    #[test]
    fn repo_local_requires_explicit_base_root() {
        let resolver =
            RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default());

        let err = resolver
            .resolve(
                RadrootsPathProfile::RepoLocal,
                &RadrootsPathOverrides::default(),
            )
            .expect_err("repo_local should require an explicit base root");

        assert_eq!(err, RadrootsRuntimePathsError::MissingRepoLocalRoot);
    }

    #[test]
    fn repo_local_uses_explicit_base_root() {
        let resolver =
            RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default());

        let roots = resolver
            .resolve(
                RadrootsPathProfile::RepoLocal,
                &RadrootsPathOverrides::repo_local("/repo/.local/radroots"),
            )
            .expect("resolve repo_local roots");

        assert_eq!(
            roots,
            RadrootsPaths::from_base_root("/repo/.local/radroots")
        );
    }

    #[test]
    fn mobile_native_requires_explicit_roots() {
        let resolver = RadrootsPathResolver::new(
            RadrootsPlatform::Android,
            RadrootsHostEnvironment::default(),
        );

        let err = resolver
            .resolve(
                RadrootsPathProfile::MobileNative,
                &RadrootsPathOverrides::default(),
            )
            .expect_err("mobile_native should require explicit roots");

        assert_eq!(err, RadrootsRuntimePathsError::MissingMobileRoots);
    }

    #[test]
    fn mobile_native_returns_explicit_roots() {
        let resolver =
            RadrootsPathResolver::new(RadrootsPlatform::Ios, RadrootsHostEnvironment::default());
        let mobile_roots = RadrootsPaths {
            config: PathBuf::from("/sandbox/config"),
            data: PathBuf::from("/sandbox/data"),
            cache: PathBuf::from("/sandbox/cache"),
            logs: PathBuf::from("/sandbox/logs"),
            run: PathBuf::from("/sandbox/run"),
            secrets: PathBuf::from("/sandbox/secrets"),
        };

        let roots = resolver
            .resolve(
                RadrootsPathProfile::MobileNative,
                &RadrootsPathOverrides::mobile(mobile_roots.clone()),
            )
            .expect("resolve mobile_native roots");

        assert_eq!(roots, mobile_roots);
    }

    #[test]
    fn namespace_derivation_keeps_runtime_segments_explicit() {
        let namespace = RadrootsRuntimeNamespace::service("myc").expect("namespace");
        let roots = RadrootsPaths::from_base_root("/logical-root");
        let namespaced = roots.namespaced(&namespace);

        assert_eq!(
            namespaced.config,
            PathBuf::from("/logical-root/config/services/myc")
        );
        assert_eq!(
            namespaced.data,
            PathBuf::from("/logical-root/data/services/myc")
        );
        assert_eq!(
            namespaced.secrets,
            PathBuf::from("/logical-root/secrets/services/myc")
        );
    }

    #[test]
    fn namespace_validation_rejects_path_escape_values() {
        let err = RadrootsRuntimeNamespace::app("../cli").expect_err("invalid namespace");
        assert_eq!(
            err,
            RadrootsRuntimePathsError::InvalidNamespaceComponent {
                value: "../cli".to_owned(),
            }
        );
    }

    #[test]
    fn interactive_user_linux_requires_home_for_xdg_defaults() {
        let resolver =
            RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default());

        let err = resolver
            .resolve(
                RadrootsPathProfile::InteractiveUser,
                &RadrootsPathOverrides::default(),
            )
            .expect_err("interactive_user on linux should require a home dir");

        assert_eq!(
            err,
            RadrootsRuntimePathsError::MissingHomeDir {
                platform: RadrootsPlatform::Linux,
            }
        );
    }

    #[test]
    fn interactive_user_windows_requires_native_dirs() {
        let resolver = RadrootsPathResolver::new(
            RadrootsPlatform::Windows,
            RadrootsHostEnvironment::default(),
        );

        let err = resolver
            .resolve(
                RadrootsPathProfile::InteractiveUser,
                &RadrootsPathOverrides::default(),
            )
            .expect_err("interactive_user on windows should require native dirs");

        assert_eq!(err, RadrootsRuntimePathsError::MissingWindowsUserDirs);
    }
}

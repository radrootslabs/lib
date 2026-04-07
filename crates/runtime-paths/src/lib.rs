#![forbid(unsafe_code)]

pub mod error;
pub mod namespace;
pub mod platform;
pub mod roots;

pub use error::RadrootsRuntimePathsError;
pub use namespace::{RadrootsRuntimeNamespace, RadrootsRuntimeNamespaceKind};
pub use platform::{RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPlatform};
pub use roots::{RadrootsPathOverrides, RadrootsPathResolver, RadrootsPaths};

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        RadrootsHostEnvironment, RadrootsPathOverrides, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPaths, RadrootsPlatform, RadrootsRuntimeNamespace, RadrootsRuntimePathsError,
    };

    #[test]
    fn interactive_user_linux_uses_home_dotradroots_root() {
        let resolver = RadrootsPathResolver::new(
            RadrootsPlatform::Linux,
            RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/home/treesap")),
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
            roots,
            RadrootsPaths::from_base_root("/home/treesap/.radroots")
        );
    }

    #[test]
    fn interactive_user_macos_uses_home_dotradroots_root() {
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
            RadrootsPaths::from_base_root("/Users/treesap/.radroots")
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
    fn service_host_unix_uses_canonical_service_roots() {
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
    fn service_host_windows_uses_programdata_roots() {
        let resolver = RadrootsPathResolver::new(
            RadrootsPlatform::Windows,
            RadrootsHostEnvironment {
                programdata_dir: Some(PathBuf::from(r"C:\ProgramData")),
                ..RadrootsHostEnvironment::default()
            },
        );

        let roots = resolver
            .resolve(
                RadrootsPathProfile::ServiceHost,
                &RadrootsPathOverrides::default(),
            )
            .expect("resolve service_host roots");

        assert_eq!(
            roots,
            RadrootsPaths {
                config: PathBuf::from(r"C:\ProgramData")
                    .join("Radroots")
                    .join("config"),
                data: PathBuf::from(r"C:\ProgramData")
                    .join("Radroots")
                    .join("data"),
                cache: PathBuf::from(r"C:\ProgramData")
                    .join("Radroots")
                    .join("cache"),
                logs: PathBuf::from(r"C:\ProgramData")
                    .join("Radroots")
                    .join("logs"),
                run: PathBuf::from(r"C:\ProgramData")
                    .join("Radroots")
                    .join("run"),
                secrets: PathBuf::from(r"C:\ProgramData")
                    .join("Radroots")
                    .join("secrets"),
            }
        );
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
        let roots = RadrootsPaths::from_base_root("/home/treesap/.radroots");
        let namespaced = roots.namespaced(&namespace);

        assert_eq!(
            namespaced.config,
            PathBuf::from("/home/treesap/.radroots/config/services/myc")
        );
        assert_eq!(
            namespaced.data,
            PathBuf::from("/home/treesap/.radroots/data/services/myc")
        );
        assert_eq!(
            namespaced.secrets,
            PathBuf::from("/home/treesap/.radroots/secrets/services/myc")
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
    fn interactive_user_unix_requires_home_dir() {
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

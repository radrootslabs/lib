use std::path::{Path, PathBuf};

use crate::{
    RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPlatform, RadrootsRuntimeNamespace,
    RadrootsRuntimePathsError,
};

const APPLICATION_DIRECTORY: &str = "radroots";
const MACOS_APPLICATION_DIRECTORY: &str = "Radroots";

#[derive(Clone, Copy)]
enum XdgDirectory {
    Config,
    Data,
    State,
    Cache,
    Runtime,
}

impl XdgDirectory {
    const fn home_default(self) -> Option<&'static str> {
        match self {
            Self::Config => Some(".config"),
            Self::Data => Some(".local/share"),
            Self::State => Some(".local/state"),
            Self::Cache => Some(".cache"),
            Self::Runtime => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsPaths {
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
    pub logs: PathBuf,
    pub run: PathBuf,
    pub secrets: PathBuf,
}

impl RadrootsPaths {
    #[must_use]
    pub fn from_base_root(base_root: impl AsRef<Path>) -> Self {
        let base_root = base_root.as_ref();
        Self {
            config: base_root.join("config"),
            data: base_root.join("data"),
            cache: base_root.join("cache"),
            logs: base_root.join("logs"),
            run: base_root.join("run"),
            secrets: base_root.join("secrets"),
        }
    }

    #[must_use]
    pub fn namespaced(&self, namespace: &RadrootsRuntimeNamespace) -> Self {
        let relative = namespace.relative_path();
        Self {
            config: self.config.join(&relative),
            data: self.data.join(&relative),
            cache: self.cache.join(&relative),
            logs: self.logs.join(&relative),
            run: self.run.join(&relative),
            secrets: self.secrets.join(relative),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RadrootsPathOverrides {
    pub repo_local_root: Option<PathBuf>,
    pub mobile_roots: Option<RadrootsPaths>,
}

impl RadrootsPathOverrides {
    #[must_use]
    pub fn repo_local(base_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_local_root: Some(base_root.into()),
            mobile_roots: None,
        }
    }

    #[must_use]
    pub fn mobile(roots: RadrootsPaths) -> Self {
        Self {
            repo_local_root: None,
            mobile_roots: Some(roots),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsPathResolver {
    platform: RadrootsPlatform,
    host_environment: RadrootsHostEnvironment,
}

impl RadrootsPathResolver {
    #[must_use]
    pub fn new(platform: RadrootsPlatform, host_environment: RadrootsHostEnvironment) -> Self {
        Self {
            platform,
            host_environment,
        }
    }

    #[must_use]
    pub fn current() -> Self {
        Self::new(
            RadrootsPlatform::current(),
            RadrootsHostEnvironment::from_current_process(),
        )
    }

    #[must_use]
    pub fn platform(&self) -> RadrootsPlatform {
        self.platform
    }

    pub fn resolve(
        &self,
        profile: RadrootsPathProfile,
        overrides: &RadrootsPathOverrides,
    ) -> Result<RadrootsPaths, RadrootsRuntimePathsError> {
        match profile {
            RadrootsPathProfile::InteractiveUser => self.resolve_interactive_user(),
            RadrootsPathProfile::ServiceHost => self.resolve_service_host(),
            RadrootsPathProfile::RepoLocal => overrides
                .repo_local_root
                .as_ref()
                .map(RadrootsPaths::from_base_root)
                .ok_or(RadrootsRuntimePathsError::MissingRepoLocalRoot),
            RadrootsPathProfile::MobileNative => match self.platform {
                RadrootsPlatform::Android | RadrootsPlatform::Ios => overrides
                    .mobile_roots
                    .clone()
                    .ok_or(RadrootsRuntimePathsError::MissingMobileRoots),
                RadrootsPlatform::Linux
                | RadrootsPlatform::Macos
                | RadrootsPlatform::Windows
                | RadrootsPlatform::Other => {
                    Err(RadrootsRuntimePathsError::UnsupportedProfilePlatform {
                        profile,
                        platform: self.platform,
                    })
                }
            },
        }
    }

    fn resolve_interactive_user(&self) -> Result<RadrootsPaths, RadrootsRuntimePathsError> {
        match self.platform {
            RadrootsPlatform::Linux => self.resolve_linux_interactive_user(),
            RadrootsPlatform::Macos => self.resolve_macos_interactive_user(),
            RadrootsPlatform::Windows => {
                let appdata = self
                    .host_environment
                    .appdata_dir
                    .as_ref()
                    .ok_or(RadrootsRuntimePathsError::MissingWindowsUserDirs)?;
                let localappdata = self
                    .host_environment
                    .localappdata_dir
                    .as_ref()
                    .ok_or(RadrootsRuntimePathsError::MissingWindowsUserDirs)?;
                let config_root = appdata.join("Radroots");
                let local_root = localappdata.join("Radroots");
                Ok(RadrootsPaths {
                    config: config_root.join("config"),
                    data: local_root.join("data"),
                    cache: local_root.join("cache"),
                    logs: local_root.join("logs"),
                    run: local_root.join("run"),
                    secrets: config_root.join("secrets"),
                })
            }
            RadrootsPlatform::Android | RadrootsPlatform::Ios | RadrootsPlatform::Other => {
                Err(RadrootsRuntimePathsError::UnsupportedProfilePlatform {
                    profile: RadrootsPathProfile::InteractiveUser,
                    platform: self.platform,
                })
            }
        }
    }

    fn resolve_linux_interactive_user(&self) -> Result<RadrootsPaths, RadrootsRuntimePathsError> {
        let config = self.resolve_xdg_directory(XdgDirectory::Config)?;
        let data = self.resolve_xdg_directory(XdgDirectory::Data)?;
        let state = self.resolve_xdg_directory(XdgDirectory::State)?;
        let cache = self.resolve_xdg_directory(XdgDirectory::Cache)?;
        let run = self.resolve_xdg_directory(XdgDirectory::Runtime)?;
        Ok(RadrootsPaths {
            config: config.join(APPLICATION_DIRECTORY),
            data: data.join(APPLICATION_DIRECTORY),
            cache: cache.join(APPLICATION_DIRECTORY),
            logs: state.join(APPLICATION_DIRECTORY).join("logs"),
            run: run.join(APPLICATION_DIRECTORY),
            secrets: config.join(APPLICATION_DIRECTORY).join("secrets"),
        })
    }

    fn resolve_xdg_directory(
        &self,
        directory: XdgDirectory,
    ) -> Result<PathBuf, RadrootsRuntimePathsError> {
        let configured = match directory {
            XdgDirectory::Config => &self.host_environment.xdg_config_home,
            XdgDirectory::Data => &self.host_environment.xdg_data_home,
            XdgDirectory::State => &self.host_environment.xdg_state_home,
            XdgDirectory::Cache => &self.host_environment.xdg_cache_home,
            XdgDirectory::Runtime => &self.host_environment.xdg_runtime_dir,
        };
        if let Some(path) = configured.as_ref().filter(|path| path.is_absolute()) {
            return Ok(path.clone());
        }
        let Some(default) = directory.home_default() else {
            return Err(RadrootsRuntimePathsError::MissingXdgRuntimeDir);
        };
        Ok(self.valid_home_dir(RadrootsPlatform::Linux)?.join(default))
    }

    fn resolve_macos_interactive_user(&self) -> Result<RadrootsPaths, RadrootsRuntimePathsError> {
        let home = self.valid_home_dir(RadrootsPlatform::Macos)?;
        let application_support = home
            .join("Library/Application Support")
            .join(MACOS_APPLICATION_DIRECTORY);
        Ok(RadrootsPaths {
            config: application_support.join("config"),
            data: application_support.join("data"),
            cache: home
                .join("Library/Caches")
                .join(MACOS_APPLICATION_DIRECTORY),
            logs: home.join("Library/Logs").join(MACOS_APPLICATION_DIRECTORY),
            run: application_support.join("run"),
            secrets: application_support.join("secrets"),
        })
    }

    fn valid_home_dir(
        &self,
        platform: RadrootsPlatform,
    ) -> Result<&Path, RadrootsRuntimePathsError> {
        let home = self
            .host_environment
            .home_dir
            .as_deref()
            .ok_or(RadrootsRuntimePathsError::MissingHomeDir { platform })?;
        if home.is_absolute() {
            Ok(home)
        } else {
            Err(RadrootsRuntimePathsError::InvalidHomeDir { platform })
        }
    }

    fn resolve_service_host(&self) -> Result<RadrootsPaths, RadrootsRuntimePathsError> {
        match self.platform {
            RadrootsPlatform::Linux => Ok(RadrootsPaths {
                config: PathBuf::from("/etc/radroots"),
                data: PathBuf::from("/var/lib/radroots"),
                cache: PathBuf::from("/var/cache/radroots"),
                logs: PathBuf::from("/var/log/radroots"),
                run: PathBuf::from("/run/radroots"),
                secrets: PathBuf::from("/etc/radroots/secrets"),
            }),
            RadrootsPlatform::Macos
            | RadrootsPlatform::Windows
            | RadrootsPlatform::Android
            | RadrootsPlatform::Ios
            | RadrootsPlatform::Other => {
                Err(RadrootsRuntimePathsError::UnsupportedProfilePlatform {
                    profile: RadrootsPathProfile::ServiceHost,
                    platform: self.platform,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{RadrootsPathOverrides, RadrootsPathResolver, RadrootsPaths};
    use crate::{
        RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPlatform, RadrootsRuntimePathsError,
    };

    #[test]
    fn path_override_helpers_only_populate_their_owned_slot() {
        let repo_local = RadrootsPathOverrides::repo_local("/repo/.local/radroots");
        assert_eq!(
            repo_local.repo_local_root,
            Some(PathBuf::from("/repo/.local/radroots"))
        );
        assert!(repo_local.mobile_roots.is_none());

        let mobile_roots = RadrootsPaths::from_base_root("/sandbox");
        let mobile = RadrootsPathOverrides::mobile(mobile_roots.clone());
        assert!(mobile.repo_local_root.is_none());
        assert_eq!(mobile.mobile_roots, Some(mobile_roots));
    }

    #[test]
    fn resolver_current_uses_process_platform_and_environment() {
        let resolver = RadrootsPathResolver::current();
        assert_eq!(resolver.platform(), RadrootsPlatform::current());
        assert_eq!(
            resolver,
            RadrootsPathResolver::new(
                RadrootsPlatform::current(),
                RadrootsHostEnvironment::from_current_process()
            )
        );
    }

    #[test]
    fn linux_interactive_uses_exact_config_data_state_cache_and_runtime_roots() {
        let resolver = RadrootsPathResolver::new(
            RadrootsPlatform::Linux,
            RadrootsHostEnvironment {
                xdg_config_home: Some(PathBuf::from("/xdg/config")),
                xdg_data_home: Some(PathBuf::from("/xdg/data")),
                xdg_state_home: Some(PathBuf::from("/xdg/state")),
                xdg_cache_home: Some(PathBuf::from("/xdg/cache")),
                xdg_runtime_dir: Some(PathBuf::from("/xdg/run")),
                ..RadrootsHostEnvironment::default()
            },
        );

        assert_eq!(
            resolver
                .resolve(
                    RadrootsPathProfile::InteractiveUser,
                    &RadrootsPathOverrides::default(),
                )
                .expect("configured XDG roots"),
            RadrootsPaths {
                config: PathBuf::from("/xdg/config/radroots"),
                data: PathBuf::from("/xdg/data/radroots"),
                cache: PathBuf::from("/xdg/cache/radroots"),
                logs: PathBuf::from("/xdg/state/radroots/logs"),
                run: PathBuf::from("/xdg/run/radroots"),
                secrets: PathBuf::from("/xdg/config/radroots/secrets"),
            }
        );
    }

    #[test]
    fn linux_interactive_uses_xdg_home_defaults_but_never_invents_runtime() {
        let resolver = RadrootsPathResolver::new(
            RadrootsPlatform::Linux,
            RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/home/treesap")),
                xdg_runtime_dir: Some(PathBuf::from("/run/user/1000")),
                ..RadrootsHostEnvironment::default()
            },
        );

        assert_eq!(
            resolver
                .resolve(
                    RadrootsPathProfile::InteractiveUser,
                    &RadrootsPathOverrides::default(),
                )
                .expect("defaulted XDG roots"),
            RadrootsPaths {
                config: PathBuf::from("/home/treesap/.config/radroots"),
                data: PathBuf::from("/home/treesap/.local/share/radroots"),
                cache: PathBuf::from("/home/treesap/.cache/radroots"),
                logs: PathBuf::from("/home/treesap/.local/state/radroots/logs"),
                run: PathBuf::from("/run/user/1000/radroots"),
                secrets: PathBuf::from("/home/treesap/.config/radroots/secrets"),
            }
        );

        let missing_runtime = RadrootsPathResolver::new(
            RadrootsPlatform::Linux,
            RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/home/treesap")),
                ..RadrootsHostEnvironment::default()
            },
        );
        assert_eq!(
            missing_runtime
                .resolve(
                    RadrootsPathProfile::InteractiveUser,
                    &RadrootsPathOverrides::default(),
                )
                .expect_err("XDG runtime has no home fallback"),
            RadrootsRuntimePathsError::MissingXdgRuntimeDir
        );
    }

    #[test]
    fn linux_interactive_ignores_empty_and_relative_xdg_directories() {
        let valid = RadrootsHostEnvironment {
            home_dir: Some(PathBuf::from("/home/treesap")),
            xdg_config_home: Some(PathBuf::from("/xdg/config")),
            xdg_data_home: Some(PathBuf::from("/xdg/data")),
            xdg_state_home: Some(PathBuf::from("/xdg/state")),
            xdg_cache_home: Some(PathBuf::from("/xdg/cache")),
            xdg_runtime_dir: Some(PathBuf::from("/xdg/run")),
            ..RadrootsHostEnvironment::default()
        };
        for (environment, expected) in [
            (
                RadrootsHostEnvironment {
                    xdg_config_home: Some(PathBuf::new()),
                    ..valid.clone()
                },
                PathBuf::from("/home/treesap/.config/radroots"),
            ),
            (
                RadrootsHostEnvironment {
                    xdg_config_home: Some(PathBuf::from("relative/config")),
                    ..valid.clone()
                },
                PathBuf::from("/home/treesap/.config/radroots"),
            ),
            (
                RadrootsHostEnvironment {
                    xdg_data_home: Some(PathBuf::new()),
                    ..valid.clone()
                },
                PathBuf::from("/home/treesap/.local/share/radroots"),
            ),
            (
                RadrootsHostEnvironment {
                    xdg_data_home: Some(PathBuf::from("relative/data")),
                    ..valid.clone()
                },
                PathBuf::from("/home/treesap/.local/share/radroots"),
            ),
            (
                RadrootsHostEnvironment {
                    xdg_state_home: Some(PathBuf::new()),
                    ..valid.clone()
                },
                PathBuf::from("/home/treesap/.local/state/radroots/logs"),
            ),
            (
                RadrootsHostEnvironment {
                    xdg_state_home: Some(PathBuf::from("relative/state")),
                    ..valid.clone()
                },
                PathBuf::from("/home/treesap/.local/state/radroots/logs"),
            ),
            (
                RadrootsHostEnvironment {
                    xdg_cache_home: Some(PathBuf::new()),
                    ..valid.clone()
                },
                PathBuf::from("/home/treesap/.cache/radroots"),
            ),
            (
                RadrootsHostEnvironment {
                    xdg_cache_home: Some(PathBuf::from("relative/cache")),
                    ..valid.clone()
                },
                PathBuf::from("/home/treesap/.cache/radroots"),
            ),
        ] {
            let resolver = RadrootsPathResolver::new(RadrootsPlatform::Linux, environment);
            let roots = resolver
                .resolve(
                    RadrootsPathProfile::InteractiveUser,
                    &RadrootsPathOverrides::default(),
                )
                .expect("invalid optional XDG directory is ignored");
            assert!(
                [roots.config, roots.data, roots.logs, roots.cache].contains(&expected),
                "expected fallback root {expected:?}"
            );
        }

        for xdg_runtime_dir in [Some(PathBuf::new()), Some(PathBuf::from("relative/run"))] {
            let resolver = RadrootsPathResolver::new(
                RadrootsPlatform::Linux,
                RadrootsHostEnvironment {
                    xdg_runtime_dir,
                    ..valid.clone()
                },
            );
            assert_eq!(
                resolver
                    .resolve(
                        RadrootsPathProfile::InteractiveUser,
                        &RadrootsPathOverrides::default(),
                    )
                    .expect_err("invalid XDG runtime is treated as missing"),
                RadrootsRuntimePathsError::MissingXdgRuntimeDir
            );
        }
    }

    #[test]
    fn interactive_home_derived_roots_require_an_absolute_nonempty_home() {
        for home_dir in [Some(PathBuf::new()), Some(PathBuf::from("relative/home"))] {
            let linux = RadrootsPathResolver::new(
                RadrootsPlatform::Linux,
                RadrootsHostEnvironment {
                    home_dir: home_dir.clone(),
                    xdg_runtime_dir: Some(PathBuf::from("/run/user/1000")),
                    ..RadrootsHostEnvironment::default()
                },
            );
            assert_eq!(
                linux
                    .resolve(
                        RadrootsPathProfile::InteractiveUser,
                        &RadrootsPathOverrides::default(),
                    )
                    .expect_err("Linux HOME defaults require an absolute HOME"),
                RadrootsRuntimePathsError::InvalidHomeDir {
                    platform: RadrootsPlatform::Linux,
                }
            );

            let macos = RadrootsPathResolver::new(
                RadrootsPlatform::Macos,
                RadrootsHostEnvironment {
                    home_dir,
                    ..RadrootsHostEnvironment::default()
                },
            );
            assert_eq!(
                macos
                    .resolve(
                        RadrootsPathProfile::InteractiveUser,
                        &RadrootsPathOverrides::default(),
                    )
                    .expect_err("macOS native roots require an absolute HOME"),
                RadrootsRuntimePathsError::InvalidHomeDir {
                    platform: RadrootsPlatform::Macos,
                }
            );
        }
    }

    #[test]
    fn macos_interactive_uses_native_library_roots_and_ignores_xdg() {
        let resolver = RadrootsPathResolver::new(
            RadrootsPlatform::Macos,
            RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/Users/treesap")),
                xdg_config_home: Some(PathBuf::from("relative/config")),
                xdg_data_home: Some(PathBuf::from("relative/data")),
                xdg_state_home: Some(PathBuf::from("relative/state")),
                xdg_cache_home: Some(PathBuf::from("relative/cache")),
                xdg_runtime_dir: Some(PathBuf::from("relative/run")),
                ..RadrootsHostEnvironment::default()
            },
        );

        assert_eq!(
            resolver
                .resolve(
                    RadrootsPathProfile::InteractiveUser,
                    &RadrootsPathOverrides::default(),
                )
                .expect("macOS native roots"),
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
    fn mobile_profile_is_rejected_on_non_mobile_platforms() {
        let resolver =
            RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default());

        let err = resolver
            .resolve(
                RadrootsPathProfile::MobileNative,
                &RadrootsPathOverrides::default(),
            )
            .expect_err("mobile profile should be rejected on linux");

        assert_eq!(
            err,
            RadrootsRuntimePathsError::UnsupportedProfilePlatform {
                profile: RadrootsPathProfile::MobileNative,
                platform: RadrootsPlatform::Linux,
            }
        );
    }

    #[test]
    fn interactive_user_is_rejected_on_mobile_platforms() {
        for platform in [RadrootsPlatform::Android, RadrootsPlatform::Ios] {
            let resolver = RadrootsPathResolver::new(platform, RadrootsHostEnvironment::default());
            let err = resolver
                .resolve(
                    RadrootsPathProfile::InteractiveUser,
                    &RadrootsPathOverrides::default(),
                )
                .expect_err("interactive_user should be unsupported on mobile");
            assert_eq!(
                err,
                RadrootsRuntimePathsError::UnsupportedProfilePlatform {
                    profile: RadrootsPathProfile::InteractiveUser,
                    platform,
                }
            );
        }
    }

    #[test]
    fn service_host_is_rejected_outside_linux() {
        for platform in [
            RadrootsPlatform::Macos,
            RadrootsPlatform::Windows,
            RadrootsPlatform::Android,
            RadrootsPlatform::Ios,
            RadrootsPlatform::Other,
        ] {
            let resolver = RadrootsPathResolver::new(platform, RadrootsHostEnvironment::default());
            let err = resolver
                .resolve(
                    RadrootsPathProfile::ServiceHost,
                    &RadrootsPathOverrides::default(),
                )
                .expect_err("service_host should be unsupported outside linux");
            assert_eq!(
                err,
                RadrootsRuntimePathsError::UnsupportedProfilePlatform {
                    profile: RadrootsPathProfile::ServiceHost,
                    platform,
                }
            );
        }
    }
}

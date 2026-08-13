use std::path::{Component, Path, PathBuf};

use crate::{
    RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPlatform, RadrootsRuntimePathsError,
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
pub(crate) struct RadrootsPaths {
    pub(crate) config: PathBuf,
    pub(crate) data: PathBuf,
    pub(crate) cache: PathBuf,
    pub(crate) logs: PathBuf,
    pub(crate) run: PathBuf,
    pub(crate) secrets: PathBuf,
}

impl RadrootsPaths {
    #[must_use]
    pub(crate) fn from_base_root(base_root: impl AsRef<Path>) -> Self {
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
    pub fn platform(&self) -> RadrootsPlatform {
        self.platform
    }

    pub(crate) fn resolve(
        &self,
        profile: RadrootsPathProfile,
        repo_local_root: Option<&Path>,
    ) -> Result<RadrootsPaths, RadrootsRuntimePathsError> {
        match profile {
            RadrootsPathProfile::InteractiveUser => self.resolve_interactive_user(),
            RadrootsPathProfile::ServiceHost => self.resolve_service_host(),
            RadrootsPathProfile::RepoLocal => {
                let root =
                    repo_local_root.ok_or(RadrootsRuntimePathsError::MissingRepoLocalRoot)?;
                validate_repo_local_root(root)?;
                Ok(RadrootsPaths::from_base_root(root))
            }
            RadrootsPathProfile::MobileNative => {
                Err(RadrootsRuntimePathsError::UnsupportedProfilePlatform {
                    profile,
                    platform: self.platform,
                })
            }
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

pub(crate) fn validate_repo_local_root(root: &Path) -> Result<(), RadrootsRuntimePathsError> {
    if !root.is_absolute()
        || root.parent().is_none()
        || root
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(RadrootsRuntimePathsError::InvalidRepoLocalRoot);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{RadrootsPathResolver, RadrootsPaths};
    use crate::{
        RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPlatform, RadrootsRuntimePathsError,
    };

    #[test]
    fn repo_local_is_explicit_validated_and_one_base() {
        let resolver =
            RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default());
        assert_eq!(
            resolver
                .resolve(RadrootsPathProfile::RepoLocal, None)
                .expect_err("missing root"),
            RadrootsRuntimePathsError::MissingRepoLocalRoot
        );
        for invalid in ["", ".", "relative/root", "/", "/repo/../escape"] {
            assert_eq!(
                resolver
                    .resolve(RadrootsPathProfile::RepoLocal, Some(Path::new(invalid)))
                    .expect_err("invalid root"),
                RadrootsRuntimePathsError::InvalidRepoLocalRoot
            );
        }
        assert_eq!(
            resolver
                .resolve(
                    RadrootsPathProfile::RepoLocal,
                    Some(Path::new("/repo/.local/radroots")),
                )
                .expect("repo roots"),
            RadrootsPaths::from_base_root("/repo/.local/radroots")
        );
    }

    #[test]
    fn linux_interactive_uses_injected_xdg_and_home_defaults() {
        let resolver = RadrootsPathResolver::new(
            RadrootsPlatform::Linux,
            RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/home/treesap")),
                xdg_config_home: Some(PathBuf::from("relative-ignored")),
                xdg_runtime_dir: Some(PathBuf::from("/run/user/1000")),
                ..RadrootsHostEnvironment::default()
            },
        );
        assert_eq!(
            resolver
                .resolve(RadrootsPathProfile::InteractiveUser, None)
                .expect("linux interactive"),
            RadrootsPaths {
                config: PathBuf::from("/home/treesap/.config/radroots"),
                data: PathBuf::from("/home/treesap/.local/share/radroots"),
                cache: PathBuf::from("/home/treesap/.cache/radroots"),
                logs: PathBuf::from("/home/treesap/.local/state/radroots/logs"),
                run: PathBuf::from("/run/user/1000/radroots"),
                secrets: PathBuf::from("/home/treesap/.config/radroots/secrets"),
            }
        );
    }

    #[test]
    fn linux_interactive_never_invents_runtime_root() {
        let resolver = RadrootsPathResolver::new(
            RadrootsPlatform::Linux,
            RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/home/treesap")),
                ..RadrootsHostEnvironment::default()
            },
        );
        assert_eq!(
            resolver
                .resolve(RadrootsPathProfile::InteractiveUser, None)
                .expect_err("runtime root is required"),
            RadrootsRuntimePathsError::MissingXdgRuntimeDir
        );
    }

    #[test]
    fn home_derived_roots_require_absolute_home() {
        for platform in [RadrootsPlatform::Linux, RadrootsPlatform::Macos] {
            let resolver = RadrootsPathResolver::new(
                platform,
                RadrootsHostEnvironment {
                    home_dir: Some(PathBuf::from("relative")),
                    xdg_runtime_dir: Some(PathBuf::from("/run/user/1000")),
                    ..RadrootsHostEnvironment::default()
                },
            );
            assert_eq!(
                resolver
                    .resolve(RadrootsPathProfile::InteractiveUser, None)
                    .expect_err("relative home"),
                RadrootsRuntimePathsError::InvalidHomeDir { platform }
            );
        }
    }

    #[test]
    fn service_host_linux_uses_canonical_roots() {
        let resolver =
            RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default());
        assert_eq!(resolver.platform(), RadrootsPlatform::Linux);
        assert_eq!(
            resolver
                .resolve(RadrootsPathProfile::ServiceHost, None)
                .expect("service host"),
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
    fn unsupported_profile_platform_pairs_fail_closed() {
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
                    .resolve(RadrootsPathProfile::ServiceHost, None)
                    .expect_err("unsupported service host"),
                RadrootsRuntimePathsError::UnsupportedProfilePlatform {
                    profile: RadrootsPathProfile::ServiceHost,
                    platform,
                }
            );
        }
        for platform in [
            RadrootsPlatform::Linux,
            RadrootsPlatform::Macos,
            RadrootsPlatform::Windows,
            RadrootsPlatform::Android,
            RadrootsPlatform::Ios,
            RadrootsPlatform::Other,
        ] {
            let resolver = RadrootsPathResolver::new(platform, RadrootsHostEnvironment::default());
            assert_eq!(
                resolver
                    .resolve(RadrootsPathProfile::MobileNative, None)
                    .expect_err("mobile roots require a product-owned adapter"),
                RadrootsRuntimePathsError::UnsupportedProfilePlatform {
                    profile: RadrootsPathProfile::MobileNative,
                    platform,
                }
            );
        }
    }
}

use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsPlatform {
    Linux,
    Macos,
    Windows,
    Android,
    Ios,
}

impl RadrootsPlatform {
    #[must_use]
    pub fn current() -> Self {
        if cfg!(target_os = "android") {
            Self::Android
        } else if cfg!(target_os = "ios") {
            Self::Ios
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }

    #[must_use]
    pub fn is_unix_like(self) -> bool {
        matches!(self, Self::Linux | Self::Macos)
    }
}

impl fmt::Display for RadrootsPlatform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Linux => "linux",
            Self::Macos => "macos",
            Self::Windows => "windows",
            Self::Android => "android",
            Self::Ios => "ios",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsPathProfile {
    InteractiveUser,
    ServiceHost,
    RepoLocal,
    MobileNative,
}

impl fmt::Display for RadrootsPathProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InteractiveUser => "interactive_user",
            Self::ServiceHost => "service_host",
            Self::RepoLocal => "repo_local",
            Self::MobileNative => "mobile_native",
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RadrootsHostEnvironment {
    pub home_dir: Option<PathBuf>,
    pub appdata_dir: Option<PathBuf>,
    pub localappdata_dir: Option<PathBuf>,
    pub programdata_dir: Option<PathBuf>,
}

impl RadrootsHostEnvironment {
    #[must_use]
    pub fn from_current_process() -> Self {
        Self {
            home_dir: std::env::var_os("HOME").map(PathBuf::from),
            appdata_dir: std::env::var_os("APPDATA").map(PathBuf::from),
            localappdata_dir: std::env::var_os("LOCALAPPDATA").map(PathBuf::from),
            programdata_dir: std::env::var_os("ProgramData").map(PathBuf::from),
        }
    }
}

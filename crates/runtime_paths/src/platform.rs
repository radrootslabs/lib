use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsPlatform {
    Linux,
    Macos,
    Windows,
    Android,
    Ios,
    Other,
}

impl RadrootsPlatform {
    #[must_use]
    #[cfg(target_os = "android")]
    pub fn current() -> Self {
        Self::Android
    }

    #[must_use]
    #[cfg(target_os = "ios")]
    pub fn current() -> Self {
        Self::Ios
    }

    #[must_use]
    #[cfg(target_os = "macos")]
    pub fn current() -> Self {
        Self::Macos
    }

    #[must_use]
    #[cfg(target_os = "windows")]
    pub fn current() -> Self {
        Self::Windows
    }

    #[must_use]
    #[cfg(target_os = "linux")]
    pub fn current() -> Self {
        Self::Linux
    }

    #[must_use]
    #[cfg(not(any(
        target_os = "android",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    )))]
    pub fn current() -> Self {
        Self::Other
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
            Self::Other => "other",
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
    pub xdg_config_home: Option<PathBuf>,
    pub xdg_data_home: Option<PathBuf>,
    pub xdg_state_home: Option<PathBuf>,
    pub xdg_cache_home: Option<PathBuf>,
    pub xdg_runtime_dir: Option<PathBuf>,
    pub appdata_dir: Option<PathBuf>,
    pub localappdata_dir: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::{RadrootsPathProfile, RadrootsPlatform};

    #[test]
    fn current_matches_compiled_target_platform() {
        #[cfg(target_os = "android")]
        let expected = RadrootsPlatform::Android;
        #[cfg(target_os = "ios")]
        let expected = RadrootsPlatform::Ios;
        #[cfg(target_os = "macos")]
        let expected = RadrootsPlatform::Macos;
        #[cfg(target_os = "windows")]
        let expected = RadrootsPlatform::Windows;
        #[cfg(target_os = "linux")]
        let expected = RadrootsPlatform::Linux;
        #[cfg(not(any(
            target_os = "android",
            target_os = "ios",
            target_os = "linux",
            target_os = "macos",
            target_os = "windows"
        )))]
        let expected = RadrootsPlatform::Other;

        assert_eq!(RadrootsPlatform::current(), expected);
    }

    #[test]
    fn unix_like_classification_is_explicit() {
        assert!(RadrootsPlatform::Linux.is_unix_like());
        assert!(RadrootsPlatform::Macos.is_unix_like());
        assert!(!RadrootsPlatform::Windows.is_unix_like());
        assert!(!RadrootsPlatform::Android.is_unix_like());
        assert!(!RadrootsPlatform::Ios.is_unix_like());
        assert!(!RadrootsPlatform::Other.is_unix_like());
    }

    #[test]
    fn display_uses_canonical_labels() {
        assert_eq!(RadrootsPlatform::Linux.to_string(), "linux");
        assert_eq!(RadrootsPlatform::Macos.to_string(), "macos");
        assert_eq!(RadrootsPlatform::Windows.to_string(), "windows");
        assert_eq!(RadrootsPlatform::Android.to_string(), "android");
        assert_eq!(RadrootsPlatform::Ios.to_string(), "ios");
        assert_eq!(RadrootsPlatform::Other.to_string(), "other");

        assert_eq!(
            RadrootsPathProfile::InteractiveUser.to_string(),
            "interactive_user"
        );
        assert_eq!(RadrootsPathProfile::ServiceHost.to_string(), "service_host");
        assert_eq!(RadrootsPathProfile::RepoLocal.to_string(), "repo_local");
        assert_eq!(
            RadrootsPathProfile::MobileNative.to_string(),
            "mobile_native"
        );
    }
}

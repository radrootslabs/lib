use radroots_log::{LogFileLayout, LoggingOptions};
use radroots_runtime_paths::{
    RadrootsPathOverrides, RadrootsPathProfile, RadrootsPathResolver, RadrootsRuntimePathsError,
    default_shared_runtime_logs_dir as resolve_shared_runtime_logs_dir,
};
use std::path::{Path, PathBuf};

use crate::error::RuntimeTracingError;

pub fn init() -> Result<(), RuntimeTracingError> {
    let logs_dir = default_shared_runtime_logs_dir()?;
    init_with_logs_dir(logs_dir, None)
}

pub fn default_shared_runtime_logs_dir() -> Result<PathBuf, RadrootsRuntimePathsError> {
    default_shared_runtime_logs_dir_for(
        &RadrootsPathResolver::current(),
        RadrootsPathProfile::InteractiveUser,
        &RadrootsPathOverrides::default(),
    )
}

pub fn default_shared_runtime_logs_dir_for(
    resolver: &RadrootsPathResolver,
    profile: RadrootsPathProfile,
    overrides: &RadrootsPathOverrides,
) -> Result<PathBuf, RadrootsRuntimePathsError> {
    resolve_shared_runtime_logs_dir(resolver, profile, overrides)
}

pub fn init_with_logs_dir(
    logs_dir: impl AsRef<Path>,
    default_level: Option<&str>,
) -> Result<(), RuntimeTracingError> {
    let logs_dir = logs_dir.as_ref();
    let env_file = env_value("LOG_FILE").or_else(|| env_value("RADROOTS_LOG_FILE"));
    let env_level = env_value("LOG_LEVEL").or_else(|| env_value("RUST_LOG"));
    let opts = LoggingOptions {
        dir: Some(logs_dir.to_path_buf()),
        file_name: env_file.unwrap_or_else(default_log_file_name),
        stdout: true,
        default_level: resolve_default_level(env_level, default_level),
        file_layout: LogFileLayout::PrefixedDate,
    };
    radroots_log::init_logging(opts)?;
    Ok(())
}

fn default_log_file_name() -> String {
    default_log_file_name_from_exe_name(log_name_from_exe())
}

fn default_log_file_name_from_exe_name(exe_name: Option<String>) -> String {
    exe_name.unwrap_or_else(|| format!("{}.log", env!("CARGO_PKG_NAME")))
}

fn log_name_from_exe() -> Option<String> {
    log_name_from_path(std::env::current_exe().ok())
}

fn log_name_from_path(exe: Option<PathBuf>) -> Option<String> {
    let exe = exe?;
    let name = exe.file_stem()?.to_string_lossy();
    log_name_from_stem(name.as_ref())
}

#[cfg(test)]
mod test_hooks {
    use std::cell::Cell;

    thread_local! {
        static IGNORE_ENV: Cell<bool> = const { Cell::new(false) };
    }

    pub fn set_ignore_env(ignore: bool) {
        IGNORE_ENV.with(|state| state.set(ignore));
    }

    pub fn ignore_env() -> bool {
        IGNORE_ENV.with(Cell::get)
    }
}

fn log_name_from_stem(stem: &str) -> Option<String> {
    if stem.is_empty() {
        None
    } else {
        Some(format!("{stem}.log"))
    }
}

fn env_value(key: &str) -> Option<String> {
    #[cfg(test)]
    if test_hooks::ignore_env() {
        return None;
    }
    let value = std::env::var(key).ok()?;
    normalize_env_value(&value)
}

fn normalize_env_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn resolve_default_level(env_level: Option<String>, default_level: Option<&str>) -> Option<String> {
    if let Some(level) = env_level {
        Some(level)
    } else {
        default_level.map(str::to_string)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        default_log_file_name, default_log_file_name_from_exe_name,
        default_shared_runtime_logs_dir_for, env_value, init_with_logs_dir, log_name_from_path,
        log_name_from_stem, normalize_env_value, resolve_default_level, test_hooks,
    };
    use radroots_runtime_paths::{
        RadrootsHostEnvironment, RadrootsPathOverrides, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform,
    };
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn normalize_env_value_handles_empty_and_non_empty_values() {
        assert_eq!(normalize_env_value(" value "), Some("value".to_string()));
        assert_eq!(normalize_env_value("   "), None);
        assert_eq!(normalize_env_value(""), None);
    }

    #[test]
    fn env_helpers_return_expected_values() {
        assert_eq!(env_value("RADROOTS_RUNTIME_TEST_MISSING_KEY"), None);
        let home = env_value("HOME").expect("home env");
        assert!(!home.is_empty());
    }

    #[test]
    fn log_name_helpers_cover_empty_and_non_empty_names() {
        assert_eq!(
            log_name_from_stem("radrootsd"),
            Some("radrootsd.log".to_string())
        );
        assert_eq!(log_name_from_stem(""), None);
    }

    #[test]
    fn log_name_from_path_handles_missing_components() {
        assert_eq!(log_name_from_path(None), None);
        assert_eq!(log_name_from_path(Some(PathBuf::from("/"))), None);
        assert_eq!(
            log_name_from_path(Some(PathBuf::from("/tmp/radrootsd"))),
            Some("radrootsd.log".to_string())
        );
    }

    #[test]
    fn default_log_file_name_helpers_cover_fallback() {
        assert_eq!(
            default_log_file_name_from_exe_name(Some("svc.log".to_string())),
            "svc.log"
        );
        assert_eq!(
            default_log_file_name_from_exe_name(None),
            format!("{}.log", env!("CARGO_PKG_NAME"))
        );
        assert!(!default_log_file_name().trim().is_empty());
    }

    #[test]
    fn resolve_default_level_prefers_env_then_fallback() {
        let env_value = resolve_default_level(Some("warn".to_string()), Some("info"));
        assert_eq!(env_value, Some("warn".to_string()));
        let fallback = resolve_default_level(None, Some("info"));
        assert_eq!(fallback, Some("info".to_string()));
        let none = resolve_default_level(None, None);
        assert_eq!(none, None);
    }

    #[test]
    fn default_shared_runtime_logs_dir_uses_shared_namespace() {
        let resolver = RadrootsPathResolver::new(
            RadrootsPlatform::Macos,
            RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/Users/treesap")),
                ..RadrootsHostEnvironment::default()
            },
        );

        let logs_dir = default_shared_runtime_logs_dir_for(
            &resolver,
            RadrootsPathProfile::InteractiveUser,
            &RadrootsPathOverrides::default(),
        )
        .expect("default shared runtime logs dir should resolve");

        assert_eq!(
            logs_dir,
            PathBuf::from("/Users/treesap/.radroots/logs/shared/runtime")
        );
    }

    #[test]
    fn init_paths_execute() {
        let dir = tempdir().expect("tempdir");
        test_hooks::set_ignore_env(true);
        let invalid = dir.path().join("not-a-dir");
        std::fs::write(&invalid, "file").expect("write invalid path");
        let err_path = init_with_logs_dir(invalid.as_path(), Some("info"));
        assert!(err_path.is_err());
        let invalid_str = invalid.to_string_lossy().to_string();
        let err_str = init_with_logs_dir(invalid_str.as_str(), Some("info"));
        assert!(err_str.is_err());
        let first = init_with_logs_dir(dir.path(), Some("info"));
        assert!(first.is_ok());
        let owned_path = dir.path().to_path_buf();
        let third = init_with_logs_dir(owned_path.as_path(), None);
        assert!(third.is_ok());
        test_hooks::set_ignore_env(false);
    }
}

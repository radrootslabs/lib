use radroots_log::LoggingOptions;
use std::path::{Path, PathBuf};

use crate::error::RuntimeTracingError;

pub fn init() -> Result<(), RuntimeTracingError> {
    init_with("logs", None)
}

pub fn init_with(
    logs_dir: impl AsRef<Path>,
    default_level: Option<&str>,
) -> Result<(), RuntimeTracingError> {
    let logs_dir = logs_dir.as_ref();
    let env_dir = env_path("LOG_DIR").or_else(|| env_path("RADROOTS_LOG_DIR"));
    let env_file = env_value("LOG_FILE").or_else(|| env_value("RADROOTS_LOG_FILE"));
    let env_level = env_value("LOG_LEVEL").or_else(|| env_value("RUST_LOG"));
    let dir = resolve_log_dir(logs_dir, env_dir);
    let opts = LoggingOptions {
        dir,
        file_name: env_file.unwrap_or_else(default_log_file_name),
        stdout: true,
        default_level: resolve_default_level(env_level, default_level),
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
    let exe = std::env::current_exe().ok()?;
    let name = exe.file_stem()?.to_string_lossy();
    log_name_from_stem(name.as_ref())
}

fn log_name_from_stem(stem: &str) -> Option<String> {
    if stem.is_empty() {
        None
    } else {
        Some(format!("{stem}.log"))
    }
}

fn env_value(key: &str) -> Option<String> {
    let value = std::env::var(key).ok()?;
    normalize_env_value(&value)
}

fn env_path(key: &str) -> Option<PathBuf> {
    env_value(key).map(PathBuf::from)
}

fn normalize_env_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn resolve_log_dir(logs_dir: &Path, env_dir: Option<PathBuf>) -> Option<PathBuf> {
    env_dir.or_else(|| {
        if logs_dir.as_os_str().is_empty() {
            None
        } else {
            Some(logs_dir.to_path_buf())
        }
    })
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
        default_log_file_name, default_log_file_name_from_exe_name, env_path, env_value, init,
        init_with, log_name_from_stem, normalize_env_value, resolve_default_level, resolve_log_dir,
    };
    use std::path::{Path, PathBuf};
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
        let home_path = env_path("HOME").expect("home path");
        assert_eq!(home_path, PathBuf::from(home));
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
    fn resolve_log_dir_prefers_env_and_handles_empty_logs_dir() {
        let fallback = resolve_log_dir(Path::new("logs"), None);
        assert_eq!(fallback, Some(PathBuf::from("logs")));

        let empty = resolve_log_dir(Path::new(""), None);
        assert_eq!(empty, None);

        let env_dir = PathBuf::from("env-logs");
        let resolved = resolve_log_dir(Path::new("logs"), Some(env_dir.clone()));
        assert_eq!(resolved, Some(env_dir));
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
    fn init_paths_execute() {
        let dir = tempdir().expect("tempdir");
        let first = init_with(dir.path(), Some("info"));
        assert!(first.is_ok());
        let owned_path = dir.path().to_path_buf();
        let third = init_with(owned_path.as_path(), None);
        assert!(third.is_ok());
        let fourth = init_with("logs", Some("debug"));
        assert!(fourth.is_ok());
        let second = init();
        assert!(second.is_ok());
    }
}

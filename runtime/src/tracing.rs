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
    let dir = env_dir.or_else(|| {
        if logs_dir.as_os_str().is_empty() {
            None
        } else {
            Some(logs_dir.to_path_buf())
        }
    });
    let opts = LoggingOptions {
        dir,
        file_name: env_file.unwrap_or_else(default_log_file_name),
        stdout: true,
        default_level: env_level.or_else(|| default_level.map(str::to_string)),
    };
    radroots_log::init_logging(opts)?;
    Ok(())
}

fn default_log_file_name() -> String {
    log_name_from_exe().unwrap_or_else(|| format!("{}.log", env!("CARGO_PKG_NAME")))
}

fn log_name_from_exe() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let name = exe.file_stem()?.to_string_lossy();
    if name.is_empty() {
        None
    } else {
        Some(format!("{name}.log"))
    }
}

fn env_value(key: &str) -> Option<String> {
    let value = std::env::var(key).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    env_value(key).map(PathBuf::from)
}

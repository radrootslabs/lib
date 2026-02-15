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
    let env_dir = std::env::var("RADROOTS_LOG_DIR").ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(PathBuf::from(trimmed))
        }
    });
    let env_file = std::env::var("RADROOTS_LOG_FILE").ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
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
        default_level: default_level.map(str::to_string),
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

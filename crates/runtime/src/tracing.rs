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
        &current_path_resolver(),
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
    use std::cell::{Cell, RefCell};

    use radroots_runtime_paths::RadrootsPathResolver;

    thread_local! {
        static IGNORE_ENV: Cell<bool> = const { Cell::new(false) };
        static CURRENT_RESOLVER: RefCell<Option<RadrootsPathResolver>> = RefCell::new(None);
    }

    pub fn set_ignore_env(ignore: bool) {
        IGNORE_ENV.with(|state| state.set(ignore));
    }

    pub fn ignore_env() -> bool {
        IGNORE_ENV.with(Cell::get)
    }

    pub fn set_current_resolver(resolver: Option<RadrootsPathResolver>) {
        CURRENT_RESOLVER.with(|state| *state.borrow_mut() = resolver);
    }

    pub fn current_resolver() -> Option<RadrootsPathResolver> {
        CURRENT_RESOLVER.with(|state| state.borrow().clone())
    }
}

fn current_path_resolver() -> RadrootsPathResolver {
    #[cfg(test)]
    if let Some(resolver) = test_hooks::current_resolver() {
        return resolver;
    }

    RadrootsPathResolver::current()
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
mod tests;

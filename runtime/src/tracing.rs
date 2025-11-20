use std::fs;
use std::path::{Path, PathBuf};
use tracing_appender::rolling;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry, fmt, prelude::*};

use crate::error::RuntimeTracingError;

pub fn init() -> Result<(), RuntimeTracingError> {
    init_with("logs", None)
}

pub fn init_with(
    logs_dir: impl AsRef<Path>,
    default_level: Option<&str>,
) -> Result<(), RuntimeTracingError> {
    let logs_dir = logs_dir.as_ref();
    ensure_dir(logs_dir)?;

    let file_appender = rolling::daily(logs_dir, concat!(env!("CARGO_PKG_NAME"), ".log"));
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    std::mem::forget(guard);

    let stdout_layer = fmt::layer().with_writer(std::io::stdout).with_target(false);

    let file_layer = fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false)
        .with_target(false);

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_level.unwrap_or("info")));

    Registry::default()
        .with(filter)
        .with(stdout_layer)
        .with(file_layer)
        .try_init()?;

    Ok(())
}

fn ensure_dir(dir: &Path) -> Result<(), RuntimeTracingError> {
    if dir.exists() {
        return Ok(());
    }
    fs::create_dir_all(dir).map_err(|source| RuntimeTracingError::CreateLogsDir {
        path: PathBuf::from(dir),
        source,
    })
}

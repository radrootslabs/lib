use std::fs;
use std::sync::OnceLock;

use tracing::info;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

use crate::options::LoggingOptions;
use crate::Result;

static GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();
static INIT: OnceLock<()> = OnceLock::new();

pub fn init_logging(opts: LoggingOptions) -> Result<()> {
    if INIT.get().is_some() {
        return Ok(());
    }

    let writer = if let Some(dir) = &opts.dir {
        fs::create_dir_all(dir)?;
        let file_appender = tracing_appender::rolling::daily(dir, &opts.file_name);
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        let _ = GUARD.set(guard);
        Some(non_blocking)
    } else {
        None
    };

    let env = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(opts.default_level.as_deref().unwrap_or("info")));
    let fmt_layer_file = writer
        .as_ref()
        .map(|w| fmt::layer().with_writer(w.clone()).with_ansi(false).with_target(false));
    let fmt_layer_stdout = if opts.also_stdout() {
        Some(fmt::layer().with_writer(std::io::stdout).with_target(false))
    } else {
        None
    };

    let subscriber = tracing_subscriber::registry()
        .with(env)
        .with(fmt_layer_file)
        .with(fmt_layer_stdout);

    subscriber.try_init()?;
    let _ = INIT.set(());
    info!(
        "logging initialized (file: {}, stdout: {})",
        opts.dir
            .as_ref()
            .map(|d| d.join(&opts.file_name).display().to_string())
            .unwrap_or_else(|| "<disabled>".into()),
        opts.also_stdout()
    );
    Ok(())
}

pub fn init_stdout() -> Result<()> {
    init_logging(LoggingOptions {
        dir: None,
        file_name: "radroots.log".into(),
        stdout: true,
        default_level: None,
    })
}

use std::fs;
use std::sync::OnceLock;

use tracing::{Level, info};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

use crate::options::LoggingOptions;
use crate::{Error, Result};

static GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();
static INIT: OnceLock<()> = OnceLock::new();

pub fn init_logging(opts: LoggingOptions) -> Result<()> {
    if INIT.get().is_some() {
        return Ok(());
    }

    let writer = if let Some(dir) = &opts.dir {
        fs::create_dir_all(dir).map_err(|_| Error::Init("mkdir"))?;
        let file_appender = tracing_appender::rolling::daily(dir, &opts.file_name);
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        let _ = GUARD.set(guard);
        Some(non_blocking)
    } else {
        None
    };

    let env = EnvFilter::from_default_env().add_directive(Level::INFO.into());
    let fmt_layer_file = writer.as_ref().map(|w| fmt::layer().with_writer(w.clone()));
    let fmt_layer_stdout = if opts.also_stdout() {
        Some(fmt::layer())
    } else {
        None
    };

    let subscriber = tracing_subscriber::registry()
        .with(env)
        .with(fmt_layer_file)
        .with(fmt_layer_stdout);

    match subscriber.try_init() {
        Ok(()) => {
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
        Err(_) => Ok(()),
    }
}

pub fn init_stdout() -> Result<()> {
    init_logging(LoggingOptions {
        dir: None,
        file_name: "radroots.log".into(),
        stdout: true,
    })
}

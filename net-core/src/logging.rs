use crate::error::{NetError, Result};
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Clone)]
pub struct LoggingOptions {
    pub dir: Option<PathBuf>,
    pub file_name: String,
    pub also_stdout: bool,
}

impl Default for LoggingOptions {
    fn default() -> Self {
        Self {
            dir: None,
            file_name: "radroots_net_core.log".into(),
            also_stdout: true,
        }
    }
}

pub fn init_logging(opts: LoggingOptions) -> Result<()> {
    let log_opts = radroots_log::LoggingOptions {
        dir: opts.dir.clone(),
        file_name: opts.file_name.clone(),
        stdout: opts.also_stdout,
        default_level: None,
    };
    radroots_log::init_logging(log_opts).map_err(|_| NetError::LoggingInit("init"))?;
    info!(
        "logging initialized (file: {}, stdout: {})",
        opts.dir
            .as_ref()
            .map(|d| d.join(&opts.file_name).display().to_string())
            .unwrap_or_else(|| "<disabled>".into()),
        opts.also_stdout
    );
    Ok(())
}

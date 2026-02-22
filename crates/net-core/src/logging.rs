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
    match radroots_log::init_logging(log_opts) {
        Ok(()) => {}
        Err(_) => return Err(NetError::LoggingInit("init")),
    }
    let file_path = opts
        .dir
        .as_ref()
        .map(|d| d.join(&opts.file_name).display().to_string())
        .unwrap_or_else(|| "<disabled>".into());
    info!(
        "logging initialized (file: {}, stdout: {})",
        file_path, opts.also_stdout
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{LoggingOptions, init_logging};
    use crate::error::NetError;
    use std::path::PathBuf;

    #[test]
    fn logging_options_default_values_are_stable() {
        let defaults = LoggingOptions::default();
        assert_eq!(defaults.dir, None);
        assert_eq!(defaults.file_name, "radroots_net_core.log");
        assert!(defaults.also_stdout);
    }

    #[test]
    fn init_logging_covers_error_and_success_paths() {
        let invalid = init_logging(LoggingOptions {
            dir: Some(PathBuf::from("/dev/null/file")),
            file_name: "x.log".to_string(),
            also_stdout: false,
        });
        assert!(matches!(invalid, Err(NetError::LoggingInit("init"))));

        let valid_with_dir = init_logging(LoggingOptions {
            dir: Some(std::env::temp_dir().join("radroots-net-core-log-tests")),
            file_name: "ok.log".to_string(),
            also_stdout: false,
        });
        assert!(valid_with_dir.is_ok());

        let valid_without_dir = init_logging(LoggingOptions {
            dir: None,
            file_name: "ok2.log".to_string(),
            also_stdout: true,
        });
        assert!(valid_without_dir.is_ok());
    }
}

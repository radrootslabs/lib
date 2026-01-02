use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeConfigError {
    #[error("failed to load configuration from {path}: {source}")]
    Load {
        path: std::path::PathBuf,
        #[source]
        source: config::ConfigError,
    },
}

#[cfg(feature = "cli")]
#[derive(Debug, Error)]
pub enum RuntimeCliError {
    #[error(transparent)]
    Parse(#[from] clap::Error),
}

#[derive(Debug, Error)]
pub enum RuntimeTracingError {
    #[error(transparent)]
    Log(#[from] radroots_log::Error),
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error(transparent)]
    Config(#[from] RuntimeConfigError),

    #[cfg(feature = "cli")]
    #[error(transparent)]
    Cli(#[from] RuntimeCliError),

    #[error(transparent)]
    Tracing(#[from] RuntimeTracingError),
}

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
    #[error("failed to initialize tracing subscriber: {0}")]
    Init(#[from] tracing_subscriber::util::TryInitError),

    #[error("failed to create logs directory at {path}: {source}")]
    CreateLogsDir {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
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

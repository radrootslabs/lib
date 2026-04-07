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

    #[error("configuration path is required; no implicit cwd-rooted default is used")]
    MissingConfigPath,
}

#[derive(Debug, Error)]
pub enum RuntimeTracingError {
    #[error(transparent)]
    Log(#[from] radroots_log::Error),

    #[error(transparent)]
    Paths(#[from] radroots_runtime_paths::RadrootsRuntimePathsError),
}

#[derive(Debug, Error)]
pub enum RuntimeProtectedFileError {
    #[error("failed to create directory {path}: {source}")]
    CreateDir {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("protected secret file io error at {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to seal protected secret file {path}: {message}")]
    Seal {
        path: std::path::PathBuf,
        message: String,
    },
    #[error("failed to decode protected secret file {path}: {message}")]
    Decode {
        path: std::path::PathBuf,
        message: String,
    },
    #[error("failed to open protected secret file {path}: {message}")]
    Open {
        path: std::path::PathBuf,
        message: String,
    },
    #[error("failed to update secret permissions for {path}: {message}")]
    Permissions {
        path: std::path::PathBuf,
        message: String,
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

#[cfg(test)]
mod tests {
    use super::{RuntimeConfigError, RuntimeError, RuntimeProtectedFileError, RuntimeTracingError};
    use std::error::Error as _;
    use std::path::PathBuf;

    #[test]
    fn runtime_config_error_message_and_source_are_accessible() {
        let err = RuntimeConfigError::Load {
            path: PathBuf::from("config.toml"),
            source: config::ConfigError::Message("invalid config".to_string()),
        };
        let display = err.to_string();
        assert!(display.contains("config.toml"));
        assert!(display.contains("invalid config"));
        assert!(err.source().is_some());
    }

    #[test]
    fn runtime_error_conversions_include_config_and_tracing_variants() {
        let cfg = RuntimeConfigError::Load {
            path: PathBuf::from("runtime.toml"),
            source: config::ConfigError::Message("bad".to_string()),
        };
        let runtime_from_cfg: RuntimeError = cfg.into();
        assert!(runtime_from_cfg.to_string().contains("runtime.toml"));
        assert!(runtime_from_cfg.source().is_some());

        let tracing =
            RuntimeTracingError::from(radroots_log::Error::Msg("log-failure".to_string()));
        let runtime_from_tracing: RuntimeError = tracing.into();
        assert!(runtime_from_tracing.to_string().contains("log-failure"));
        assert!(runtime_from_tracing.source().is_none());

        let paths = RuntimeTracingError::from(
            radroots_runtime_paths::RadrootsRuntimePathsError::MissingHomeDir {
                platform: radroots_runtime_paths::RadrootsPlatform::Linux,
            },
        );
        let runtime_from_paths: RuntimeError = paths.into();
        assert!(runtime_from_paths.to_string().contains("home directory"));
        assert!(runtime_from_paths.source().is_none());
    }

    #[test]
    fn protected_file_error_displays_path_and_message() {
        let err = RuntimeProtectedFileError::Open {
            path: PathBuf::from("identity.secret.json"),
            message: "missing wrapping key".to_string(),
        };
        let display = err.to_string();
        assert!(display.contains("identity.secret.json"));
        assert!(display.contains("missing wrapping key"));
    }
}

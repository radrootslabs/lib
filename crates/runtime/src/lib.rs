pub mod backoff;
#[cfg(feature = "cli")]
pub mod cli;
pub mod config;
pub mod error;
pub mod json;
pub mod secret_file;
pub mod service;
pub mod signals;
pub mod tracing;

#[cfg(feature = "cli")]
pub use cli::{parse_and_load_path, parse_and_load_path_with_env_overrides};
#[cfg(feature = "cli")]
pub use cli::{parse_and_load_path_with_env_overrides_and_init, parse_and_load_path_with_init};

pub use backoff::{Backoff, BackoffConfig};

pub use config::{
    load_required_file, load_required_file_with_env, load_required_file_with_env_and_overrides,
};

#[cfg(feature = "cli")]
pub use error::RuntimeCliError;
pub use error::RuntimeProtectedFileError;
pub use error::{RuntimeConfigError, RuntimeError, RuntimeTracingError};

pub use json::{JsonFile, JsonWriteOptions, RuntimeJsonError};
pub use secret_file::{local_wrapping_key_path, open_local_secret_file, seal_local_secret_file};
pub use service::RadrootsNostrServiceConfig;
#[cfg(feature = "cli")]
pub use service::RadrootsServiceCliArgs;
pub use service::{
    DEFAULT_SERVICE_IDENTITY_PATH, default_service_bootstrap_paths, default_service_config_path,
    default_service_identity_path, default_service_logs_dir, service_bootstrap_paths_for,
};
pub use signals::shutdown_signal;
pub use tracing::{
    default_shared_runtime_logs_dir, default_shared_runtime_logs_dir_for, init, init_with,
};

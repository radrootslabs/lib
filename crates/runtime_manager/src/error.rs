use std::path::PathBuf;

use radroots_runtime_paths::RadrootsRuntimePathsError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadrootsRuntimeManagerError {
    #[error("parse runtime management contract: {0}")]
    Parse(String),
    #[error("runtime management schema `{found}` does not match `{expected}`")]
    UnexpectedSchema {
        expected: &'static str,
        found: String,
    },
    #[error("management mode `{0}` not found in runtime management contract")]
    UnknownManagementMode(String),
    #[error("management mode `{mode_id}` does not support profile `{profile}`")]
    UnsupportedProfile { mode_id: String, profile: String },
    #[error("management mode `{0}` has no shared path specification")]
    MissingPathSpec(String),
    #[error("unknown root class `{0}` in runtime management contract")]
    UnknownRootClass(String),
    #[error("runtime `{0}` has no bootstrap entry in runtime management contract")]
    UnknownBootstrapRuntime(String),
    #[error("read runtime instance registry {path}: {source}")]
    ReadRegistry {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("parse runtime instance registry {path}: {details}")]
    ParseRegistry { path: PathBuf, details: String },
    #[error("serialize runtime instance registry: {0}")]
    SerializeRegistry(String),
    #[error("create runtime instance registry parent {path}: {source}")]
    CreateRegistryParent {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("write runtime instance registry {path}: {source}")]
    WriteRegistry {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    RuntimePaths(#[from] RadrootsRuntimePathsError),
}

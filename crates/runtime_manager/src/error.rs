use std::path::PathBuf;

use radroots_runtime_paths::{RadrootsRuntimePathSelectionError, RadrootsRuntimePathsError};
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
    #[error("create directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("copy runtime binary from {from} to {to}: {source}")]
    CopyBinary {
        from: PathBuf,
        to: PathBuf,
        source: std::io::Error,
    },
    #[error("serialize runtime instance metadata: {0}")]
    SerializeInstanceMetadata(String),
    #[error("write runtime instance metadata {path}: {source}")]
    WriteInstanceMetadata {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("write managed file {path}: {source}")]
    WriteManagedFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("read managed file {path}: {source}")]
    ReadManagedFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("open runtime log file {path}: {source}")]
    OpenLogFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("spawn managed runtime process {binary_path}: {source}")]
    SpawnProcess {
        binary_path: PathBuf,
        source: std::io::Error,
    },
    #[error("write pid file {path}: {source}")]
    WritePidFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("read pid file {path}: {source}")]
    ReadPidFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("parse pid file {path}: invalid contents `{contents}`")]
    ParsePidFile { path: PathBuf, contents: String },
    #[error("remove managed path {path}: {source}")]
    RemovePath {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("set file permissions for {path}: {source}")]
    SetPermissions {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("signal pid {pid} with {signal}: {source}")]
    ExecuteProcessSignal {
        pid: u32,
        signal: String,
        source: std::io::Error,
    },
    #[error("stop pid {pid}: {details}")]
    StopProcess { pid: u32, details: String },
    #[error("unsupported archive format `{archive_format}` for {archive_path}")]
    UnsupportedArchiveFormat {
        archive_path: PathBuf,
        archive_format: String,
    },
    #[error("unpack archive {archive_path}: {source}")]
    UnpackArchive {
        archive_path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    RuntimePaths(#[from] RadrootsRuntimePathsError),
    #[error(transparent)]
    RuntimePathSelection(#[from] RadrootsRuntimePathSelectionError),
}

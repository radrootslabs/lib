use thiserror::Error;

/// Stable, path-free runtime-management failures.
///
/// Filesystem paths, file contents, and dependency-owned causes are retained
/// only inside the operation that handles them. Ordinary `Display`, `Debug`,
/// and error-chain traversal therefore cannot disclose them.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum RadrootsRuntimeManagerError {
    #[error("parse runtime management contract failed")]
    Parse,
    #[error("runtime management schema is unsupported")]
    UnexpectedSchema,
    #[error("runtime management schema version is unsupported")]
    UnexpectedSchemaVersion,
    #[error("runtime management contract violates the hardened service inventory")]
    InvalidContract,
    #[error("management mode does not support the selected profile")]
    UnsupportedProfile,
    #[error("runtime has no bootstrap entry in runtime management contract")]
    UnknownBootstrapRuntime,
    #[error("runtime is not a hardened service target")]
    UnsupportedServiceTarget,
    #[error("hardened service target is metadata-only until service integration is complete")]
    MetadataOnlyServiceTarget,
    #[error("runtime context does not share the manager path scope")]
    RuntimeContextMismatch,
    #[error("read runtime instance registry failed: {kind}")]
    ReadRegistry { kind: std::io::ErrorKind },
    #[error("parse runtime instance registry failed")]
    ParseRegistry,
    #[error("runtime instance registry schema is unsupported")]
    UnexpectedRegistrySchema,
    #[error("runtime instance registry version is unsupported")]
    UnexpectedRegistryVersion,
    #[error("runtime instance registry contains a duplicate service instance")]
    DuplicateRegistryInstance,
    #[error("serialize runtime instance registry failed")]
    SerializeRegistry,
    #[error("create runtime instance registry parent failed: {kind}")]
    CreateRegistryParent { kind: std::io::ErrorKind },
    #[error("write runtime instance registry failed: {kind}")]
    WriteRegistry { kind: std::io::ErrorKind },
    #[error("create managed runtime directory failed: {kind}")]
    CreateDirectory { kind: std::io::ErrorKind },
    #[error("copy managed runtime binary failed: {kind}")]
    CopyBinary { kind: std::io::ErrorKind },
    #[error("write managed runtime config failed: {kind}")]
    WriteManagedConfig { kind: std::io::ErrorKind },
    #[error("read managed runtime file failed: {kind}")]
    ReadManagedFile { kind: std::io::ErrorKind },
    #[error("open managed runtime log failed: {kind}")]
    OpenLogFile { kind: std::io::ErrorKind },
    #[error("spawn managed runtime process failed: {kind}")]
    SpawnProcess { kind: std::io::ErrorKind },
    #[error("write managed runtime pid failed: {kind}")]
    WritePidFile { kind: std::io::ErrorKind },
    #[error("read managed runtime pid failed: {kind}")]
    ReadPidFile { kind: std::io::ErrorKind },
    #[error("managed runtime pid is malformed")]
    ParsePidFile,
    #[error("remove manager-owned runtime path failed: {kind}")]
    RemovePath { kind: std::io::ErrorKind },
    #[error("set managed runtime file permissions failed: {kind}")]
    SetPermissions { kind: std::io::ErrorKind },
    #[error("signal managed runtime process failed: {kind}")]
    ExecuteProcessSignal { kind: std::io::ErrorKind },
    #[error("managed runtime process did not stop")]
    StopProcess,
    #[error("managed runtime archive format is unsupported")]
    UnsupportedArchiveFormat,
    #[error("unpack managed runtime archive failed: {kind}")]
    UnpackArchive { kind: std::io::ErrorKind },
    #[error("managed runtime artifact name is invalid")]
    InvalidArtifactName,
}

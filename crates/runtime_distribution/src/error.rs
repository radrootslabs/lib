use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RadrootsRuntimeDistributionError {
    #[error("parse runtime distribution contract: {0}")]
    Parse(String),
    #[error("runtime distribution schema `{found}` does not match `{expected}`")]
    UnexpectedSchema {
        expected: &'static str,
        found: String,
    },
    #[error("runtime `{0}` not found in distribution contract")]
    UnknownRuntime(String),
    #[error("runtime `{0}` is not installable through the local runtime distribution contract")]
    RuntimeNotInstallable(String),
    #[error("runtime `{0}` has no target set in the distribution contract")]
    MissingTargetSet(String),
    #[error("runtime `{runtime_id}` references unknown artifact adapter `{adapter_id}`")]
    UnknownArtifactAdapter {
        runtime_id: String,
        adapter_id: String,
    },
    #[error("channel `{0}` is not defined in the runtime distribution contract")]
    UnknownChannel(String),
    #[error("channel `{0}` is defined but not active in the runtime distribution contract")]
    InactiveChannel(String),
    #[error(
        "target set `{target_set_id}` for runtime `{runtime_id}` references unknown target `{target_id}`"
    )]
    UnknownTarget {
        runtime_id: String,
        target_set_id: String,
        target_id: String,
    },
    #[error("runtime `{runtime_id}` does not support os `{os}` arch `{arch}`")]
    UnsupportedPlatform {
        runtime_id: String,
        os: String,
        arch: String,
    },
    #[error("target `{target_id}` references unknown archive format `{archive_format_id}`")]
    UnknownArchiveFormat {
        target_id: String,
        archive_format_id: String,
    },
    #[error("target `{target_id}` for runtime `{runtime_id}` does not define an archive format")]
    MissingArchiveFormat {
        runtime_id: String,
        target_id: String,
    },
}

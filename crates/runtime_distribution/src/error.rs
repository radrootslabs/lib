use thiserror::Error;

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum RadrootsRuntimeDistributionError {
    #[error("runtime distribution contract exceeds its size limit")]
    ContractTooLarge,
    #[error("parse runtime distribution contract failed")]
    Parse,
    #[error("runtime distribution schema is unsupported")]
    UnexpectedSchema,
    #[error("runtime distribution schema version is unsupported")]
    UnexpectedSchemaVersion,
    #[error("runtime is not present in the distribution contract")]
    UnknownRuntime,
    #[error("runtime is not installable through the distribution contract")]
    RuntimeNotInstallable,
    #[error("hardened service artifact authority is deferred")]
    HardenedServiceArtifactDeferred,
    #[error("runtime has no target set in the distribution contract")]
    MissingTargetSet,
    #[error("runtime references an unknown artifact adapter")]
    UnknownArtifactAdapter,
    #[error("channel is not defined in the distribution contract")]
    UnknownChannel,
    #[error("channel is defined but not active in the distribution contract")]
    InactiveChannel,
    #[error("runtime target set references an unknown target")]
    UnknownTarget,
    #[error("runtime does not support the requested platform")]
    UnsupportedPlatform,
    #[error("target references an unknown archive format")]
    UnknownArchiveFormat,
    #[error("target does not define an archive format")]
    MissingArchiveFormat,
    #[error("service is not a hardened distribution target")]
    UnsupportedService,
    #[error("service target is not eligible for Tier-1 qualification")]
    UnsupportedServiceTarget,
}

#![forbid(unsafe_code)]

//! Reusable, service-neutral host mechanics for Radroots services.

pub mod build_info;
pub mod entropy;
pub mod error;
pub mod lifecycle;
pub mod status;
pub mod time;

pub use build_info::{
    BuildInfo, BuildInfoEnvironment, BuildInfoError, BuildInfoField, BuildMode, ContractVersions,
    StatusBuildInfo,
};
pub use entropy::{EntropyError, EntropySource, SystemEntropy};
pub use error::{HostError, HostErrorCode, HostErrorKind, SafeHostError};
pub use lifecycle::{
    CancellationToken, GracefulShutdown, ShutdownConfigError, ShutdownDisposition, ShutdownPhase,
    ShutdownPhaseFailure, ShutdownPhaseFuture, ShutdownPhaseHandler, ShutdownStartError,
    ShutdownSummary, SupervisedTaskExit, SupervisedTaskExitStatus, SupervisionFailure,
    SupervisionFailureKind, TaskClassification, TaskCompletionExpectation, TaskMetadata,
    TaskMetadataError, TaskName, TaskRegistrationError, TaskSupervisor, UnfinishedWork,
};
pub use status::{
    CONFIGURATION_SCHEMA_VERSION, CachedServiceState, CachedServiceStatePublisher,
    CachedServiceStateReader, CommonReasonCode, ConfigurationIdentity, ConfigurationSource,
    IntegrityState, PersistenceHealth, PersistenceSummary, ProviderHealth, Readiness, ReasonCode,
    ReasonCodes, SERVICE_STATUS_CONTRACT_VERSION, SERVICE_STATUS_MAX_UTF8_BYTES,
    ServiceOperationalState, ServicePhase, ServiceStatus, ServiceStatusDetail, Sha256Digest,
    StatusContractError, StatusEncodingError, StatusId, StatusModelError, StatusPublisherDropped,
    TransportHealth, UptimeMillis, cached_service_state,
};
pub use time::{
    MonotonicClock, MonotonicClockError, MonotonicDeadline, MonotonicTime, SystemMonotonicClock,
    SystemWallClock, UnixTimeSeconds, WallClock, WallClockError,
};

/// Constructs validated build information from the consuming service's compile-time environment.
///
/// Release builds fail closed when any governed build variable is absent. Debug builds use stable
/// development placeholders so ordinary local development does not require release orchestration.
#[macro_export]
macro_rules! compile_time_build_info {
    (feature_profile: $feature_profile:expr, contract_versions: $contract_versions:expr $(,)?) => {{
        let mode = if cfg!(debug_assertions) {
            $crate::BuildMode::Development
        } else {
            $crate::BuildMode::Release
        };
        $crate::BuildInfo::from_compile_time(
            mode,
            $crate::BuildInfoEnvironment {
                service_version: Some(env!("CARGO_PKG_VERSION")),
                service_commit: option_env!("RADROOTS_SERVICE_REVISION"),
                lib_revision: option_env!("RADROOTS_LIB_REVISION"),
                rust_version: option_env!("RADROOTS_RUST_VERSION"),
                target: option_env!("RADROOTS_BUILD_TARGET"),
                feature_profile: Some($feature_profile),
                contract_versions: $contract_versions,
            },
        )
    }};
}

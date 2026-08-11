#![forbid(unsafe_code)]

//! Reusable, service-neutral host mechanics for Radroots services.

pub mod admin;
pub mod build_info;
pub mod config;
pub mod entropy;
pub mod error;
pub mod lifecycle;
pub mod operations;
pub mod status;
pub mod time;

pub use admin::{
    ADMIN_CONTRACT_VERSION, ADMIN_CORRELATION_ID_MAX_UTF8_BYTES, ADMIN_ERROR_CODE_MAX_UTF8_BYTES,
    ADMIN_ERROR_MESSAGE_MAX_UTF8_BYTES, ADMIN_OPERATION_ID_MAX_UTF8_BYTES,
    AdminContractVersionError, AdminCorrelationId, AdminError, AdminErrorCode, AdminErrorCodeError,
    AdminErrorMessage, AdminErrorMessageError, AdminFailureResponse, AdminIdentifierError,
    AdminIdentifierField, AdminMutationRequest, AdminOperationId, AdminPayloadError,
    AdminPeerAuthorizationPolicy, AdminPeerAuthorizationPolicyError, AdminPeerAuthorizationSupport,
    AdminSuccessResponse, AdminTransportLimitField, AdminTransportLimitValues,
    AdminTransportLimits, AdminTransportLimitsError,
};
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use admin::{
    ADMIN_MIN_RESPONSE_BODY_UTF8_BYTES, ADMIN_ROUTE_PARAMETER_NAME_MAX_UTF8_BYTES,
    ADMIN_ROUTE_PARAMETER_VALUE_MAX_UTF8_BYTES, ADMIN_ROUTE_PATH_MAX_UTF8_BYTES, AdminClient,
    AdminClientError, AdminClientErrorKind, AdminClientTarget, AdminClientTargetError,
    AdminHttpMethod, AdminRequest, AdminRequestDecodeError, AdminRouteFailure,
    AdminRouteFailureStatus, AdminRouteOutcome, AdminRouteOutcomeError, AdminRoutePath,
    AdminRoutePathError, AdminRouteRegistrationError, AdminRouter, AdminServer,
    AdminServerConfigError, AdminServerError, UNIX_ADMIN_ACTIVE_PROBE_TIMEOUT,
    UNIX_ADMIN_GROUP_DIRECTORY_MODE, UNIX_ADMIN_GROUP_SOCKET_MODE, UNIX_ADMIN_OWNER_DIRECTORY_MODE,
    UNIX_ADMIN_OWNER_SOCKET_MODE, UnixAdminSocketBinding, UnixAdminSocketError,
    UnixAdminSocketWriterAuthority,
};
pub use build_info::{
    BuildInfo, BuildInfoEnvironment, BuildInfoError, BuildInfoField, BuildMode, ContractVersions,
    StatusBuildInfo,
};
pub use config::{
    CONFIG_DOCUMENT_MAX_UTF8_BYTES, CONFIG_SCHEMA_ID_MAX_UTF8_BYTES, ConfigDocumentError,
    ConfigDocumentErrorKind, ConfigDocumentExpectation, ConfigDocumentExpectationError,
    ConfigDocumentLocation, load_config_document,
};
pub use entropy::{EntropyError, EntropySource, SystemEntropy};
pub use error::{HostError, HostErrorCode, HostErrorKind, SafeHostError};
pub use lifecycle::{
    CancellationToken, GracefulShutdown, ProcessSignal, ProcessSignalAction, ProcessSignalAdapter,
    ProcessSignalFuture, ProcessSignalSource, ProcessSignalSourceClosed, ProcessSignalStage,
    ShutdownConfigError, ShutdownDisposition, ShutdownPhase, ShutdownPhaseFailure,
    ShutdownPhaseFuture, ShutdownPhaseHandler, ShutdownStartError, ShutdownSummary,
    SupervisedTaskExit, SupervisedTaskExitStatus, SupervisionFailure, SupervisionFailureKind,
    TaskClassification, TaskCompletionExpectation, TaskMetadata, TaskMetadataError, TaskName,
    TaskRegistrationError, TaskSupervisor, UnfinishedWork,
};
pub use operations::{
    BoundOperationsServer, BoundedMetricsSnapshot, CommonMetricGroup, LIVEZ_PATH,
    METRICS_CONTENT_TYPE, METRICS_MAX_DESCRIPTORS, METRICS_MAX_LABELS_PER_SAMPLE,
    METRICS_MAX_RENDER_UTF8_BYTES, METRICS_MAX_SAMPLES, METRICS_PATH, MetricComponentId,
    MetricDescriptor, MetricHealthState, MetricKind, MetricLabel, MetricLabelKey, MetricName,
    MetricSample, MetricTaskOutcome, MetricValue, MetricsContractError, MetricsRenderError,
    OPERATIONS_HEALTH_CONTENT_TYPE, OPERATIONS_HTTP_MIN_HEADER_BYTES, OperationsBindPolicy,
    OperationsConfigError, OperationsConfigField, OperationsHealthResponse,
    OperationsListenAddress, OperationsListenAddressError, OperationsListenerConfig,
    OperationsServer, OperationsServerError, OperationsTransportLimitField,
    OperationsTransportLimitValues, OperationsTransportLimits, OperationsTransportLimitsError,
    READYZ_PATH, StableRelayId, livez, readyz,
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

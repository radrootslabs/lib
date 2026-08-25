//! Versioned, bounded local-administration protocol models.

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod client;
mod limits;
mod model;
mod peer;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod server;
#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod test_support;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix;

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use client::{
    AdminClient, AdminClientError, AdminClientErrorKind, AdminClientTarget, AdminClientTargetError,
};
pub use limits::{
    AdminTransportLimitField, AdminTransportLimitValues, AdminTransportLimits,
    AdminTransportLimitsError,
};
pub use model::{
    ADMIN_CONTRACT_VERSION, ADMIN_CORRELATION_ID_MAX_UTF8_BYTES, ADMIN_ERROR_CODE_MAX_UTF8_BYTES,
    ADMIN_ERROR_MESSAGE_MAX_UTF8_BYTES, ADMIN_OPERATION_ID_MAX_UTF8_BYTES,
    AdminContractVersionError, AdminCorrelationId, AdminError, AdminErrorCode, AdminErrorCodeError,
    AdminErrorMessage, AdminErrorMessageError, AdminFailureResponse, AdminIdentifierError,
    AdminIdentifierField, AdminMutationRequest, AdminOperationId, AdminPayloadError,
    AdminSuccessResponse,
};
pub use peer::{
    AdminPeerAuthorizationPolicy, AdminPeerAuthorizationPolicyError, AdminPeerAuthorizationSupport,
};
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use server::{
    ADMIN_MIN_RESPONSE_BODY_UTF8_BYTES, ADMIN_ROUTE_PARAMETER_NAME_MAX_UTF8_BYTES,
    ADMIN_ROUTE_PARAMETER_VALUE_MAX_UTF8_BYTES, ADMIN_ROUTE_PATH_MAX_UTF8_BYTES, AdminHttpMethod,
    AdminRequest, AdminRequestDecodeError, AdminRouteFailure, AdminRouteFailureStatus,
    AdminRouteOutcome, AdminRouteOutcomeError, AdminRoutePath, AdminRoutePathError,
    AdminRouteRegistrationError, AdminRouter, AdminServer, AdminServerConfigError,
    AdminServerError,
};
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use unix::{
    UNIX_ADMIN_ACTIVE_PROBE_TIMEOUT, UNIX_ADMIN_GROUP_DIRECTORY_MODE, UNIX_ADMIN_GROUP_SOCKET_MODE,
    UNIX_ADMIN_OWNER_DIRECTORY_MODE, UNIX_ADMIN_OWNER_SOCKET_MODE, UnixAdminSocketBinding,
    UnixAdminSocketError, UnixAdminSocketWriterAuthority,
};

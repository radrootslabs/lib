//! Versioned, bounded local-administration protocol models.

mod limits;
mod model;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix;

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
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use unix::{
    UNIX_ADMIN_ACTIVE_PROBE_TIMEOUT, UNIX_ADMIN_OWNER_DIRECTORY_MODE, UNIX_ADMIN_OWNER_SOCKET_MODE,
    UnixAdminSocketBinding, UnixAdminSocketError, UnixAdminSocketWriterAuthority,
};

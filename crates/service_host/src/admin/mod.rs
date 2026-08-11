//! Versioned, bounded local-administration protocol models.

mod model;

pub use model::{
    ADMIN_CONTRACT_VERSION, ADMIN_CORRELATION_ID_MAX_UTF8_BYTES, ADMIN_ERROR_CODE_MAX_UTF8_BYTES,
    ADMIN_ERROR_MESSAGE_MAX_UTF8_BYTES, ADMIN_OPERATION_ID_MAX_UTF8_BYTES,
    AdminContractVersionError, AdminCorrelationId, AdminError, AdminErrorCode, AdminErrorCodeError,
    AdminErrorMessage, AdminErrorMessageError, AdminFailureResponse, AdminIdentifierError,
    AdminIdentifierField, AdminMutationRequest, AdminOperationId, AdminPayloadError,
    AdminSuccessResponse,
};

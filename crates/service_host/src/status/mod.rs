//! Common service lifecycle and status value objects.

mod phase;
mod reason;
mod service;

pub use phase::{Readiness, ServiceOperationalState, ServicePhase, StatusContractError};
pub use reason::{CommonReasonCode, REASON_CODES_MAX_ITEMS, ReasonCode, ReasonCodes};
pub use service::{
    CONFIGURATION_SCHEMA_VERSION, ConfigurationIdentity, ConfigurationSource, IntegrityState,
    PersistenceHealth, PersistenceSummary, ProviderHealth, SERVICE_STATUS_CONTRACT_VERSION,
    SERVICE_STATUS_MAX_UTF8_BYTES, ServiceStatus, ServiceStatusDetail, Sha256Digest,
    StatusEncodingError, StatusId, StatusModelError, TransportHealth, UptimeMillis,
};

//! Common service lifecycle and status value objects.

mod cache;
mod phase;
mod reason;
mod service;

pub use cache::{
    CachedServiceState, CachedServiceStatePublisher, CachedServiceStateReader,
    StatusPublisherDropped, cached_service_state,
};
pub use phase::{Readiness, ServiceOperationalState, ServicePhase, StatusContractError};
pub use reason::{CommonReasonCode, ReasonCode, ReasonCodes};
pub use service::{
    CONFIGURATION_SCHEMA_VERSION, ConfigurationIdentity, ConfigurationSource, IntegrityState,
    PersistenceHealth, PersistenceSummary, ProviderHealth, SERVICE_STATUS_CONTRACT_VERSION,
    SERVICE_STATUS_MAX_UTF8_BYTES, ServiceStatus, ServiceStatusDetail, Sha256Digest,
    StatusEncodingError, StatusId, StatusModelError, TransportHealth, UptimeMillis,
};

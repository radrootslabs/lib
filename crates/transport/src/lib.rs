#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

extern crate alloc;

pub mod capability;
mod delivery;
pub mod endpoint;
pub mod error;
mod id;
mod kind;
mod message;
pub mod outcome;
mod payload;
pub mod policy;
mod reticulum;
pub mod sink;
pub mod source;
mod status;
pub mod target;
mod transport;

pub use delivery::{
    RADROOTS_TRANSPORT_DELIVERY_REQUEST_ID_MAX_BYTES, RadrootsTransportDeliveryReceipt,
    RadrootsTransportDeliveryRequest, RadrootsTransportSatisfactionClass,
    RadrootsTransportSatisfactionPolicy, RadrootsTransportTargetReceipt,
};
pub use error::{Error, RadrootsTransportError};
pub use id::{RadrootsTransportKind, TRANSPORT_ID_MAX_BYTES, TransportId};
pub use kind::{
    RadrootsTransportCapabilityAvailability, RadrootsTransportCapabilityMaturity,
    RadrootsTransportImplementationState,
};
pub use message::{
    RADROOTS_RETICULUM_ENDPOINT_URI, RADROOTS_RETICULUM_SCOPE_ID,
    RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE,
};
pub use payload::RadrootsTransportPayload;
pub use reticulum::{
    ReticulumCapabilityReportV1, ReticulumDestinationV1, ReticulumDuplicateFragmentBehaviorV1,
    ReticulumFragmentIntegrityV1, ReticulumFragmentPolicyV1, ReticulumFragmentationModeV1,
    ReticulumGatewaySemanticsV1, ReticulumPayloadPolicyV1, ReticulumPrivacySemanticsV1,
    ReticulumRoutingMetadataV1,
};
pub use sink::{DeliveryReceipt, DeliveryRequest, EventSink, SinkStatus};
pub use source::{BoxFuture, EventSource, FetchPage, FetchRequest, SourceStatus};
pub use status::{
    RadrootsTransportCapabilities, RadrootsTransportDeliveryTargetStatus, RadrootsTransportOutcome,
    RadrootsTransportOutcomeKind, RadrootsTransportStatus,
};
pub use target::{
    RadrootsTransportMeshScopeId, RadrootsTransportTarget, RadrootsTransportTargetFingerprint,
    RadrootsTransportTargetLabel, RadrootsTransportTargetSet, RadrootsTransportTargetUri,
};
pub use transport::{
    RadrootsTransport, RadrootsTransportFetchReceipt, RadrootsTransportFetchRequest,
    RadrootsTransportFuture,
};

#[cfg(test)]
extern crate self as radroots_transport;

#[cfg(test)]
extern crate std;

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
#[path = "../tests/transport.rs"]
mod tests;

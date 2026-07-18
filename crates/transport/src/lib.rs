#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

extern crate alloc;

mod delivery;
mod error;
mod kind;
mod message;
mod payload;
mod reticulum;
mod status;
mod target;
mod transport;

pub use delivery::{
    RadrootsTransportDeliveryReceipt, RadrootsTransportDeliveryRequest,
    RadrootsTransportSatisfactionClass, RadrootsTransportSatisfactionPolicy,
    RadrootsTransportTargetReceipt,
};
pub use error::RadrootsTransportError;
pub use kind::{
    RadrootsTransportCapabilityAvailability, RadrootsTransportCapabilityMaturity,
    RadrootsTransportImplementationState, RadrootsTransportKind,
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

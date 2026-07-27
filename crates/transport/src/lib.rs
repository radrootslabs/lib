#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

extern crate alloc;

mod delivery;
mod error;
mod kind;
mod limits;
mod message;
mod payload;
mod reticulum;
#[cfg(feature = "serde")]
mod serde_bounds;
mod status;
mod target;
mod transport;

pub use delivery::{
    RADROOTS_TRANSPORT_DELIVERY_REQUEST_ID_MAX_BYTES, RadrootsTransportDeliveryReceipt,
    RadrootsTransportDeliveryRequest, RadrootsTransportSatisfactionClass,
    RadrootsTransportSatisfactionPolicy, RadrootsTransportSatisfactionPolicyKind,
    RadrootsTransportTargetReceipt,
};
pub use error::RadrootsTransportError;
pub use kind::{
    RadrootsTransportCapabilityAvailability, RadrootsTransportCapabilityMaturity,
    RadrootsTransportImplementationState, RadrootsTransportKind,
};
pub use limits::{
    RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES, RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES,
    RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT, RADROOTS_TRANSPORT_FETCH_FILTER_MAX_BYTES,
    RADROOTS_TRANSPORT_FETCH_FILTER_MAX_COUNT, RADROOTS_TRANSPORT_FETCH_FILTERS_MAX_BYTES,
    RADROOTS_TRANSPORT_FETCH_RAW_ITEM_MAX_COUNT, RADROOTS_TRANSPORT_FETCH_RAW_JSON_MAX_BYTES,
    RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES, RADROOTS_TRANSPORT_OPAQUE_PAYLOAD_MAX_BYTES,
    RADROOTS_TRANSPORT_OUTCOME_CODE_MAX_BYTES, RADROOTS_TRANSPORT_OUTCOME_MESSAGE_MAX_BYTES,
    RADROOTS_TRANSPORT_RETICULUM_PAYLOAD_MAX_BYTES, RADROOTS_TRANSPORT_SIGNED_EVENT_JSON_MAX_BYTES,
    RADROOTS_TRANSPORT_TARGET_LABEL_MAX_BYTES, RADROOTS_TRANSPORT_TARGET_MAX_COUNT,
    RADROOTS_TRANSPORT_TARGET_SCOPE_MAX_BYTES, RADROOTS_TRANSPORT_TOTAL_DEADLINE_MAX_MS,
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
    RadrootsTransportOutcomeKind, RadrootsTransportRetryClass, RadrootsTransportStatus,
};
pub use target::{
    RADROOTS_TRANSPORT_TARGET_FINGERPRINT_BYTES, RadrootsTransportMeshScopeId,
    RadrootsTransportTarget, RadrootsTransportTargetFingerprint, RadrootsTransportTargetLabel,
    RadrootsTransportTargetSet, RadrootsTransportTargetUri,
};
pub use transport::{
    RADROOTS_TRANSPORT_FETCH_REQUEST_ID_MAX_BYTES, RadrootsTransport,
    RadrootsTransportFetchReceipt, RadrootsTransportFetchRequest, RadrootsTransportFuture,
};

#[cfg(test)]
extern crate self as radroots_transport;

#[cfg(test)]
extern crate std;

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
#[path = "../tests/transport.rs"]
mod tests;

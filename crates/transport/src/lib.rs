#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod delivery;
mod error;
mod kind;
mod message;
mod payload;
mod status;
mod target;
mod transport;

pub use delivery::{
    RadrootsTransportDeliveryReceipt, RadrootsTransportDeliveryRequest,
    RadrootsTransportSatisfactionClass, RadrootsTransportSatisfactionPolicy,
    RadrootsTransportTargetReceipt,
};
pub use error::RadrootsTransportError;
pub use kind::{RadrootsTransportImplementationState, RadrootsTransportKind};
pub use message::{
    RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI, RADROOTS_RETICULUM_PREVIEW_SCOPE_ID,
    RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE,
};
pub use payload::RadrootsTransportPayload;
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

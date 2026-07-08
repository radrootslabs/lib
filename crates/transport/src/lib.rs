#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod delivery;
mod error;
mod kind;
mod message;
mod status;
mod target;

pub use delivery::{
    RadrootsTransportDeliveryReceipt, RadrootsTransportDeliveryRequest,
    RadrootsTransportSatisfactionClass, RadrootsTransportSatisfactionPolicy,
    RadrootsTransportTargetReceipt,
};
pub use error::RadrootsTransportError;
pub use kind::{RadrootsTransportImplementationState, RadrootsTransportKind};
pub use message::{
    RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI, RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE,
};
pub use status::{
    RadrootsTransportDeliveryTargetStatus, RadrootsTransportOutcome,
    RadrootsTransportReadinessState, RadrootsTransportStatus,
};
pub use target::{
    RadrootsTransportTarget, RadrootsTransportTargetFingerprint, RadrootsTransportTargetSet,
    RadrootsTransportTargetUri,
};

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod delivery;
mod error;
mod kind;
mod status;
mod target;

pub use delivery::{
    RadrootsTransportDeliveryReceipt, RadrootsTransportDeliveryRequest,
    RadrootsTransportSatisfactionPolicy, RadrootsTransportTargetReceipt,
};
pub use error::RadrootsTransportError;
pub use kind::{RadrootsTransportImplementationState, RadrootsTransportKind};
pub use status::{RadrootsTransportDeliveryTargetStatus, RadrootsTransportOutcome};
pub use target::{
    RadrootsTransportTarget, RadrootsTransportTargetFingerprint, RadrootsTransportTargetSet,
    RadrootsTransportTargetUri,
};

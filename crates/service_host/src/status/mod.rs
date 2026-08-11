//! Common service lifecycle and status value objects.

mod phase;
mod reason;

pub use phase::{Readiness, ServiceOperationalState, ServicePhase, StatusContractError};
pub use reason::{CommonReasonCode, REASON_CODES_MAX_ITEMS, ReasonCode, ReasonCodes};

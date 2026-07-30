//! Validation ownership for canonical trade-domain inputs.
//!
//! Validated event-domain constructors and [`crate::WorkflowPlan::prepare`]
//! enforce the active invariants. Validation never implies actor authority,
//! cryptographic verification, persistence, or delivery.

/// Canonical trade-input validation failure.
pub use crate::workflow::Error as ValidationError;

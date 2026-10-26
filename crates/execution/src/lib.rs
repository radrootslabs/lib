#![no_std]
#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod diagnostic;
mod error;
#[cfg(feature = "signer_session")]
mod failure;
#[cfg(feature = "signer_session")]
pub use failure::ExecutionFailure;
/// Effectful command results, including optional bounded signer diagnostics.
#[cfg(feature = "signer_session")]
pub type ExecutionResult<T> = core::result::Result<T, ExecutionFailure>;
extern crate alloc;
mod receipt;
pub use receipt::{
    ArtifactObservation, BusinessInstallation, Delivery, DeliveryObservation, Issuance,
    OperationKind, OperationReceipt, OperationStatus, OperationValue, RecoveryRequirement,
    TargetObservation, WaitTermination,
};

#[cfg(feature = "session")]
extern crate std;

#[cfg(feature = "session")]
pub mod session;

#[cfg(feature = "workspace_session")]
pub mod workspace;

#[cfg(feature = "workspace_session")]
pub mod inbox;

pub use diagnostic::{Detail, Field, Reason};
pub use error::{Admission, Code, Error, InvalidErrorContext, OperationContext, RetryClass};

/// A result owned by the execution boundary.
pub type Result<T> = core::result::Result<T, Error>;

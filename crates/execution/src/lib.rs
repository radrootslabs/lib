#![no_std]
#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod diagnostic;
mod error;
extern crate alloc;
mod receipt;
pub use receipt::{
    ArtifactObservation, BusinessInstallation, Delivery, DeliveryObservation, Issuance,
    OperationReceipt, RecoveryRequirement, TargetObservation, WaitTermination,
};

#[cfg(feature = "session")]
extern crate std;

#[cfg(feature = "session")]
pub mod session;

#[cfg(feature = "workspace_session")]
pub mod workspace;

pub use diagnostic::{Detail, Field, Reason};
pub use error::{Admission, Code, Error, InvalidErrorContext, OperationContext, RetryClass};

/// A result owned by the execution boundary.
pub type Result<T> = core::result::Result<T, Error>;

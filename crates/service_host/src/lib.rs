#![forbid(unsafe_code)]

//! Reusable, service-neutral host mechanics for Radroots services.

pub mod error;

pub use error::{HostError, HostErrorCode, HostErrorKind, SafeHostError};

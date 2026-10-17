#![no_std]
#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod diagnostic;
mod error;

pub use diagnostic::{Detail, Field, Reason};
pub use error::{Admission, Code, Error, InvalidErrorContext, OperationContext, RetryClass};

/// A result owned by the execution boundary.
pub type Result<T> = core::result::Result<T, Error>;

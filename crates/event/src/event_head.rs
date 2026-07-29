#![forbid(unsafe_code)]

#[doc(hidden)]
#[path = "event_head/v1.rs"]
pub mod v1;

pub use v1::*;

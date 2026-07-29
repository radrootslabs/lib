#![forbid(unsafe_code)]

/// Exact package version implementing this event contract surface.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[doc(hidden)]
pub mod registry_v7;

pub use registry_v7::*;

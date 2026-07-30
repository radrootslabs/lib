//! Transport endpoint identity and validation.

pub use crate::target::{EndpointUri, TargetLabel, TargetScope};

/// Maximum encoded endpoint URI length.
pub const ENDPOINT_URI_MAX_BYTES: usize = 2_048;

/// Maximum encoded target scope length.
pub const TARGET_SCOPE_MAX_BYTES: usize = 128;

/// Maximum encoded human-readable target label length.
pub const TARGET_LABEL_MAX_BYTES: usize = 128;

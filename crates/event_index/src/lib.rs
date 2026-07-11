#![cfg_attr(not(feature = "std"), no_std)]
#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod checkpoint;
#[cfg(feature = "dto-bindgen")]
pub mod dto;
pub mod manifest;
pub mod serde_ext;
pub mod types;

pub use checkpoint::{RadrootsEventIndexIndexCheckpoint, RadrootsEventIndexShardCheckpoint};
pub use manifest::{
    RadrootsEventIndexManifest, RadrootsEventIndexManifestError, RadrootsEventIndexShardMetadata,
    validate_manifest,
};
pub use types::{RadrootsEventIndexIdRange, RadrootsEventIndexShardId};

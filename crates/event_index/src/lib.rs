//! Publish-frozen compatibility models. Use `radroots_storage::projection` for
//! new integrations. Step 313 removes this package.
#![doc(hidden)]
#![cfg_attr(not(feature = "std"), no_std)]
#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod checkpoint;
#[cfg(feature = "dto-bindgen")]
pub mod dto;
pub mod manifest;
pub mod serde_ext;
pub mod types;

pub use checkpoint::{RadrootsEventIndexCheckpoint, RadrootsEventIndexShardCheckpoint};
pub use manifest::{
    RadrootsEventIndexManifest, RadrootsEventIndexManifestError, RadrootsEventIndexShardMetadata,
    validate_manifest,
};
pub use types::{RadrootsEventIndexIdRange, RadrootsEventIndexShardId};

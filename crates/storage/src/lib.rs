//! Backend-neutral persistence abstractions for Radroots.

#![forbid(unsafe_code)]

pub mod atomic;
pub mod backup;
mod error;
pub mod event;
pub mod journal;
#[cfg(feature = "memory")]
pub mod memory;
pub mod outbox;
pub mod private_artifact;
pub mod projection;
pub mod status;

pub use backup::StorageReliability;
pub use error::Error;
pub use event::EventStore;
pub use journal::Journal;
pub use outbox::Outbox;
pub use private_artifact::PrivateArtifactStore;
pub use projection::ProjectionStore;

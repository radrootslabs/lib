#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

pub mod atomic;
pub mod authored;
pub mod authored_delivery;
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

pub use backup::StorageReliability as BackupSource;
pub use error::Error;
pub use event::EventStore;
pub use journal::Journal;
pub use outbox::Outbox;
pub use projection::ProjectionStore;
pub use status::StorageStatus;

/// Complete backend-neutral storage capability implemented by concrete stores.
pub trait Storage:
    EventStore
    + Journal
    + Outbox
    + ProjectionStore
    + private_artifact::PrivateArtifactStore
    + backup::StorageReliability
    + atomic::AtomicStorage
{
}

impl<T> Storage for T where
    T: EventStore
        + Journal
        + Outbox
        + ProjectionStore
        + private_artifact::PrivateArtifactStore
        + backup::StorageReliability
        + atomic::AtomicStorage
{
}

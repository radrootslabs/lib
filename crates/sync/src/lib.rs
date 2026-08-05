//! Executor-neutral local-first synchronization orchestration.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod engine;

pub mod ingest;
pub mod policy;
pub mod projection;
pub mod pull;
pub mod push;
pub mod status;

pub use engine::Engine;
pub use policy::Error;
pub use pull::{PullReceipt, PullRequest};
pub use push::{
    AdmissionRunReceipt, DeliveryExecutionReceipt, PushPreparation, PushRequest, PushStatus,
    SigningRunReceipt,
};
pub use status::SyncStatus;

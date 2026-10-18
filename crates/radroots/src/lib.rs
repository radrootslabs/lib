#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![warn(missing_docs)]
#![doc = include_str!("../README.md")]

pub mod client;
pub mod domain;
pub mod event;
pub mod farm;
pub mod identity;
pub mod knowledge;
pub mod listing;
pub mod signing;
pub mod storage;
pub mod sync;
pub mod trade;
pub mod transport;

#[cfg(feature = "native")]
pub use client::NativeConfig;
pub use client::{Client, CloseReceipt, SessionStatus, StartMode};
pub use radroots_domain::OperationId;
pub use radroots_execution::workspace::{CommandContext, WorkspaceSession};
pub use radroots_execution::{
    ArtifactObservation, BusinessInstallation, Delivery, DeliveryObservation, Issuance,
    OperationReceipt, RecoveryRequirement, TargetObservation, WaitTermination,
};
pub use radroots_execution::{Error, Result};
pub use radroots_storage::workspace::{AttachmentInput, AttachmentProof};

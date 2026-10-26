//! Concrete generated native calls over the one shared Radroots client.
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod client;
mod error;
mod inputs;
mod receipt;
mod signer_rejection;
mod states;
mod values;

pub use client::{NativeClient, Workspace};
pub use error::{Error, InputFailure};
pub use inputs::{CommandRequest, InquiryInput, ListingInput, ListingReference};
pub use receipt::{Artifact, DeliveryAttempt, Observation, Receipt, Target, TargetPolicy};
pub use signer_rejection::{SignerFingerprint, SignerRejection, SignerRejectionReason};
pub use states::{
    Admission, ArtifactRole, BusinessInstallation, Delivery, Issuance, OperationKind,
    RecoveryRequirement, SatisfactionClass, SessionState, StartMode, TargetObservation,
    WaitTermination,
};
pub use values::{ChatRelation, ChatValue, ListingValue, Value};

uniffi::setup_scaffolding!();

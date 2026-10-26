//! Generated origin-thread JavaScript calls over the shared browser owner.
//! Rust host startup supplies authenticated handles explicitly. No JSON factory
//! creates authority, keys or an independently executing foreign outbox.
mod client;
mod error;
mod inputs;
mod receipt;
mod signer_rejection;
mod states;
mod values;

pub use client::{BrowserClient, Workspace};
pub use error::{Error, Failure, InputFailure};
pub use inputs::{CommandRequest, InquiryInput, ListingInput, ListingReference};
pub use receipt::{Artifact, DeliveryAttempt, Observation, Receipt, Target, TargetPolicy};
pub use signer_rejection::{SignerFingerprint, SignerRejection, SignerRejectionReason};
pub use states::{
    Admission, ArtifactRole, BusinessInstallation, Delivery, Issuance, OperationKind,
    RecoveryRequirement, SatisfactionClass, SessionState, StartMode, TargetObservation,
    WaitTermination,
};
pub use values::{ChatValue, ListingValue, Value};

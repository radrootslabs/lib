//! Bounded output-only signer diagnostics, without raw response material.
use radroots::signing::{RejectedArtifact, RejectionReason};

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum SignerRejectionReason {
    TooLarge,
    InvalidArtifact,
    InvalidSignature,
    BindingMismatch,
    Unsupported,
}
#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct SignerFingerprint {
    pub sha256: Vec<u8>,
    pub covered_bytes: String,
    pub total_bytes: String,
}
#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct SignerRejection {
    pub reason: SignerRejectionReason,
    pub fingerprint: Option<SignerFingerprint>,
}
impl From<RejectedArtifact> for SignerRejection {
    fn from(value: RejectedArtifact) -> Self {
        Self {
            reason: match value.reason() {
                RejectionReason::TooLarge => SignerRejectionReason::TooLarge,
                RejectionReason::InvalidArtifact => SignerRejectionReason::InvalidArtifact,
                RejectionReason::InvalidSignature => SignerRejectionReason::InvalidSignature,
                RejectionReason::BindingMismatch => SignerRejectionReason::BindingMismatch,
                _ => SignerRejectionReason::Unsupported,
            },
            fingerprint: value.fingerprint().map(|v| SignerFingerprint {
                sha256: v.sha256().to_vec(),
                covered_bytes: v.covered_bytes().to_string(),
                total_bytes: v.total_bytes().to_string(),
            }),
        }
    }
}

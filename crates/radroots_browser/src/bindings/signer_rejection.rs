//! Output-only bounded diagnostics; no raw signer response is retained.
use radroots_signing::issuance::{RejectedArtifact, RejectionReason};
use wasm_bindgen::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[wasm_bindgen]
pub enum SignerRejectionReason {
    TooLarge,
    InvalidArtifact,
    InvalidSignature,
    BindingMismatch,
    Unsupported,
}
#[derive(Clone, Debug)]
#[wasm_bindgen]
pub struct SignerFingerprint {
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub sha256: Vec<u8>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub covered_bytes: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub total_bytes: String,
}
#[derive(Clone, Debug)]
#[wasm_bindgen]
pub struct SignerRejection {
    #[wasm_bindgen(readonly)]
    pub reason: SignerRejectionReason,
    #[wasm_bindgen(readonly, getter_with_clone)]
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

//! Stable signing operation, artifact, and signer-request identities.

use core::fmt;
use radroots_event_codec::authoring::PlanDigest;
use sha2::{Digest, Sha256};

use crate::{Error, error::Kind};

const REQUEST_ID_DOMAIN: &[u8] = b"radroots.signer_request.v1";

macro_rules! nonzero_id {
    ($name:ident, $label:literal) => {
        #[cfg_attr(feature = "serde", derive(serde::Serialize))]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 16]);

        impl $name {
            pub fn new(bytes: [u8; 16]) -> Result<Self, Error> {
                if bytes == [0; 16] {
                    return Err(Error::new(Kind::InvalidArgument));
                }
                Ok(Self(bytes))
            }

            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 16] {
                &self.0
            }

            #[must_use]
            pub fn to_hex(self) -> alloc_or_std::String {
                hex::encode(self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.debug_tuple($label).field(&self.to_hex()).finish()
            }
        }
    };
}

#[cfg(not(feature = "std"))]
mod alloc_or_std {
    pub use alloc::string::String;
}
#[cfg(feature = "std")]
mod alloc_or_std {
    pub use std::string::String;
}

nonzero_id!(SigningOperationId, "SigningOperationId");
nonzero_id!(AuthoredArtifactId, "AuthoredArtifactId");

/// Stable parent/child identity for one authored artifact signing operation.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SigningIntentId {
    operation_id: SigningOperationId,
    artifact_id: AuthoredArtifactId,
}

impl SigningIntentId {
    #[must_use]
    pub const fn new(operation_id: SigningOperationId, artifact_id: AuthoredArtifactId) -> Self {
        Self {
            operation_id,
            artifact_id,
        }
    }

    #[must_use]
    pub const fn operation_id(self) -> SigningOperationId {
        self.operation_id
    }

    #[must_use]
    pub const fn artifact_id(self) -> AuthoredArtifactId {
        self.artifact_id
    }
}

/// Deterministic identity a replay-capable remote signer must deduplicate.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SignerRequestId([u8; 32]);

impl SignerRequestId {
    #[must_use]
    pub fn derive(artifact_id: AuthoredArtifactId, plan_digest: PlanDigest) -> Self {
        let mut digest = Sha256::new();
        digest.update(REQUEST_ID_DOMAIN);
        digest.update(artifact_id.as_bytes());
        digest.update(plan_digest.as_bytes());
        Self(digest.finalize().into())
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    #[must_use]
    pub fn to_hex(self) -> alloc_or_std::String {
        hex::encode(self.0)
    }
}

impl fmt::Debug for SignerRequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SignerRequestId")
            .field(&self.to_hex())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_identities_reject_zero_and_expose_exact_bytes() {
        assert_eq!(
            SigningOperationId::new([0; 16])
                .expect_err("zero operation ID must fail")
                .kind(),
            Kind::InvalidArgument
        );
        assert_eq!(
            AuthoredArtifactId::new([0; 16])
                .expect_err("zero artifact ID must fail")
                .kind(),
            Kind::InvalidArgument
        );

        let operation = SigningOperationId::new([1; 16]).expect("operation ID");
        let artifact = AuthoredArtifactId::new([2; 16]).expect("artifact ID");
        assert_eq!(operation.as_bytes(), &[1; 16]);
        assert_eq!(artifact.as_bytes(), &[2; 16]);
        assert_eq!(operation.to_hex(), "01".repeat(16));
        assert_eq!(artifact.to_hex(), "02".repeat(16));
        assert_eq!(
            format!("{operation:?}"),
            format!("SigningOperationId(\"{}\")", "01".repeat(16))
        );
        assert_eq!(
            format!("{artifact:?}"),
            format!("AuthoredArtifactId(\"{}\")", "02".repeat(16))
        );

        let intent = SigningIntentId::new(operation, artifact);
        assert_eq!(intent.operation_id(), operation);
        assert_eq!(intent.artifact_id(), artifact);
    }

    #[test]
    fn signer_request_identity_is_deterministic_and_domain_separated() {
        let artifact = AuthoredArtifactId::new([3; 16]).expect("artifact ID");
        let digest = PlanDigest::from_bytes([4; 32]);
        let request = SignerRequestId::derive(artifact, digest);
        assert_eq!(request.as_bytes().len(), 32);
        assert_eq!(request.to_hex().len(), 64);
        assert_eq!(request, SignerRequestId::derive(artifact, digest));
        assert_eq!(
            format!("{request:?}"),
            format!("SignerRequestId(\"{}\")", request.to_hex())
        );
    }
}

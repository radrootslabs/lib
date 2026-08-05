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

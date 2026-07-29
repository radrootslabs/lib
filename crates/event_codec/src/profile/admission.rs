use core::fmt;

use radroots_event::{
    contract::{RadrootsEventContract, event_contract},
    envelope::RadrootsEventEnvelope,
    envelope::kind::KIND_PROFILE,
};

use crate::profile::inbound::{
    RadrootsInboundProfileMetadata, RadrootsProfileMetadataParseError,
    parse_inbound_profile_metadata,
};
use crate::verification::{
    RadrootsNip01VerificationError, RadrootsSignatureVerifiedEvent, verify_nip01_event,
};

/// A verified kind-0 event bound to its tolerant metadata projection.
#[derive(Clone, Debug, PartialEq)]
pub struct RadrootsAdmittedProfileEvent {
    verified_event: RadrootsSignatureVerifiedEvent,
    metadata: RadrootsInboundProfileMetadata,
}

impl RadrootsAdmittedProfileEvent {
    pub fn verified_event(&self) -> &RadrootsSignatureVerifiedEvent {
        &self.verified_event
    }

    pub fn event(&self) -> &RadrootsEventEnvelope {
        self.verified_event.event()
    }

    pub fn metadata(&self) -> &RadrootsInboundProfileMetadata {
        &self.metadata
    }

    pub fn contract(&self) -> &'static RadrootsEventContract {
        event_contract("radroots.profile.metadata.v1")
            .expect("Profile metadata contract is registry-owned")
    }

    pub fn into_parts(
        self,
    ) -> (
        RadrootsSignatureVerifiedEvent,
        RadrootsInboundProfileMetadata,
    ) {
        (self.verified_event, self.metadata)
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsProfileAdmissionError {
    Nip01Verification(RadrootsNip01VerificationError),
    InvalidKind { expected: u32, actual: u32 },
    Metadata(RadrootsProfileMetadataParseError),
}

impl RadrootsProfileAdmissionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Nip01Verification(error) => error.code(),
            Self::InvalidKind { .. } => "invalid_kind",
            Self::Metadata(error) => error.code(),
        }
    }
}

impl fmt::Display for RadrootsProfileAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nip01Verification(error) => write!(formatter, "{error}"),
            Self::InvalidKind { expected, actual } => {
                write!(
                    formatter,
                    "Profile event kind must be {expected}, got {actual}"
                )
            }
            Self::Metadata(error) => write!(formatter, "{error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsProfileAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Nip01Verification(error) => Some(error),
            Self::Metadata(error) => Some(error),
            Self::InvalidKind { .. } => None,
        }
    }
}

impl From<RadrootsNip01VerificationError> for RadrootsProfileAdmissionError {
    fn from(value: RadrootsNip01VerificationError) -> Self {
        Self::Nip01Verification(value)
    }
}

impl From<RadrootsProfileMetadataParseError> for RadrootsProfileAdmissionError {
    fn from(value: RadrootsProfileMetadataParseError) -> Self {
        Self::Metadata(value)
    }
}

/// Admits an already verified Profile event and preserves its exact envelope.
pub fn admit_verified_profile_event(
    verified_event: RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsAdmittedProfileEvent, RadrootsProfileAdmissionError> {
    let actual = verified_event.event().kind_u32();
    if actual != KIND_PROFILE {
        return Err(RadrootsProfileAdmissionError::InvalidKind {
            expected: KIND_PROFILE,
            actual,
        });
    }
    let metadata = parse_inbound_profile_metadata(verified_event.event().content())?;
    Ok(RadrootsAdmittedProfileEvent {
        verified_event,
        metadata,
    })
}

/// Verifies id and signature before parsing and admitting exact kind-0 content.
pub fn verify_and_admit_profile_event(
    event: RadrootsEventEnvelope,
) -> Result<RadrootsAdmittedProfileEvent, RadrootsProfileAdmissionError> {
    admit_verified_profile_event(verify_nip01_event(event)?)
}

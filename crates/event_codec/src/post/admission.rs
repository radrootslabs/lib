use core::fmt;

use radroots_event::{RadrootsEventEnvelope, contract::RadrootsEventContract};

use crate::{
    post::inbound::{
        RadrootsInboundPostProjection, RadrootsPostProjectionError, project_verified_post_event,
    },
    verification::{
        RadrootsNip01VerificationError, RadrootsSignatureVerifiedEvent, verify_nip01_event,
    },
};

/// A signature-and-id verified kind-1 event bound to its tolerant projection.
///
/// Admission here means admission to the public kind-1 post boundary. Reply is
/// preserved as an exclusion classification; this type does not claim strict
/// NIP-10 reply validity or relay-policy acceptance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAdmittedPostEvent {
    verified_event: RadrootsSignatureVerifiedEvent,
    projection: RadrootsInboundPostProjection,
}

impl RadrootsAdmittedPostEvent {
    pub fn verified_event(&self) -> &RadrootsSignatureVerifiedEvent {
        &self.verified_event
    }

    pub fn event(&self) -> &RadrootsEventEnvelope {
        self.verified_event.event()
    }

    pub fn projection(&self) -> &RadrootsInboundPostProjection {
        &self.projection
    }

    pub fn contract(&self) -> &'static RadrootsEventContract {
        radroots_event::contract::event_contract(self.projection.classification().contract_id())
            .expect("post projection contract IDs are registry-owned")
    }

    pub fn into_parts(
        self,
    ) -> (
        RadrootsSignatureVerifiedEvent,
        RadrootsInboundPostProjection,
    ) {
        (self.verified_event, self.projection)
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsPostAdmissionError {
    Nip01Verification(RadrootsNip01VerificationError),
    Projection(RadrootsPostProjectionError),
}

impl RadrootsPostAdmissionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Nip01Verification(error) => error.code(),
            Self::Projection(error) => error.code(),
        }
    }
}

impl fmt::Display for RadrootsPostAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nip01Verification(error) => write!(formatter, "{error}"),
            Self::Projection(error) => write!(formatter, "{error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsPostAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Nip01Verification(error) => Some(error),
            Self::Projection(error) => Some(error),
        }
    }
}

impl From<RadrootsNip01VerificationError> for RadrootsPostAdmissionError {
    fn from(value: RadrootsNip01VerificationError) -> Self {
        Self::Nip01Verification(value)
    }
}

impl From<RadrootsPostProjectionError> for RadrootsPostAdmissionError {
    fn from(value: RadrootsPostProjectionError) -> Self {
        Self::Projection(value)
    }
}

/// Admits an already verified kind-1 event and binds its tolerant projection.
pub fn admit_verified_post_event(
    verified_event: RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsAdmittedPostEvent, RadrootsPostAdmissionError> {
    let projection = project_verified_post_event(&verified_event)?;
    Ok(RadrootsAdmittedPostEvent {
        verified_event,
        projection,
    })
}

/// Verifies NIP-01 identifier/signature state before kind-1 projection.
pub fn verify_and_admit_post_event(
    event: RadrootsEventEnvelope,
) -> Result<RadrootsAdmittedPostEvent, RadrootsPostAdmissionError> {
    admit_verified_post_event(verify_nip01_event(event)?)
}

use core::fmt;

use radroots_event::{RadrootsEventEnvelope, contract::RadrootsEventContract};

use crate::{
    post::admission::RadrootsThreadExcludedPostCandidate,
    reply::inbound::{
        RadrootsInboundNip10ReplyProjection, RadrootsNip10ReplyProjectionError,
        project_verified_nip10_reply_event,
    },
    verification::{
        RadrootsNip01VerificationError, RadrootsSignatureVerifiedEvent, verify_nip01_event,
    },
};

/// A signature-and-id verified kind-1 event admitted as a NIP-10 Reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAdmittedNip10ReplyEvent {
    verified_event: RadrootsSignatureVerifiedEvent,
    projection: RadrootsInboundNip10ReplyProjection,
}

impl RadrootsAdmittedNip10ReplyEvent {
    pub fn verified_event(&self) -> &RadrootsSignatureVerifiedEvent {
        &self.verified_event
    }

    pub fn event(&self) -> &RadrootsEventEnvelope {
        self.verified_event.event()
    }

    pub fn projection(&self) -> &RadrootsInboundNip10ReplyProjection {
        &self.projection
    }

    pub fn contract(&self) -> &'static RadrootsEventContract {
        radroots_event::contract::event_contract(self.projection.contract_id())
            .expect("NIP-10 Reply contract is registry-owned")
    }

    pub fn into_parts(
        self,
    ) -> (
        RadrootsSignatureVerifiedEvent,
        RadrootsInboundNip10ReplyProjection,
    ) {
        (self.verified_event, self.projection)
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsNip10ReplyAdmissionError {
    Nip01Verification(RadrootsNip01VerificationError),
    Projection(RadrootsNip10ReplyProjectionError),
}

impl RadrootsNip10ReplyAdmissionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Nip01Verification(error) => error.code(),
            Self::Projection(error) => error.code(),
        }
    }
}

impl fmt::Display for RadrootsNip10ReplyAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nip01Verification(error) => write!(formatter, "{error}"),
            Self::Projection(error) => write!(formatter, "{error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsNip10ReplyAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Nip01Verification(error) => Some(error),
            Self::Projection(error) => Some(error),
        }
    }
}

impl From<RadrootsNip01VerificationError> for RadrootsNip10ReplyAdmissionError {
    fn from(value: RadrootsNip01VerificationError) -> Self {
        Self::Nip01Verification(value)
    }
}

impl From<RadrootsNip10ReplyProjectionError> for RadrootsNip10ReplyAdmissionError {
    fn from(value: RadrootsNip10ReplyProjectionError) -> Self {
        Self::Projection(value)
    }
}

pub fn admit_verified_nip10_reply_event(
    verified_event: RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsAdmittedNip10ReplyEvent, RadrootsNip10ReplyAdmissionError> {
    let projection = project_verified_nip10_reply_event(&verified_event)?;
    Ok(RadrootsAdmittedNip10ReplyEvent {
        verified_event,
        projection,
    })
}

/// Promotes a verified candidate already excluded from root-card admission.
pub fn admit_thread_excluded_post_candidate(
    candidate: RadrootsThreadExcludedPostCandidate,
) -> Result<RadrootsAdmittedNip10ReplyEvent, RadrootsNip10ReplyAdmissionError> {
    let (verified_event, _) = candidate.into_parts();
    admit_verified_nip10_reply_event(verified_event)
}

pub fn verify_and_admit_nip10_reply_event(
    event: RadrootsEventEnvelope,
) -> Result<RadrootsAdmittedNip10ReplyEvent, RadrootsNip10ReplyAdmissionError> {
    admit_verified_nip10_reply_event(verify_nip01_event(event)?)
}

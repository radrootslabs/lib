use core::fmt;

use radroots_event::{contract::EventContract, envelope::EventEnvelope};

use crate::{
    comment::inbound::{
        RadrootsInboundNip22CommentProjection, RadrootsNip22CommentProjectionError,
        project_verified_nip22_comment_event,
    },
    verification::{
        RadrootsNip01VerificationError, RadrootsSignatureVerifiedEvent, verify_nip01_event,
    },
};

/// A signature-and-id verified kind-1111 event admitted as a NIP-22 Comment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAdmittedNip22CommentEvent {
    verified_event: RadrootsSignatureVerifiedEvent,
    projection: RadrootsInboundNip22CommentProjection,
}

impl RadrootsAdmittedNip22CommentEvent {
    pub fn verified_event(&self) -> &RadrootsSignatureVerifiedEvent {
        &self.verified_event
    }

    pub fn event(&self) -> &EventEnvelope {
        self.verified_event.event()
    }

    pub const fn projection(&self) -> &RadrootsInboundNip22CommentProjection {
        &self.projection
    }

    pub fn contract(&self) -> &'static EventContract {
        radroots_event::contract::event_contract(self.projection.contract_id())
            .expect("NIP-22 Comment contract is registry-owned")
    }

    pub fn into_parts(
        self,
    ) -> (
        RadrootsSignatureVerifiedEvent,
        RadrootsInboundNip22CommentProjection,
    ) {
        (self.verified_event, self.projection)
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsNip22CommentAdmissionError {
    Nip01Verification(RadrootsNip01VerificationError),
    Projection(RadrootsNip22CommentProjectionError),
}

impl RadrootsNip22CommentAdmissionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Nip01Verification(error) => error.code(),
            Self::Projection(error) => error.code(),
        }
    }
}

impl fmt::Display for RadrootsNip22CommentAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nip01Verification(error) => write!(formatter, "{error}"),
            Self::Projection(error) => write!(formatter, "{error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsNip22CommentAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Nip01Verification(error) => Some(error),
            Self::Projection(error) => Some(error),
        }
    }
}

impl From<RadrootsNip01VerificationError> for RadrootsNip22CommentAdmissionError {
    fn from(value: RadrootsNip01VerificationError) -> Self {
        Self::Nip01Verification(value)
    }
}

impl From<RadrootsNip22CommentProjectionError> for RadrootsNip22CommentAdmissionError {
    fn from(value: RadrootsNip22CommentProjectionError) -> Self {
        Self::Projection(value)
    }
}

pub fn admit_verified_nip22_comment_event(
    verified_event: RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsAdmittedNip22CommentEvent, RadrootsNip22CommentAdmissionError> {
    let projection = project_verified_nip22_comment_event(&verified_event)?;
    Ok(RadrootsAdmittedNip22CommentEvent {
        verified_event,
        projection,
    })
}

pub fn verify_and_admit_nip22_comment_event(
    event: EventEnvelope,
) -> Result<RadrootsAdmittedNip22CommentEvent, RadrootsNip22CommentAdmissionError> {
    admit_verified_nip22_comment_event(verify_nip01_event(event)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_error_codes_delegate_to_verification_and_projection() {
        let verification = RadrootsNip22CommentAdmissionError::Nip01Verification(
            RadrootsNip01VerificationError::SignatureInvalid,
        );
        let projection = RadrootsNip22CommentAdmissionError::Projection(
            RadrootsNip22CommentProjectionError::ParentAuthorAmbiguous,
        );
        assert_eq!(verification.code(), "signature_invalid");
        assert_eq!(projection.code(), "comment_parent_author_ambiguous");
        assert!(!verification.to_string().is_empty());
        assert!(!projection.to_string().is_empty());
    }
}

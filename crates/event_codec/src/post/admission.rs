use core::fmt;

use radroots_event::{contract::RadrootsEventContract, envelope::RadrootsEventEnvelope};

use crate::{
    post::inbound::{
        RadrootsInboundPostProjection, RadrootsPostProjectionError, project_verified_post_event,
    },
    verification::{
        RadrootsNip01VerificationError, RadrootsSignatureVerifiedEvent, verify_nip01_event,
    },
};

/// A signature-and-id verified root kind-1 event bound to a product projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAdmittedRootPostEvent {
    verified_event: RadrootsSignatureVerifiedEvent,
    projection: RadrootsInboundPostProjection,
}

impl RadrootsAdmittedRootPostEvent {
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
        debug_assert!(self.projection.classification().is_root_card());
        radroots_event::contract::event_contract(self.projection.classification().contract_id())
            .expect("root post projection contract IDs are registry-owned")
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

/// A verified kind-1 event excluded from root-card admission by an `e` tag.
///
/// This candidate carries no Reply claim. Promotion into a semantic thread
/// model is available only through the dedicated NIP-10 admission boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsThreadExcludedPostCandidate {
    verified_event: RadrootsSignatureVerifiedEvent,
    projection: RadrootsInboundPostProjection,
}

impl RadrootsThreadExcludedPostCandidate {
    pub fn verified_event(&self) -> &RadrootsSignatureVerifiedEvent {
        &self.verified_event
    }

    pub fn event(&self) -> &RadrootsEventEnvelope {
        self.verified_event.event()
    }

    pub fn projection(&self) -> &RadrootsInboundPostProjection {
        &self.projection
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

/// Result of verifying a kind-1 event at the root-post admission boundary.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsPostAdmissionOutcome {
    Root(RadrootsAdmittedRootPostEvent),
    ThreadExcluded(RadrootsThreadExcludedPostCandidate),
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

/// Classifies an already verified kind-1 event at the product post boundary.
///
/// Root post profiles are admitted as product posts. Thread-shaped candidates
/// are returned separately and do not establish a root post or reply contract.
pub fn admit_verified_post_event(
    verified_event: RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsPostAdmissionOutcome, RadrootsPostAdmissionError> {
    let projection = project_verified_post_event(&verified_event)?;
    if projection.classification().is_root_card() {
        Ok(RadrootsPostAdmissionOutcome::Root(
            RadrootsAdmittedRootPostEvent {
                verified_event,
                projection,
            },
        ))
    } else {
        Ok(RadrootsPostAdmissionOutcome::ThreadExcluded(
            RadrootsThreadExcludedPostCandidate {
                verified_event,
                projection,
            },
        ))
    }
}

/// Verifies NIP-01 state, admitting only root profiles as product posts.
///
/// A verified thread candidate is returned as thread-excluded compatibility
/// data; this boundary does not claim that it is a valid Radroots reply.
pub fn verify_and_admit_post_event(
    event: RadrootsEventEnvelope,
) -> Result<RadrootsPostAdmissionOutcome, RadrootsPostAdmissionError> {
    admit_verified_post_event(verify_nip01_event(event)?)
}

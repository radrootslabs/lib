#[cfg(not(feature = "std"))]
use alloc::boxed::Box;

use core::fmt;

use radroots_event::{
    contract::EventContract, envelope::EventEnvelope,
    food::availability::RADROOTS_FOOD_AVAILABILITY_CONTRACT_ID,
    listing::classified::ClassifiedListingPartition,
};

use crate::{
    food_availability::inbound::{
        RadrootsFoodAvailabilityProjectionError, RadrootsFoodAvailabilityProjectionOutcome,
        RadrootsInboundFoodAvailabilityProjection, project_verified_food_availability_event,
    },
    verification::{
        RadrootsNip01VerificationError, RadrootsSignatureVerifiedEvent, verify_nip01_event,
    },
};

/// A verified focused FoodAvailability event bound to its tolerant projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAdmittedFoodAvailabilityEvent {
    verified_event: RadrootsSignatureVerifiedEvent,
    projection: RadrootsInboundFoodAvailabilityProjection,
}

impl RadrootsAdmittedFoodAvailabilityEvent {
    pub fn verified_event(&self) -> &RadrootsSignatureVerifiedEvent {
        &self.verified_event
    }

    pub fn event(&self) -> &EventEnvelope {
        self.verified_event.event()
    }

    pub fn projection(&self) -> &RadrootsInboundFoodAvailabilityProjection {
        &self.projection
    }

    pub fn contract(&self) -> &'static EventContract {
        radroots_event::contract::event_contract(RADROOTS_FOOD_AVAILABILITY_CONTRACT_ID)
            .expect("FoodAvailability contract ID is registry-owned")
    }

    pub fn into_parts(
        self,
    ) -> (
        RadrootsSignatureVerifiedEvent,
        RadrootsInboundFoodAvailabilityProjection,
    ) {
        (self.verified_event, self.projection)
    }
}

/// A verified kind-30402 event excluded from the focused Food profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsExcludedClassifiedListingCandidate {
    verified_event: RadrootsSignatureVerifiedEvent,
    partition: ClassifiedListingPartition,
}

impl RadrootsExcludedClassifiedListingCandidate {
    pub fn verified_event(&self) -> &RadrootsSignatureVerifiedEvent {
        &self.verified_event
    }

    pub fn event(&self) -> &EventEnvelope {
        self.verified_event.event()
    }

    pub const fn partition(&self) -> ClassifiedListingPartition {
        self.partition
    }

    pub fn into_parts(self) -> (RadrootsSignatureVerifiedEvent, ClassifiedListingPartition) {
        (self.verified_event, self.partition)
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsFoodAvailabilityAdmissionOutcome {
    Admitted(Box<RadrootsAdmittedFoodAvailabilityEvent>),
    Excluded(RadrootsExcludedClassifiedListingCandidate),
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsFoodAvailabilityAdmissionError {
    Nip01Verification(RadrootsNip01VerificationError),
    Projection(RadrootsFoodAvailabilityProjectionError),
}

impl RadrootsFoodAvailabilityAdmissionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Nip01Verification(error) => error.code(),
            Self::Projection(error) => error.code(),
        }
    }
}

impl fmt::Display for RadrootsFoodAvailabilityAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nip01Verification(error) => write!(formatter, "{error}"),
            Self::Projection(error) => write!(formatter, "{error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsFoodAvailabilityAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Nip01Verification(error) => Some(error),
            Self::Projection(error) => Some(error),
        }
    }
}

impl From<RadrootsNip01VerificationError> for RadrootsFoodAvailabilityAdmissionError {
    fn from(value: RadrootsNip01VerificationError) -> Self {
        Self::Nip01Verification(value)
    }
}

impl From<RadrootsFoodAvailabilityProjectionError> for RadrootsFoodAvailabilityAdmissionError {
    fn from(value: RadrootsFoodAvailabilityProjectionError) -> Self {
        Self::Projection(value)
    }
}

/// Admits or explicitly excludes an already verified classified listing.
pub fn admit_verified_food_availability_event(
    verified_event: RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsFoodAvailabilityAdmissionOutcome, RadrootsFoodAvailabilityAdmissionError> {
    match project_verified_food_availability_event(&verified_event)? {
        RadrootsFoodAvailabilityProjectionOutcome::Focused(projection) => {
            Ok(RadrootsFoodAvailabilityAdmissionOutcome::Admitted(
                Box::new(RadrootsAdmittedFoodAvailabilityEvent {
                    verified_event,
                    projection: *projection,
                }),
            ))
        }
        RadrootsFoodAvailabilityProjectionOutcome::Excluded(partition) => {
            debug_assert!(matches!(
                partition,
                ClassifiedListingPartition::OperationalListing
                    | ClassifiedListingPartition::GenericNip99
            ));
            Ok(RadrootsFoodAvailabilityAdmissionOutcome::Excluded(
                RadrootsExcludedClassifiedListingCandidate {
                    verified_event,
                    partition,
                },
            ))
        }
    }
}

/// Verifies NIP-01 id and signature before focused Food admission.
pub fn verify_and_admit_food_availability_event(
    event: EventEnvelope,
) -> Result<RadrootsFoodAvailabilityAdmissionOutcome, RadrootsFoodAvailabilityAdmissionError> {
    admit_verified_food_availability_event(verify_nip01_event(event)?)
}

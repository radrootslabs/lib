use core::fmt;

use crate::{
    food_availability::inbound::{
        RadrootsFoodAvailabilityProjectionError, project_strict_verified_food_availability_event,
    },
    verification::RadrootsSignatureVerifiedEvent,
};

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsFoodAvailabilityRevisionError {
    PreviousInvalid(RadrootsFoodAvailabilityProjectionError),
    CurrentInvalid(RadrootsFoodAvailabilityProjectionError),
    CoordinateChanged,
    PublishedAtChanged,
    NotNewer,
}

impl RadrootsFoodAvailabilityRevisionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::PreviousInvalid(_) => "food_revision_previous_invalid",
            Self::CurrentInvalid(_) => "food_revision_current_invalid",
            Self::CoordinateChanged => "food_coordinate_changed",
            Self::PublishedAtChanged => "food_published_at_changed",
            Self::NotNewer => "food_revision_not_newer",
        }
    }
}

impl fmt::Display for RadrootsFoodAvailabilityRevisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PreviousInvalid(error) => {
                write!(
                    formatter,
                    "previous FoodAvailability revision is invalid: {error}"
                )
            }
            Self::CurrentInvalid(error) => {
                write!(
                    formatter,
                    "current FoodAvailability revision is invalid: {error}"
                )
            }
            Self::CoordinateChanged => {
                formatter.write_str("FoodAvailability revision coordinate changed")
            }
            Self::PublishedAtChanged => {
                formatter.write_str("FoodAvailability revision published_at changed")
            }
            Self::NotNewer => {
                formatter.write_str("FoodAvailability revision does not win NIP-01 ordering")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsFoodAvailabilityRevisionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PreviousInvalid(error) | Self::CurrentInvalid(error) => Some(error),
            _ => None,
        }
    }
}

/// Validates a replacement using strict authored wire semantics on both sides.
///
/// Both inputs must already have passed NIP-01 id and signature verification.
pub fn validate_food_availability_revision(
    previous: &RadrootsSignatureVerifiedEvent,
    current: &RadrootsSignatureVerifiedEvent,
) -> Result<(), RadrootsFoodAvailabilityRevisionError> {
    let previous_projection = project_strict_verified_food_availability_event(previous)
        .map_err(RadrootsFoodAvailabilityRevisionError::PreviousInvalid)?;
    let current_projection = project_strict_verified_food_availability_event(current)
        .map_err(RadrootsFoodAvailabilityRevisionError::CurrentInvalid)?;
    let previous_event = previous.event();
    let current_event = current.event();

    if previous_event.kind_u32() != current_event.kind_u32()
        || previous_event.author().to_hex() != current_event.author().to_hex()
        || previous_projection.identifier() != current_projection.identifier()
    {
        return Err(RadrootsFoodAvailabilityRevisionError::CoordinateChanged);
    }
    if previous_projection.published_at() != current_projection.published_at() {
        return Err(RadrootsFoodAvailabilityRevisionError::PublishedAtChanged);
    }

    let current_wins = current_event.created_at_u64() > previous_event.created_at_u64()
        || (current_event.created_at_u64() == previous_event.created_at_u64()
            && current_event.id_str() < previous_event.id_str());
    if !current_wins {
        return Err(RadrootsFoodAvailabilityRevisionError::NotNewer);
    }
    Ok(())
}

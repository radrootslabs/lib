#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::string::String;

use crate::listing::operational::RadrootsOperationalListingParseError;

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsOperationalListingValidationError {
    InvalidKind {
        kind: u32,
    },
    InvalidProfile,
    MissingListingId,
    ListingEventNotFound {
        listing_addr: String,
    },
    ListingEventFetchFailed {
        listing_addr: String,
    },
    ParseError {
        error: RadrootsOperationalListingParseError,
    },
    InvalidSeller,
    MissingFarmProfile,
    MissingFarmRecord,
    MissingTitle,
    MissingDescription,
    MissingProductType,
    MissingBins,
    MissingPrimaryBin,
    InvalidBin,
    MissingPrice,
    InvalidPrice,
    MissingInventory,
    InvalidInventory,
    MissingAvailability,
    MissingLocation,
    MissingLocationLocality,
    MissingLocationGeohash,
    InvalidLocationGeohash,
    MissingDeliveryMethod,
}

impl core::fmt::Display for RadrootsOperationalListingValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidKind { kind } => write!(f, "invalid listing kind: {kind}"),
            Self::InvalidProfile => {
                write!(
                    f,
                    "classified listing is not an Operational Listing profile"
                )
            }
            Self::MissingListingId => write!(f, "missing listing id"),
            Self::ListingEventNotFound { listing_addr } => {
                write!(f, "listing event not found: {listing_addr}")
            }
            Self::ListingEventFetchFailed { listing_addr } => {
                write!(f, "listing event fetch failed: {listing_addr}")
            }
            Self::ParseError { error } => write!(f, "invalid listing data: {error}"),
            Self::InvalidSeller => write!(f, "listing author does not match farm pubkey"),
            Self::MissingFarmProfile => write!(f, "missing farm profile"),
            Self::MissingFarmRecord => write!(f, "missing farm record"),
            Self::MissingTitle => write!(f, "missing listing title"),
            Self::MissingDescription => write!(f, "missing listing description"),
            Self::MissingProductType => write!(f, "missing listing product type"),
            Self::MissingBins => write!(f, "missing listing bins"),
            Self::MissingPrimaryBin => write!(f, "missing primary listing bin"),
            Self::InvalidBin => write!(f, "invalid listing bin"),
            Self::MissingPrice => write!(f, "missing listing price"),
            Self::InvalidPrice => write!(f, "invalid listing price"),
            Self::MissingInventory => write!(f, "missing listing inventory"),
            Self::InvalidInventory => write!(f, "invalid listing inventory"),
            Self::MissingAvailability => write!(f, "missing listing availability"),
            Self::MissingLocation => write!(f, "missing listing location"),
            Self::MissingLocationLocality => write!(f, "missing listing location locality"),
            Self::MissingLocationGeohash => write!(f, "missing listing location geohash"),
            Self::InvalidLocationGeohash => write!(f, "invalid listing location geohash"),
            Self::MissingDeliveryMethod => write!(f, "missing listing delivery method"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsOperationalListingValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listing_validation_error_display_covers_location_variants() {
        assert_eq!(
            RadrootsOperationalListingValidationError::InvalidProfile.to_string(),
            "classified listing is not an Operational Listing profile"
        );
        assert_eq!(
            RadrootsOperationalListingValidationError::MissingLocation.to_string(),
            "missing listing location"
        );
        assert_eq!(
            RadrootsOperationalListingValidationError::MissingLocationLocality.to_string(),
            "missing listing location locality"
        );
        assert_eq!(
            RadrootsOperationalListingValidationError::MissingLocationGeohash.to_string(),
            "missing listing location geohash"
        );
        assert_eq!(
            RadrootsOperationalListingValidationError::InvalidLocationGeohash.to_string(),
            "invalid listing location geohash"
        );
    }
}

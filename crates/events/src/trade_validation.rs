#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::{RadrootsNostrEventPtr, order::RadrootsListingParseError};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTradeValidationListingError {
    InvalidKind { kind: u32 },
    MissingListingId,
    ListingEventNotFound { listing_addr: String },
    ListingEventFetchFailed { listing_addr: String },
    ParseError { error: RadrootsListingParseError },
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
    MissingDeliveryMethod,
}

impl core::fmt::Display for RadrootsTradeValidationListingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidKind { kind } => write!(f, "invalid listing kind: {kind}"),
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
            Self::MissingDeliveryMethod => write!(f, "missing listing delivery method"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsTradeValidationListingError {}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeValidationListingRequest {
    pub listing_event: Option<RadrootsNostrEventPtr>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeValidationListingResult {
    pub valid: bool,
    pub errors: Vec<RadrootsTradeValidationListingError>,
}

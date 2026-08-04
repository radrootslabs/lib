//! Canonicalization for Radroots Listing v1 drafts.

#![forbid(unsafe_code)]

use core::fmt;

use std::{format, vec::Vec};

use radroots_event::{
    envelope::kind::KIND_CLASSIFIED_LISTING,
    id::{ClassifiedListingAddress, InventoryBinId, ParseError},
    listing::operational::OperationalListing,
    trade::validation::OperationalListingValidationError,
};
use radroots_identity::{Error as PublicKeyError, PublicKey};

use super::validation::validate_operational_listing_model;

#[derive(Clone, Debug)]
pub struct RadrootsOperationalListingEditDocumentV1 {
    pub listing: OperationalListing,
}

impl RadrootsOperationalListingEditDocumentV1 {
    pub fn new(listing: OperationalListing) -> Self {
        Self { listing }
    }
}

#[derive(Clone, Debug)]
pub struct RadrootsOperationalListingCanonicalEdit {
    listing: OperationalListing,
    seller_pubkey: PublicKey,
    public_listing_addr: ClassifiedListingAddress,
}

impl RadrootsOperationalListingCanonicalEdit {
    pub fn new(
        mut listing: OperationalListing,
        seller_pubkey: PublicKey,
    ) -> Result<Self, RadrootsOperationalListingEditError> {
        let farm_pubkey = PublicKey::from_hex(listing.farm.pubkey.as_str())
            .map_err(RadrootsOperationalListingEditError::InvalidFarmPubkey)?;
        if farm_pubkey != seller_pubkey {
            return Err(RadrootsOperationalListingEditError::FarmPubkeyMismatch {
                expected_pubkey: seller_pubkey,
                actual_pubkey: farm_pubkey,
            });
        }
        listing.farm.pubkey = farm_pubkey.to_hex();
        validate_listing_bins(&listing)?;
        let listing = validate_operational_listing_model(listing, &seller_pubkey)
            .map_err(RadrootsOperationalListingEditError::InvalidModel)?
            .listing;

        let public_listing_addr = listing_addr(
            KIND_CLASSIFIED_LISTING,
            &seller_pubkey,
            listing.d_tag.as_str(),
        );

        Ok(Self {
            listing,
            seller_pubkey,
            public_listing_addr,
        })
    }

    pub fn listing(&self) -> &OperationalListing {
        &self.listing
    }

    pub fn seller_pubkey(&self) -> &PublicKey {
        &self.seller_pubkey
    }

    pub fn public_listing_addr(&self) -> &ClassifiedListingAddress {
        &self.public_listing_addr
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsOperationalListingEditError {
    InvalidFarmPubkey(PublicKeyError),
    InvalidClassifiedListingAddress(ParseError),
    InvalidModel(OperationalListingValidationError),
    FarmPubkeyMismatch {
        expected_pubkey: PublicKey,
        actual_pubkey: PublicKey,
    },
    MissingPrimaryBin {
        primary_bin_id: InventoryBinId,
    },
    DuplicateBinId {
        bin_id: InventoryBinId,
    },
}

impl fmt::Display for RadrootsOperationalListingEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFarmPubkey(error) => {
                write!(f, "invalid listing edit farm pubkey: {error}")
            }
            Self::InvalidClassifiedListingAddress(error) => {
                write!(f, "invalid listing edit address: {error}")
            }
            Self::InvalidModel(error) => {
                write!(f, "invalid listing edit model: {error}")
            }
            Self::FarmPubkeyMismatch { .. } => {
                f.write_str("listing edit farm pubkey does not match seller")
            }
            Self::MissingPrimaryBin { .. } => f.write_str("listing edit primary bin is missing"),
            Self::DuplicateBinId { .. } => f.write_str("listing edit contains duplicate bin ID"),
        }
    }
}

impl core::error::Error for RadrootsOperationalListingEditError {}

fn validate_listing_bins(
    listing: &OperationalListing,
) -> Result<(), RadrootsOperationalListingEditError> {
    let primary_bin_id = listing.primary_bin_id.clone();
    let mut seen_bin_ids = Vec::new();
    let mut primary_bin_found = false;
    for bin in &listing.bins {
        if seen_bin_ids
            .iter()
            .any(|seen_bin_id| seen_bin_id == &bin.bin_id)
        {
            return Err(RadrootsOperationalListingEditError::DuplicateBinId {
                bin_id: bin.bin_id.clone(),
            });
        }
        if bin.bin_id == primary_bin_id {
            primary_bin_found = true;
        }
        seen_bin_ids.push(bin.bin_id.clone());
    }

    if !primary_bin_found {
        return Err(RadrootsOperationalListingEditError::MissingPrimaryBin { primary_bin_id });
    }
    Ok(())
}

fn listing_addr(kind: u32, seller_pubkey: &PublicKey, d_tag: &str) -> ClassifiedListingAddress {
    ClassifiedListingAddress::parse(format!("{kind}:{seller_pubkey}:{d_tag}"))
        .expect("typed listing identity must form a listing address")
}

/// Canonicalizes an untrusted listing edit for an already-authorized seller.
///
/// Actor provenance and role authorization belong to the signing/workflow
/// boundary. This deterministic function validates only the supplied public
/// identity and listing data and performs no signing or authorization.
pub fn canonicalize_operational_listing_edit(
    seller_pubkey: PublicKey,
    mut document: RadrootsOperationalListingEditDocumentV1,
) -> Result<RadrootsOperationalListingCanonicalEdit, RadrootsOperationalListingEditError> {
    let farm_pubkey = document.listing.farm.pubkey.as_str();
    if farm_pubkey.is_empty() {
        document.listing.farm.pubkey = seller_pubkey.to_hex();
    }

    RadrootsOperationalListingCanonicalEdit::new(document.listing, seller_pubkey)
}

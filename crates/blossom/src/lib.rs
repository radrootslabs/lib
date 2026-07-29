#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod authorization;
pub mod descriptor;
mod error;
pub mod hash;
pub mod media_type;
pub mod publication_readiness;
pub mod url;

pub use authorization::{
    AuthoredUploadClaim, AuthorizationAction, AuthorizationClaim, AuthorizationContent,
    AuthorizationTarget, AuthorizationValidation, AuthorizationWireParts,
    RADROOTS_BLOSSOM_AUTH_CONTENT_MAX_BYTES, RADROOTS_BLOSSOM_AUTH_MAX_CREATED_AGE_SECONDS,
    RADROOTS_BLOSSOM_AUTH_MAX_HORIZON_SECONDS, RADROOTS_BLOSSOM_AUTHORIZATION_EVENT_KIND,
    ServerDomain, ServerScopeRequirement, ValidatedAuthorizationClaim,
};
pub use descriptor::{ApprovedDescriptor, BlobDescriptor, ByteCommitment, ByteVerifiedDescriptor};
pub use error::Error;
pub use hash::{FileExtension, HashPath, Sha256};
pub use media_type::MediaType;
#[cfg(feature = "raster-decode")]
pub use publication_readiness::verify_publication_readiness;
pub use publication_readiness::{
    AuthoredRasterDimensions, Bud01GetCollector, Bud01GetObservation, Bud01HeadObservation,
    Bud02UploadObservation, Bud02UploadStatus, PublicationReadinessEvidence,
    PublicationReadinessEvidenceDigest, RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES,
    RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES,
    RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION,
    RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_PIXELS,
    RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_MAX_BYTES,
    RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_SCHEMA_VERSION,
    RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION,
    RADROOTS_BLOSSOM_PUBLICATION_READINESS_URL_MAX_BYTES, RasterDimensions, RasterFormat,
};
pub use url::{ApprovedBlobUrl, BlobUrl};

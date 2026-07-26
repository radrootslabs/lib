#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod authorization;
pub mod descriptor;
pub mod error;
pub mod hash;
pub mod publication_readiness;
pub mod url;

pub use authorization::{
    RADROOTS_BLOSSOM_AUTH_CONTENT_MAX_BYTES, RADROOTS_BLOSSOM_AUTH_MAX_CREATED_AGE_SECONDS,
    RADROOTS_BLOSSOM_AUTH_MAX_HORIZON_SECONDS, RADROOTS_BLOSSOM_AUTHORIZATION_EVENT_KIND,
    RadrootsBlossomAuthoredUploadClaim, RadrootsBlossomAuthorizationAction,
    RadrootsBlossomAuthorizationContent, RadrootsBlossomAuthorizationTarget,
    RadrootsBlossomAuthorizationValidation, RadrootsBlossomAuthorizationWireParts,
    RadrootsBlossomParsedAuthorizationClaim, RadrootsBlossomServerDomain,
    RadrootsBlossomServerScopeRequirement, RadrootsBlossomValidatedAuthorizationClaim,
};
pub use descriptor::{
    RadrootsBlossomApprovedDescriptor, RadrootsBlossomBlobDescriptor,
    RadrootsBlossomByteCommitment, RadrootsBlossomByteVerifiedDescriptor, RadrootsBlossomMediaType,
};
pub use error::RadrootsBlossomError;
pub use hash::{RadrootsBlossomFileExtension, RadrootsBlossomHashPath, RadrootsBlossomSha256};
#[cfg(feature = "raster-decode")]
pub use publication_readiness::verify_publication_readiness;
pub use publication_readiness::{
    RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES,
    RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES,
    RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION,
    RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_PIXELS,
    RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_MAX_BYTES,
    RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_SCHEMA_VERSION,
    RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION,
    RADROOTS_BLOSSOM_PUBLICATION_READINESS_URL_MAX_BYTES, RadrootsBlossomAuthoredRasterDimensions,
    RadrootsBlossomBud01GetCollector, RadrootsBlossomBud01GetObservation,
    RadrootsBlossomBud01HeadObservation, RadrootsBlossomBud02UploadObservation,
    RadrootsBlossomBud02UploadStatus, RadrootsBlossomPublicationReadinessEvidence,
    RadrootsBlossomPublicationReadinessEvidenceDigest, RadrootsBlossomRasterDimensions,
    RadrootsBlossomRasterFormat,
};
pub use url::{RadrootsBlossomApprovedBlobUrl, RadrootsBlossomBlobUrl};

pub const RADROOTS_BLOSSOM_PROTOCOL_COMMIT: &str = "b5bd2801d1763aa635fc8fea7a76597e0eb18990";

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod authorization;
pub mod descriptor;
pub mod error;
pub mod hash;
pub mod url;

pub use authorization::{
    RADROOTS_BLOSSOM_AUTH_MAX_CREATED_AGE_SECONDS, RADROOTS_BLOSSOM_AUTH_MAX_HORIZON_SECONDS,
    RADROOTS_BLOSSOM_AUTHORIZATION_EVENT_KIND, RadrootsBlossomAuthoredUploadClaim,
    RadrootsBlossomAuthorizationAction, RadrootsBlossomAuthorizationContent,
    RadrootsBlossomAuthorizationTarget, RadrootsBlossomAuthorizationValidation,
    RadrootsBlossomAuthorizationWireParts, RadrootsBlossomParsedAuthorizationClaim,
    RadrootsBlossomServerDomain, RadrootsBlossomServerScopeRequirement,
    RadrootsBlossomValidatedAuthorizationClaim,
};
pub use descriptor::{
    RadrootsBlossomApprovedDescriptor, RadrootsBlossomBlobDescriptor,
    RadrootsBlossomByteCommitment, RadrootsBlossomByteVerifiedDescriptor, RadrootsBlossomMediaType,
};
pub use error::RadrootsBlossomError;
pub use hash::{RadrootsBlossomFileExtension, RadrootsBlossomHashPath, RadrootsBlossomSha256};
pub use url::{RadrootsBlossomApprovedBlobUrl, RadrootsBlossomBlobUrl};

pub const RADROOTS_BLOSSOM_PROTOCOL_COMMIT: &str = "b5bd2801d1763aa635fc8fea7a76597e0eb18990";

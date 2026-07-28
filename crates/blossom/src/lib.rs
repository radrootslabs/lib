#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod authorization;
pub mod descriptor;
mod error;
pub mod hash;
pub mod media_type;
pub mod url;

pub use authorization::AuthorizationClaim;
pub use descriptor::BlobDescriptor;
pub use descriptor::ByteVerifiedDescriptor;
pub use error::Error;
pub use hash::Sha256;
pub use media_type::MediaType;
pub use url::BlobUrl;

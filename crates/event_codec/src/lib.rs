#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]
#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod d_tag;
pub mod error;
pub mod event_ref;
mod field_helpers;
pub mod job;
#[cfg(feature = "knowledge")]
pub mod knowledge;
#[cfg(feature = "contract-manifest")]
pub mod manifest;
pub mod parsed;
pub mod profile;
pub mod report;
pub mod repost;
mod social_helpers;
pub mod tag_builders;
#[cfg(feature = "knowledge")]
pub mod verification;
pub mod wire;

pub mod app_data;
pub mod article;
pub mod calendar;
pub mod comment;
pub mod coop;
pub mod document;
pub mod farm;
pub mod farm_crdt;
pub mod farm_file;
pub mod farm_workspace;
pub mod file_metadata;
pub mod follow;
pub mod geochat;
pub mod gift_wrap;
pub mod group;
pub mod http_auth;
pub mod message;
pub mod message_file;
pub mod plot;
pub mod post;
pub mod reaction;
pub mod relay_auth;
pub mod resource_area;
pub mod resource_cap;
pub mod seal;

pub mod list;
pub mod list_set;
pub mod listing;
pub mod order;
pub mod trade;

#[cfg(test)]
mod test_fixtures;

#[cfg(feature = "serde_json")]
pub mod relay_document;

#[cfg(feature = "contract-manifest")]
pub use manifest::{
    RADROOTS_KNOWLEDGE_CONTRACT_MANIFEST_SCHEMA_VERSION, contract_manifest_json,
    contract_manifest_sha256, knowledge_contract_manifest,
};
pub use tag_builders::RadrootsEventTagBuilder;
#[cfg(feature = "knowledge")]
pub use verification::{
    RadrootsContractValidatedEvent, RadrootsDecodeError, RadrootsDecodedEvent,
    RadrootsIdVerifiedEvent, RadrootsNip01VerificationError, RadrootsSignatureVerifiedEvent,
    decode_validated_event, validate_event_contract, verify_and_decode_radroots_event,
    verify_event_id, verify_event_signature,
};

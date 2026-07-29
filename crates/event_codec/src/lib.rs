#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]
#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod canonical;
mod codec;
pub mod d_tag;
pub mod decode;
pub mod encode;
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
pub mod reply;
pub mod report;
pub mod repost;
mod social_helpers;
pub mod tag_builders;
pub mod verification;
pub mod verify;
pub mod wire;

pub use codec::Codec;
pub use decode::DecodeError;
pub use encode::EncodeError;
pub use verify::VerificationError;

#[cfg(feature = "serde_json")]
pub mod admission;
pub mod app_data;
pub mod article;
pub mod calendar;
pub mod comment;
pub mod coop;
pub mod deletion;
pub mod document;
pub mod farm;
pub mod farm_crdt;
pub mod farm_file;
pub mod farm_workspace;
pub mod file_metadata;
pub mod follow;
pub mod food_availability;
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
pub mod operational_listing;
pub mod order;
pub mod trade;

#[cfg(test)]
mod test_fixtures;

#[cfg(feature = "serde_json")]
pub mod relay_document;

#[cfg(feature = "contract-manifest")]
pub use manifest::registry_v7::{
    RADROOTS_EVENT_CONTRACT_REGISTRY_V7_EVENT_COUNT,
    RADROOTS_EVENT_CONTRACT_REGISTRY_V7_INVENTORY_SCHEMA_VERSION,
    RADROOTS_EVENT_CONTRACT_REGISTRY_V7_KIND_COUNT, RADROOTS_EVENT_CONTRACT_REGISTRY_V7_VERSION,
    RadrootsEventContractRegistryV7Inventory, event_contract_registry_v7_inventory,
    event_contract_registry_v7_inventory_json, event_contract_registry_v7_inventory_sha256,
    parse_event_contract_registry_v7_inventory_json,
};
#[cfg(feature = "contract-manifest")]
pub use manifest::{
    RADROOTS_KNOWLEDGE_CONTRACT_MANIFEST_SCHEMA_VERSION, contract_manifest_json,
    contract_manifest_sha256, knowledge_contract_manifest,
};
pub use tag_builders::RadrootsEventTagBuilder;
pub use verification::{
    RadrootsContractValidatedEvent, RadrootsIdVerifiedEvent, RadrootsNip01VerificationError,
    RadrootsSignatureVerifiedEvent, validate_event_contract, validate_event_contract_registry_v7,
    verify_event_id, verify_event_id_v1, verify_event_signature, verify_event_signature_v1,
    verify_nip01_event, verify_nip01_event_v1,
};
#[cfg(feature = "knowledge")]
pub use verification::{
    RadrootsDecodeError, RadrootsDecodedEvent, decode_validated_event,
    verify_and_decode_radroots_event,
};

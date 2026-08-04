#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]
#![deny(rustdoc::broken_intra_doc_links)]
#![doc = include_str!("../README.md")]
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(test)]
mod test_fixtures;

// TEMPORARY COMPATIBILITY QUARANTINE (publish = false): binding generation,
// operational-listing consumers, and SP1 validation receipts migrate in
// Steps 238 and 261-262. Every declaration below is removed in Step 313.
#[cfg(feature = "dto-bindgen")]
#[doc(hidden)]
pub mod dto;
pub mod evidence;
pub mod model;
#[doc(hidden)]
pub mod operational_listing;
pub mod reducer;
#[path = "reducer_impl.rs"]
mod trade_contract_v1;
pub mod validation;
#[cfg(feature = "json")]
#[doc(hidden)]
pub mod validation_receipt;
pub mod workflow;

pub use model::RadrootsTradeProjectionV1 as Projection;
pub use reducer::{
    RadrootsTradeReducerIssueV1 as ReducerIssue, RadrootsTradeReductionInputV1 as ReductionInput,
};
pub use validation::ValidationError;
pub use workflow::{Error, WorkflowPlan};

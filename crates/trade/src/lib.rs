#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(feature = "dto-bindgen")]
pub mod dto;
pub mod evidence;
pub mod identity;
pub mod model;
pub mod operational_listing;
pub mod prelude;
pub mod reducer;
#[path = "reducer_impl.rs"]
mod trade_contract_v1;
pub mod validation;
#[cfg(feature = "serde_json")]
pub mod validation_receipt;
pub mod workflow;

pub use model::RadrootsTradeProjectionV1 as Projection;
pub use reducer::{
    RadrootsTradeReducerIssueV1 as ReducerIssue, RadrootsTradeReductionInputV1 as ReductionInput,
};
pub use workflow::{Error, WorkflowPlan};

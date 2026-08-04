#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]
#![deny(rustdoc::broken_intra_doc_links)]
#![doc = include_str!("../README.md")]
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(test)]
mod test_fixtures;

pub mod evidence;
pub mod model;
pub mod reducer;
#[path = "reducer_impl.rs"]
mod trade_contract_v1;
pub mod validation;
pub mod workflow;

pub use model::RadrootsTradeProjectionV1 as Projection;
pub use reducer::{
    RadrootsTradeReducerIssueV1 as ReducerIssue, RadrootsTradeReductionInputV1 as ReductionInput,
};
pub use validation::ValidationError;
pub use workflow::{Error, WorkflowPlan};

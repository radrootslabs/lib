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
#[path = "workflow.rs"]
mod trade_contract_v1;
pub mod validation;
#[cfg(feature = "serde_json")]
pub mod validation_receipt;

/// Side-effect-free trade workflow contracts.
///
/// The versioned reexports are a temporary migration surface for the existing
/// serialized contract. New reducer consumers should use [`crate::model`],
/// [`crate::evidence`], and [`crate::reducer`].
pub mod workflow {
    pub use crate::trade_contract_v1::{
        RADROOTS_TRADE_REDUCER_CONTRACT_ID, RADROOTS_TRADE_REDUCER_VERSION,
        RadrootsTradeAgreementClaimV1, RadrootsTradeAgreementStateV1,
        RadrootsTradeAttestationRecordV1, RadrootsTradeAttestationResultV1,
        RadrootsTradeAttestationStateV1, RadrootsTradeConflictStateV1,
        RadrootsTradeEvidenceStateV1, RadrootsTradeFulfillmentStateV1,
        RadrootsTradeMutationRecordV1, RadrootsTradeNegotiationStateV1,
        RadrootsTradePaymentStateV1, RadrootsTradePrivateTermsEvidenceV1,
        RadrootsTradePrivateTermsStateV1, RadrootsTradeProjectionV1, RadrootsTradeReducerIssueV1,
        RadrootsTradeReductionInputV1, reduce_trade_records,
    };
}

pub use model::RadrootsTradeProjectionV1 as Projection;
pub use reducer::{
    RadrootsTradeReducerIssueV1 as ReducerIssue, RadrootsTradeReductionInputV1 as ReductionInput,
};

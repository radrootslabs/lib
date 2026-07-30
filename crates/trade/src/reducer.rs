//! Deterministic trade reduction and conflict analysis.
//!
//! Reduction consumes caller-supplied evidence only. It performs no record
//! lookup, authorization, persistence, signing, or delivery.

pub use crate::trade_contract_v1::{
    RADROOTS_TRADE_REDUCER_CONTRACT_ID, RADROOTS_TRADE_REDUCER_VERSION,
    RadrootsTradeReducerIssueV1, RadrootsTradeReductionInputV1, reduce_trade_records,
};

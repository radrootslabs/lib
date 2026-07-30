//! Evidence supplied explicitly to deterministic trade reduction.
//!
//! These values describe observed mutation, private-term, and attestation
//! records. They perform no retrieval, signature verification, or decryption.

pub use crate::trade_contract_v1::{
    RadrootsTradeAttestationRecordV1, RadrootsTradeAttestationResultV1,
    RadrootsTradeEvidenceStateV1, RadrootsTradeMutationRecordV1,
    RadrootsTradePrivateTermsEvidenceV1,
};

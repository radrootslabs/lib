use crate::{RadrootsTransportTargetReceipt, TargetSet};
use alloc::string::String;
use alloc::vec::Vec;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportFetchRequest {
    pub request_id: String,
    pub target_set: TargetSet,
}

impl RadrootsTransportFetchRequest {
    pub fn new(request_id: impl Into<String>, target_set: TargetSet) -> Self {
        Self {
            request_id: request_id.into(),
            target_set,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportFetchReceipt {
    pub request_id: String,
    pub target_receipts: Vec<RadrootsTransportTargetReceipt>,
    pub fetched_count: usize,
}

impl RadrootsTransportFetchReceipt {
    pub fn new(
        request_id: impl Into<String>,
        target_receipts: Vec<RadrootsTransportTargetReceipt>,
        fetched_count: usize,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            target_receipts,
            fetched_count,
        }
    }
}

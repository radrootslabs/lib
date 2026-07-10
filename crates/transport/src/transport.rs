use crate::{
    RadrootsTransportDeliveryReceipt, RadrootsTransportDeliveryRequest, RadrootsTransportError,
    RadrootsTransportKind, RadrootsTransportStatus, RadrootsTransportTargetReceipt,
    RadrootsTransportTargetSet,
};
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::future::Future;
use core::pin::Pin;

pub type RadrootsTransportFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, RadrootsTransportError>> + Send + 'a>>;

pub trait RadrootsTransport: Send + Sync {
    fn transport_kind(&self) -> RadrootsTransportKind;

    fn status<'a>(&'a self) -> RadrootsTransportFuture<'a, RadrootsTransportStatus>;

    fn deliver<'a>(
        &'a self,
        request: RadrootsTransportDeliveryRequest,
    ) -> RadrootsTransportFuture<'a, RadrootsTransportDeliveryReceipt>;

    fn fetch<'a>(
        &'a self,
        request: RadrootsTransportFetchRequest,
    ) -> RadrootsTransportFuture<'a, RadrootsTransportFetchReceipt>;
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportFetchRequest {
    pub request_id: String,
    pub target_set: RadrootsTransportTargetSet,
}

impl RadrootsTransportFetchRequest {
    pub fn new(request_id: impl Into<String>, target_set: RadrootsTransportTargetSet) -> Self {
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

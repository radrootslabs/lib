//! Inbound event source SPI and page models.

use crate::{
    Error, RadrootsTransportFetchReceipt, RadrootsTransportFetchRequest, RadrootsTransportStatus,
};
use alloc::boxed::Box;
use core::{future::Future, pin::Pin};

/// Heap-backed future returned by transport SPIs.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Current source status.
///
/// This compatibility alias is replaced by the dedicated source status model
/// in the ordered capability/status checkpoint.
pub type SourceStatus = RadrootsTransportStatus;

/// Bounded source request.
///
/// The dedicated bounded-page checkpoint replaces this compatibility alias
/// with the final request model.
pub type FetchRequest = RadrootsTransportFetchRequest;

/// One bounded page returned by an event source.
pub type FetchPage = RadrootsTransportFetchReceipt;

/// Host SPI for inbound event retrieval.
///
/// This trait supports external implementations and is dyn-compatible. Its
/// futures are `Send`; implementations must not borrow request data after a
/// future completes. `status` observes source state and does not initiate a
/// fetch. `fetch` performs at most the work bounded by its request and owns no
/// hidden retry loop.
///
/// Dropping a returned future requests cancellation. If it is dropped before
/// a remote request is published, the implementation must leave no remote
/// operation behind. Once publication may have occurred, cancellation cannot
/// claim rollback; a later observation may report the remote outcome. An
/// explicit request deadline bounds work independently of future cancellation.
pub trait EventSource: Send + Sync {
    /// Returns the source's current runtime status.
    fn status(&self) -> BoxFuture<'_, Result<SourceStatus, Error>>;

    /// Fetches one bounded page of transport-neutral events.
    fn fetch(&self, request: FetchRequest) -> BoxFuture<'_, Result<FetchPage, Error>>;
}

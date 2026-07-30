//! Outbound event delivery SPI and request models.

use crate::{
    Error, RadrootsTransportDeliveryReceipt, RadrootsTransportDeliveryRequest, source::BoxFuture,
};

pub use crate::status::SinkStatus;

/// Bounded delivery request.
///
/// The dedicated delivery checkpoint replaces this compatibility alias with
/// the final request model.
pub type DeliveryRequest = RadrootsTransportDeliveryRequest;

/// Per-target delivery result.
pub type DeliveryReceipt = RadrootsTransportDeliveryReceipt;

/// Host SPI for outbound event delivery.
///
/// This trait supports external implementations and is dyn-compatible. Its
/// futures are `Send`; implementations must not borrow request data after a
/// future completes. `status` observes sink state and does not initiate
/// delivery. `deliver` performs only the attempts authorized by its request,
/// returns partial success per target, and owns no hidden retry loop.
///
/// Dropping a returned future requests cancellation. If it is dropped before
/// a remote request is published, the implementation must leave no remote
/// operation behind. Once publication may have occurred, cancellation cannot
/// claim rollback; a later observation may report the remote outcome. An
/// explicit request deadline bounds work independently of future cancellation.
pub trait EventSink: Send + Sync {
    /// Returns the sink's current runtime status.
    fn status(&self) -> BoxFuture<'_, Result<SinkStatus, Error>>;

    /// Delivers an event according to the request's bounded target policy.
    fn deliver(&self, request: DeliveryRequest) -> BoxFuture<'_, Result<DeliveryReceipt, Error>>;
}

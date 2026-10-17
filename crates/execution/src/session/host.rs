use core::{future::Future, pin::Pin};
use std::boxed::Box;

#[cfg(target_arch = "wasm32")]
pub(crate) use std::rc::Rc as Shared;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use std::sync::Arc as Shared;

/// An owned host future. Browser futures remain on their originating thread.
#[cfg(not(target_arch = "wasm32"))]
pub type HostFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
/// An owned host future. Browser futures remain on their originating thread.
#[cfg(target_arch = "wasm32")]
pub type HostFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// Native hosts are thread safe; browser hosts may retain local callbacks.
#[cfg(not(target_arch = "wasm32"))]
pub trait HostBound: Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + Sync + ?Sized> HostBound for T {}

/// Native hosts are thread safe; browser hosts may retain local callbacks.
#[cfg(target_arch = "wasm32")]
pub trait HostBound {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> HostBound for T {}

/// Resources exclusively owned by one session, without a library-owned runtime.
///
/// Hooks must retain progress across cancellation: repeating an unfinished hook
/// resumes it, including a result already obtained before its waiter was lost.
/// Successful hooks are not repeated. A failed hook must leave its resources
/// available for retry. No hook may report completion while its work is detached.
/// Concrete host qualification must exercise these requirements independently.
pub trait SessionHost: HostBound {
    /// Starts the configured bounded workers. Repeat calls resume partial start.
    /// No command admission is enabled until this hook succeeds.
    fn start(&self) -> HostFuture<'_, crate::Result<()>>;

    /// Stops new background work and quiesces workers. Retain unresolved signing
    /// and obtained results in their journal before releasing their work permits.
    /// Existing permitted work must remain able to reach its safe boundary.
    fn quiesce(&self) -> HostFuture<'_, crate::Result<()>>;

    /// Disconnects every owned transport and its authentication state.
    fn disconnect(&self) -> HostFuture<'_, crate::Result<()>>;

    /// Disposes retained callbacks and subscriptions on their owning thread.
    fn dispose_callbacks(&self) -> HostFuture<'_, crate::Result<()>>;

    /// Closes storage after workers, admitted work, transports and callbacks.
    /// Completion must include any required checkpoint and authority release.
    fn close_storage(&self) -> HostFuture<'_, crate::Result<()>>;
}

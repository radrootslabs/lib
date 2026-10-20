//! Target-aware host bounds and shared handles for transport composition.

/// Concrete WebSocket I/O supplied by native or browser hosts.
#[cfg(feature = "std")]
pub mod websocket;

/// Native host implementations are thread safe; browser hosts may be local.
#[cfg(not(target_arch = "wasm32"))]
pub trait HostBound: Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + Sync + ?Sized> HostBound for T {}
/// Native host implementations are thread safe; browser hosts may be local.
#[cfg(target_arch = "wasm32")]
pub trait HostBound {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> HostBound for T {}

/// Native subscription ownership may move between threads.
#[cfg(not(target_arch = "wasm32"))]
pub trait MoveBound: Send {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + ?Sized> MoveBound for T {}
/// Browser subscription ownership remains local to its host.
#[cfg(target_arch = "wasm32")]
pub trait MoveBound {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> MoveBound for T {}

/// Shared host ownership. Browser handles never require an unsafe Send bridge.
#[cfg(not(target_arch = "wasm32"))]
pub type Shared<T> = alloc::sync::Arc<T>;
/// Shared host ownership. Browser handles never require an unsafe Send bridge.
#[cfg(target_arch = "wasm32")]
pub type Shared<T> = alloc::rc::Rc<T>;

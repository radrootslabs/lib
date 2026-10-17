//! Target-aware ownership of signer and progress callback state.

/// Native signer hosts are thread safe; browser hosts may retain local state.
#[cfg(not(target_arch = "wasm32"))]
pub trait HostBound: Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + Sync + ?Sized> HostBound for T {}
/// Native signer hosts are thread safe; browser hosts may retain local state.
#[cfg(target_arch = "wasm32")]
pub trait HostBound {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> HostBound for T {}

/// Shared signer/callback ownership on its qualified host.
#[cfg(all(not(target_arch = "wasm32"), not(feature = "std")))]
pub type Shared<T> = alloc::sync::Arc<T>;
/// Shared signer/callback ownership on its qualified host.
#[cfg(all(not(target_arch = "wasm32"), feature = "std"))]
pub type Shared<T> = std::sync::Arc<T>;
/// Shared signer/callback ownership on its qualified host.
#[cfg(all(target_arch = "wasm32", not(feature = "std")))]
pub type Shared<T> = alloc::rc::Rc<T>;
/// Shared signer/callback ownership on its qualified host.
#[cfg(all(target_arch = "wasm32", feature = "std"))]
pub type Shared<T> = std::rc::Rc<T>;

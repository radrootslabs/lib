#![forbid(unsafe_code)]

mod snapshot;
#[cfg(target_arch = "wasm32")]
mod utils;
#[cfg(target_arch = "wasm32")]
mod wasm_impl;
pub use snapshot::*;
#[cfg(target_arch = "wasm32")]
pub use wasm_impl::*;

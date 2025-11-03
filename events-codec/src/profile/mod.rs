#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod error;

#[cfg(feature = "nostr")]
pub mod encode;

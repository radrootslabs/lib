#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

pub use radroots_events_codec::wire::WireEventParts;

pub type NostrTags = Vec<Vec<String>>;

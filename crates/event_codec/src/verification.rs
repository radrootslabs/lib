#![forbid(unsafe_code)]

#[doc(hidden)]
pub mod v1;

pub use v1::*;

#[cfg(feature = "knowledge")]
pub use crate::knowledge::verification::{
    RadrootsDecodeError, RadrootsDecodedEvent, decode_validated_event,
    verify_and_decode_radroots_event,
};

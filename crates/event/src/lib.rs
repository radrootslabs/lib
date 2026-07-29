#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]
#![forbid(unsafe_code)]
#![deny(rustdoc::broken_intra_doc_links)]

//! Canonical Radroots event-domain models.
//!
//! Public behavior is grouped under the approved singular domain modules.
//! The crate root intentionally exposes only [`Event`], [`EventDraft`],
//! [`SignedEvent`], [`VerifiedEvent`], [`EventId`], [`EventKind`], [`EventTag`],
//! and [`Error`].

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(test)]
/// Returns deterministic 64-character fixtures that are also valid secp256k1
/// x-only public keys; labels without a curve point are remapped.
pub(crate) fn test_valid_hex_64(character: char) -> String {
    if matches!(character, 'b' | 'B') {
        let value = "2f8bde4d1a07209355b4a7250a5c5128e88b84bddc619ab7cba8d569b240efe4";
        return if character.is_ascii_uppercase() {
            value.to_ascii_uppercase()
        } else {
            value.to_owned()
        };
    }
    let character = match character {
        '0' => '7',
        '1' => '8',
        '5' => 'd',
        '6' => 'e',
        '9' => 'a',
        'c' | 'C' => '3',
        'f' | 'F' => '4',
        other => other,
    };
    core::iter::repeat_n(character, 64).collect()
}

pub mod admission;
pub mod calendar;
pub mod contract;
pub mod draft;
#[cfg(feature = "dto-bindgen")]
mod dto;
pub mod envelope;
pub mod farm;
pub mod food;
pub mod id;
#[cfg(feature = "knowledge")]
pub mod knowledge;
pub mod listing;
pub mod media;
pub mod post;
pub mod profile;
pub mod social;
pub mod tag;
pub mod trade;
mod verification;
pub mod wire;

pub use draft::{RadrootsEventDraft as EventDraft, RadrootsSignedEvent as SignedEvent};
pub use envelope::{
    RadrootsEventEnvelope as Event, RadrootsEventKind as EventKind, RadrootsEventTag as EventTag,
};
pub use id::EventId;
pub use verification::{Error, SignatureVerifiedEvent as VerifiedEvent};

//! Portable conversion between Radroots events and Nostr protocol events.
//!
//! Upstream protocol values remain confined to this explicit adapter boundary;
//! canonical Radroots event and coordinate values cross it through validated,
//! deterministic conversions.

use alloc::string::ToString;

use radroots_event::id::Nip01Coordinate;

use crate::Error;

/// Upstream Nostr coordinate used only at the explicit protocol boundary.
pub type Coordinate = nostr::nips::nip01::Coordinate;
/// Upstream Nostr event used only at the explicit protocol boundary.
pub type Event = nostr::Event;
/// Upstream Nostr event identifier used only at the explicit protocol boundary.
pub type EventId = nostr::EventId;
/// Upstream Nostr event kind used only at the explicit protocol boundary.
pub type Kind = nostr::Kind;
/// Upstream Nostr metadata used only at the explicit protocol boundary.
pub type Metadata = nostr::Metadata;
/// Upstream Nostr timestamp used only at the explicit protocol boundary.
pub type Timestamp = nostr::Timestamp;

#[cfg(feature = "events")]
pub use crate::event_convert::{
    nostr_event_from_radroots, radroots_event_from_nostr, radroots_event_ptr_from_nostr,
};

/// Converts a canonical Radroots NIP-01 coordinate to its Nostr value.
pub fn coordinate_to_nostr(coordinate: &Nip01Coordinate) -> Result<Coordinate, Error> {
    Coordinate::from_kpi_format(coordinate.as_str()).map_err(|_| Error::CoordinateConversion)
}

/// Converts a Nostr coordinate to the canonical Radroots NIP-01 value.
pub fn coordinate_from_nostr(coordinate: &Coordinate) -> Result<Nip01Coordinate, Error> {
    Nip01Coordinate::parse(coordinate.to_string()).map_err(|_| Error::CoordinateConversion)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBLIC_KEY: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";

    #[test]
    fn coordinate_round_trip_is_canonical_and_rejects_invalid_kinds() {
        let native = Nip01Coordinate::parse(format!("30402:{PUBLIC_KEY}:listing-1"))
            .expect("native coordinate");
        let nostr = coordinate_to_nostr(&native).expect("Nostr coordinate");

        assert_eq!(nostr.to_string(), native.as_str());
        assert_eq!(
            coordinate_from_nostr(&nostr).expect("native coordinate"),
            native
        );

        let invalid = Coordinate::new(
            Kind::TextNote,
            nostr::PublicKey::from_hex(PUBLIC_KEY).expect("public key"),
        );
        assert!(matches!(
            coordinate_from_nostr(&invalid),
            Err(Error::CoordinateConversion)
        ));
    }
}

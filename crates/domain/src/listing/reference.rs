use super::ListingId;
use crate::{EnvelopeEventId, PublicKey};
use core::fmt;

/// Exact public listing coordinate, without an authenticated observation claim.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ListingCoordinate {
    author: PublicKey,
    d: ListingId,
}
impl ListingCoordinate {
    /// Captures validated identity parts; the kind is always NIP-99 kind30402.
    pub const fn new(author: PublicKey, d: ListingId) -> Self {
        Self { author, d }
    }
    /// Returns the fixed listing kind, never a caller-supplied alternative.
    pub const fn kind(self) -> u16 {
        30402
    }
    /// Returns the participant named by the coordinate, without author proof.
    pub const fn author(self) -> PublicKey {
        self.author
    }
    /// Returns the exact stable 128-bit discriminator.
    pub const fn d(self) -> ListingId {
        self.d
    }
}
impl fmt::Debug for ListingCoordinate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ListingCoordinate(<redacted>)")
    }
}
#[cfg(feature = "serde")]
impl serde::Serialize for ListingCoordinate {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut out = serializer.serialize_struct("ListingCoordinate", 3)?;
        out.serialize_field("kind", &self.kind())?;
        out.serialize_field("author", &self.author)?;
        out.serialize_field("d", &self.d)?;
        out.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ListingCoordinate {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            kind: u16,
            #[serde(deserialize_with = "canonical_author")]
            author: PublicKey,
            d: ListingId,
        }
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Wire;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a closed NIP-99 listing coordinate object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<Wire, A::Error> {
                <Wire as serde::Deserialize>::deserialize(
                    serde::de::value::MapAccessDeserializer::new(map),
                )
            }
        }
        let wire = deserializer.deserialize_map(Visitor)?;
        if wire.kind != 30402 {
            return Err(serde::de::Error::custom(
                "unsupported listing coordinate kind",
            ));
        }
        Ok(Self::new(wire.author, wire.d))
    }
}

#[cfg(feature = "serde")]
fn canonical_author<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<PublicKey, D::Error> {
    struct Author;
    impl serde::de::Visitor<'_> for Author {
        type Value = PublicKey;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a canonical lowercase public key")
        }
        fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<PublicKey, E> {
            let key = PublicKey::from_hex(value).map_err(E::custom)?;
            if key.to_hex() != value {
                return Err(E::custom("noncanonical listing author"));
            }
            Ok(key)
        }
    }
    deserializer.deserialize_str(Author)
}

/// Exact revision identity and coordinate; bytes alone authenticate no source.
#[derive(Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ListingRevision {
    coordinate: ListingCoordinate,
    event_id: EnvelopeEventId,
}
impl ListingRevision {
    /// Captures structural data; source authentication/admission remains separate.
    pub const fn new(coordinate: ListingCoordinate, event_id: EnvelopeEventId) -> Self {
        Self {
            coordinate,
            event_id,
        }
    }
    /// Returns the complete stable coordinate.
    pub const fn coordinate(self) -> ListingCoordinate {
        self.coordinate
    }
    /// Returns the exact signed-artifact identity claimed as the source.
    pub const fn event_id(self) -> EnvelopeEventId {
        self.event_id
    }
}
impl fmt::Debug for ListingRevision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ListingRevision(<redacted>)")
    }
}

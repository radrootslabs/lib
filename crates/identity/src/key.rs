//! Canonical public-key and identity identifier value types.
//!
//! [`PublicKey`] validates a 32-byte x-only secp256k1 public key. [`IdentityId`]
//! is a distinct Rust type derived from the same canonical bytes. Both values
//! parse exact-width hexadecimal text, preserve fixed-width binary form, and
//! emit lowercase hexadecimal text without exposing secret-key or Nostr
//! bech32 behavior.

use crate::Error;

pub(crate) const IDENTIFIER_BYTE_LENGTH: usize = 32;
pub(crate) const IDENTIFIER_HEX_LENGTH: usize = IDENTIFIER_BYTE_LENGTH * 2;

const HEX_ALPHABET: &[u8; 16] = b"0123456789abcdef";

pub(crate) struct EncodedHex([u8; IDENTIFIER_HEX_LENGTH]);

impl EncodedHex {
    pub(crate) fn new(bytes: &[u8; IDENTIFIER_BYTE_LENGTH]) -> Self {
        let mut encoded = [0; IDENTIFIER_HEX_LENGTH];
        for (index, byte) in bytes.iter().copied().enumerate() {
            encoded[index * 2] = HEX_ALPHABET[usize::from(byte >> 4)];
            encoded[index * 2 + 1] = HEX_ALPHABET[usize::from(byte & 0x0f)];
        }
        Self(encoded)
    }

    pub(crate) fn as_str(&self) -> &str {
        core::str::from_utf8(&self.0).expect("the hexadecimal alphabet is valid UTF-8")
    }
}

fn decode_nibble(byte: u8, index: usize) -> Result<u8, Error> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(Error::InvalidHexCharacter { index }),
    }
}

pub(crate) fn parse_hex(value: &str) -> Result<[u8; IDENTIFIER_BYTE_LENGTH], Error> {
    let encoded = value.as_bytes();
    if encoded.len() != IDENTIFIER_HEX_LENGTH {
        return Err(Error::InvalidHexLength {
            expected: IDENTIFIER_HEX_LENGTH,
            actual: encoded.len(),
        });
    }

    let mut bytes = [0; IDENTIFIER_BYTE_LENGTH];
    for (index, output) in bytes.iter_mut().enumerate() {
        let high_index = index * 2;
        let high = decode_nibble(encoded[high_index], high_index)?;
        let low = decode_nibble(encoded[high_index + 1], high_index + 1)?;
        *output = (high << 4) | low;
    }
    Ok(bytes)
}

pub(crate) fn validate_public_key_bytes(bytes: &[u8; IDENTIFIER_BYTE_LENGTH]) -> Result<(), Error> {
    let mut compressed = [0; IDENTIFIER_BYTE_LENGTH + 1];
    compressed[0] = 0x02;
    compressed[1..].copy_from_slice(bytes);
    k256::PublicKey::from_sec1_bytes(&compressed)
        .map(|_| ())
        .map_err(|_| Error::InvalidPublicKeyBytes)
}

macro_rules! define_identifier {
    ($(#[$meta:meta])* $visibility:vis struct $name:ident;) => {
        $(#[$meta])*
        #[repr(transparent)]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $visibility struct $name([u8; $crate::key::IDENTIFIER_BYTE_LENGTH]);

        impl $name {
            /// The canonical binary representation length.
            pub const BYTE_LENGTH: usize = $crate::key::IDENTIFIER_BYTE_LENGTH;

            /// The canonical hexadecimal representation length.
            pub const HEX_LENGTH: usize = $crate::key::IDENTIFIER_HEX_LENGTH;

            /// Constructs the value from its canonical fixed-width bytes.
            pub fn from_bytes(bytes: [u8; Self::BYTE_LENGTH]) -> Result<Self, $crate::Error> {
                $crate::key::validate_public_key_bytes(&bytes)?;
                Ok(Self::from_validated_bytes(bytes))
            }

            pub(crate) const fn from_validated_bytes(
                bytes: [u8; Self::BYTE_LENGTH],
            ) -> Self {
                Self(bytes)
            }

            /// Parses an exact-width byte slice.
            pub fn from_slice(bytes: &[u8]) -> Result<Self, $crate::Error> {
                let bytes: [u8; Self::BYTE_LENGTH] = bytes.try_into().map_err(|_| {
                    $crate::Error::InvalidByteLength {
                        expected: Self::BYTE_LENGTH,
                        actual: bytes.len(),
                    }
                })?;
                Self::from_bytes(bytes)
            }

            /// Parses a 64-character hexadecimal representation.
            pub fn from_hex(value: &str) -> Result<Self, $crate::Error> {
                Self::from_bytes($crate::key::parse_hex(value)?)
            }

            /// Borrows the canonical fixed-width bytes.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; Self::BYTE_LENGTH] {
                &self.0
            }

            /// Returns the canonical fixed-width bytes.
            #[must_use]
            pub const fn into_bytes(self) -> [u8; Self::BYTE_LENGTH] {
                self.0
            }

            /// Encodes the value as canonical lowercase hexadecimal text.
            #[must_use]
            pub fn to_hex(self) -> alloc::string::String {
                alloc::string::String::from(
                    $crate::key::EncodedHex::new(&self.0).as_str(),
                )
            }
        }

        impl core::fmt::Debug for $name {
            fn fmt(
                &self,
                formatter: &mut core::fmt::Formatter<'_>,
            ) -> core::fmt::Result {
                write!(formatter, "{}(\"{}\")", stringify!($name), self)
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(
                &self,
                formatter: &mut core::fmt::Formatter<'_>,
            ) -> core::fmt::Result {
                formatter.write_str($crate::key::EncodedHex::new(&self.0).as_str())
            }
        }

        impl core::str::FromStr for $name {
            type Err = $crate::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::from_hex(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = $crate::Error;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::from_hex(value)
            }
        }

        impl TryFrom<alloc::string::String> for $name {
            type Error = $crate::Error;

            fn try_from(value: alloc::string::String) -> Result<Self, Self::Error> {
                Self::from_hex(&value)
            }
        }

        impl TryFrom<&[u8]> for $name {
            type Error = $crate::Error;

            fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
                Self::from_slice(value)
            }
        }

        impl TryFrom<[u8; $crate::key::IDENTIFIER_BYTE_LENGTH]> for $name {
            type Error = $crate::Error;

            fn try_from(
                value: [u8; $crate::key::IDENTIFIER_BYTE_LENGTH],
            ) -> Result<Self, Self::Error> {
                Self::from_bytes(value)
            }
        }

        impl AsRef<[u8]> for $name {
            fn as_ref(&self) -> &[u8] {
                self.as_bytes()
            }
        }

        #[cfg(feature = "serde")]
        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str($crate::key::EncodedHex::new(&self.0).as_str())
            }
        }

        #[cfg(feature = "serde")]
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct IdentifierVisitor;

                impl serde::de::Visitor<'_> for IdentifierVisitor {
                    type Value = $name;

                    fn expecting(
                        &self,
                        formatter: &mut core::fmt::Formatter<'_>,
                    ) -> core::fmt::Result {
                        formatter.write_str(concat!(
                            "a 64-character hexadecimal ",
                            stringify!($name)
                        ))
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        $name::from_hex(value).map_err(E::custom)
                    }
                }

                deserializer.deserialize_str(IdentifierVisitor)
            }
        }
    };
}

pub(crate) use define_identifier;

define_identifier! {
    /// A canonical 32-byte public key.
    ///
    /// The key is an explicit byte value and intentionally does not dereference
    /// to text:
    ///
    /// ```compile_fail
    /// use radroots_identity::PublicKey;
    ///
    /// fn accepts_text(_: &str) {}
    /// let key = PublicKey::from_hex(
    ///     "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df",
    /// ).unwrap();
    /// accepts_text(&key);
    /// ```
    pub struct PublicKey;
}

define_identifier! {
    /// A canonical public identity identifier.
    pub struct IdentityId;
}

impl IdentityId {
    /// Derives the identity identifier from its canonical public key.
    #[must_use]
    pub const fn from_public_key(public_key: PublicKey) -> Self {
        Self::from_validated_bytes(public_key.into_bytes())
    }
}

impl From<PublicKey> for IdentityId {
    fn from(value: PublicKey) -> Self {
        Self::from_public_key(value)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "serde")]
    use alloc::format;
    use alloc::string::{String, ToString};
    use core::str::FromStr;

    use super::*;

    const ALICE: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
    const BOB: &str = "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";

    #[test]
    fn public_keys_validate_and_canonicalize_hex() {
        let uppercase = ALICE.to_ascii_uppercase();
        let public_key = PublicKey::from_hex(&uppercase).expect("valid fixture public key");

        assert_eq!(public_key.to_hex(), ALICE);
        assert_eq!(public_key.to_string(), ALICE);
        assert_eq!(PublicKey::from_str(ALICE).unwrap(), public_key);
        assert_eq!(PublicKey::try_from(ALICE).unwrap(), public_key);
    }

    #[test]
    fn public_keys_reject_invalid_encodings_and_curve_points() {
        assert!(matches!(
            PublicKey::from_hex("00"),
            Err(Error::InvalidHexLength {
                expected: PublicKey::HEX_LENGTH,
                actual: 2,
            })
        ));

        let mut invalid_hex = ALICE.as_bytes().to_vec();
        invalid_hex[17] = b'g';
        let invalid_hex = String::from_utf8(invalid_hex).expect("ASCII test input");
        assert!(matches!(
            PublicKey::from_hex(&invalid_hex),
            Err(Error::InvalidHexCharacter { index: 17 })
        ));
        assert!(matches!(
            PublicKey::from_bytes([0; PublicKey::BYTE_LENGTH]),
            Err(Error::InvalidPublicKeyBytes)
        ));
        assert!(matches!(
            PublicKey::from_slice(&[0; PublicKey::BYTE_LENGTH - 1]),
            Err(Error::InvalidByteLength {
                expected: PublicKey::BYTE_LENGTH,
                actual,
            }) if actual == PublicKey::BYTE_LENGTH - 1
        ));
    }

    #[test]
    fn canonical_bytes_round_trip_and_order() {
        let alice = PublicKey::from_hex(ALICE).expect("alice fixture");
        let bob = PublicKey::from_hex(BOB).expect("bob fixture");

        assert_eq!(PublicKey::from_bytes(alice.into_bytes()).unwrap(), alice);
        assert_eq!(PublicKey::try_from(alice.as_ref()).unwrap(), alice);
        assert!(alice < bob);
    }

    #[test]
    fn identity_ids_are_distinct_key_derived_values() {
        let public_key = PublicKey::from_hex(ALICE).expect("valid fixture public key");
        let identity_id = IdentityId::from(public_key);

        assert_eq!(identity_id.to_hex(), ALICE);
        assert_eq!(identity_id.as_bytes(), public_key.as_bytes());
        assert_eq!(IdentityId::from_hex(ALICE).unwrap(), identity_id);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn key_values_serde_as_validated_canonical_hex() {
        let public_key = PublicKey::from_hex(ALICE).expect("valid fixture public key");
        let encoded = serde_json::to_string(&public_key).expect("serialize public key");

        assert_eq!(encoded, format!("\"{ALICE}\""));
        assert_eq!(
            serde_json::from_str::<PublicKey>(&encoded).expect("deserialize public key"),
            public_key
        );
        assert!(serde_json::from_str::<IdentityId>("\"invalid\"").is_err());
    }
}

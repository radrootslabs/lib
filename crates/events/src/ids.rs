#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, string::ToString, vec::Vec};

#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

use core::{borrow::Borrow, fmt, str::FromStr};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsIdParseError {
    Empty,
    InvalidFormat,
    InvalidLength { expected: usize, actual: usize },
    InvalidCharacter,
    TooLong { max: usize, actual: usize },
}

impl fmt::Display for RadrootsIdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "identifier is empty"),
            Self::InvalidFormat => write!(f, "identifier has invalid format"),
            Self::InvalidLength { expected, actual } => {
                write!(
                    f,
                    "identifier length {actual} does not match required length {expected}"
                )
            }
            Self::InvalidCharacter => write!(f, "identifier contains an invalid character"),
            Self::TooLong { max, actual } => {
                write!(f, "identifier length {actual} exceeds maximum length {max}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsIdParseError {}

macro_rules! validated_string_id {
    ($name:ident, $validator:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsIdParseError> {
                $validator(value.as_ref()).map(Self)
            }

            #[inline]
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }

            #[inline]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl AsRef<str> for $name {
            #[inline]
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl Borrow<str> for $name {
            #[inline]
            fn borrow(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = RadrootsIdParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = RadrootsIdParseError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::parse(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = RadrootsIdParseError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::parse(value)
            }
        }

        #[cfg(feature = "serde")]
        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        #[cfg(feature = "serde")]
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

validated_string_id!(RadrootsPublicKey, validate_hex_64);
validated_string_id!(RadrootsEventId, validate_hex_64);
validated_string_id!(RadrootsEventSignature, validate_hex_128);
validated_string_id!(RadrootsDTag, validate_d_tag);
validated_string_id!(
    RadrootsAddressableCoordinate,
    validate_addressable_coordinate
);
validated_string_id!(RadrootsListingAddress, validate_addressable_coordinate);
validated_string_id!(RadrootsOrderId, validate_commercial_id);
validated_string_id!(RadrootsOrderRevisionId, validate_commercial_id);
validated_string_id!(RadrootsOrderQuoteId, validate_commercial_id);
validated_string_id!(RadrootsInventoryBinId, validate_commercial_id);
validated_string_id!(RadrootsEconomicsDigest, validate_economics_digest);
validated_string_id!(RadrootsEventPointer, validate_hex_64);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsNostrEventPointer {
    pub event_id: RadrootsEventId,
    pub relays: Vec<String>,
}

impl RadrootsNostrEventPointer {
    pub fn new<I, S>(event_id: RadrootsEventId, relays: I) -> Result<Self, RadrootsIdParseError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut canonical_relays = Vec::new();
        for relay in relays {
            let relay = relay.into();
            if relay.is_empty()
                || relay.trim() != relay
                || relay.chars().any(|character| character.is_control())
            {
                return Err(RadrootsIdParseError::InvalidCharacter);
            }
            canonical_relays.push(relay);
        }
        Ok(Self {
            event_id,
            relays: canonical_relays,
        })
    }
}

fn validate_hex_64(value: &str) -> Result<String, RadrootsIdParseError> {
    validate_hex(value, 64)
}

fn validate_hex_128(value: &str) -> Result<String, RadrootsIdParseError> {
    validate_hex(value, 128)
}

fn validate_hex(value: &str, expected_len: usize) -> Result<String, RadrootsIdParseError> {
    if value.len() != expected_len {
        return Err(RadrootsIdParseError::InvalidLength {
            expected: expected_len,
            actual: value.len(),
        });
    }

    let mut canonical = String::with_capacity(expected_len);
    for byte in value.bytes() {
        match byte {
            b'0'..=b'9' => canonical.push(byte as char),
            b'a'..=b'f' => canonical.push(byte as char),
            b'A'..=b'F' => canonical.push((byte + 32) as char),
            _ => return Err(RadrootsIdParseError::InvalidCharacter),
        }
    }
    Ok(canonical)
}

fn validate_d_tag(value: &str) -> Result<String, RadrootsIdParseError> {
    validate_visible_token(value, 512)
}

fn validate_commercial_id(value: &str) -> Result<String, RadrootsIdParseError> {
    validate_visible_token(value, 128)
}

fn validate_economics_digest(value: &str) -> Result<String, RadrootsIdParseError> {
    if let Some(hex) = value.strip_prefix("sha256:") {
        validate_hex(hex, 64)?;
        return Ok(value.to_string());
    }
    validate_visible_token(value, 128)
}

fn validate_addressable_coordinate(value: &str) -> Result<String, RadrootsIdParseError> {
    let (kind, remainder) = value
        .split_once(':')
        .ok_or(RadrootsIdParseError::InvalidFormat)?;
    let (pubkey, d_tag) = remainder
        .split_once(':')
        .ok_or(RadrootsIdParseError::InvalidFormat)?;
    kind.parse::<u32>()
        .map_err(|_| RadrootsIdParseError::InvalidFormat)?;
    validate_hex_64(pubkey)?;
    validate_d_tag(d_tag)?;
    Ok(value.to_string())
}

fn validate_visible_token(value: &str, max_len: usize) -> Result<String, RadrootsIdParseError> {
    if value.is_empty() {
        return Err(RadrootsIdParseError::Empty);
    }
    if value.len() > max_len {
        return Err(RadrootsIdParseError::TooLong {
            max: max_len,
            actual: value.len(),
        });
    }
    if value.trim() != value
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(RadrootsIdParseError::InvalidCharacter);
    }
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_64(character: char) -> String {
        core::iter::repeat_n(character, 64).collect()
    }

    fn hex_128(character: char) -> String {
        core::iter::repeat_n(character, 128).collect()
    }

    #[test]
    fn public_keys_and_event_ids_require_64_hex_chars() {
        let upper = "A".repeat(64);
        let public_key = RadrootsPublicKey::parse(&upper).expect("public key");
        assert_eq!(public_key.as_str(), "a".repeat(64));

        let event_id = RadrootsEventId::parse(hex_64('f')).expect("event id");
        assert_eq!(event_id.as_str(), hex_64('f'));
        assert_eq!(
            RadrootsEventId::parse(" ".repeat(64)).unwrap_err(),
            RadrootsIdParseError::InvalidCharacter
        );
        assert_eq!(
            RadrootsEventId::parse("a".repeat(63)).unwrap_err(),
            RadrootsIdParseError::InvalidLength {
                expected: 64,
                actual: 63
            }
        );
    }

    #[test]
    fn signatures_require_128_hex_chars() {
        let signature = RadrootsEventSignature::parse(hex_128('B')).expect("signature");
        assert_eq!(signature.as_str(), "b".repeat(128));
        assert_eq!(
            RadrootsEventSignature::parse(hex_64('b')).unwrap_err(),
            RadrootsIdParseError::InvalidLength {
                expected: 128,
                actual: 64
            }
        );
    }

    #[test]
    fn d_tags_reject_empty_control_and_whitespace() {
        assert_eq!(
            RadrootsDTag::parse("").unwrap_err(),
            RadrootsIdParseError::Empty
        );
        assert_eq!(
            RadrootsDTag::parse(" listing").unwrap_err(),
            RadrootsIdParseError::InvalidCharacter
        );
        assert_eq!(
            RadrootsDTag::parse("listing\none").unwrap_err(),
            RadrootsIdParseError::InvalidCharacter
        );
        assert_eq!(
            RadrootsDTag::parse("farm:farm-1:members")
                .expect("d tag")
                .as_str(),
            "farm:farm-1:members"
        );
    }

    #[test]
    fn addressable_coordinates_validate_kind_pubkey_and_d_tag() {
        let addr = format!("30402:{}:listing-1", hex_64('0'));
        assert_eq!(
            RadrootsAddressableCoordinate::parse(&addr)
                .expect("coordinate")
                .as_str(),
            addr
        );
        assert_eq!(
            RadrootsListingAddress::parse("30402:not_hex:listing-1").unwrap_err(),
            RadrootsIdParseError::InvalidLength {
                expected: 64,
                actual: 7
            }
        );
    }

    #[test]
    fn commercial_ids_reject_empty_whitespace_control_and_long_values() {
        assert_eq!(
            RadrootsOrderId::parse("order-1")
                .expect("order id")
                .as_str(),
            "order-1"
        );
        assert_eq!(
            RadrootsOrderRevisionId::parse("rev 1").unwrap_err(),
            RadrootsIdParseError::InvalidCharacter
        );
        assert_eq!(
            RadrootsInventoryBinId::parse("a".repeat(129)).unwrap_err(),
            RadrootsIdParseError::TooLong {
                max: 128,
                actual: 129
            }
        );
    }

    #[test]
    fn economics_digest_accepts_sha256_and_existing_wire_tokens() {
        let digest = format!("sha256:{}", hex_64('c'));
        assert_eq!(
            RadrootsEconomicsDigest::parse(&digest)
                .expect("digest")
                .as_str(),
            digest
        );
        assert_eq!(
            RadrootsEconomicsDigest::parse("digest-1")
                .expect("wire v1 digest")
                .as_str(),
            "digest-1"
        );
    }

    #[test]
    fn validated_types_do_not_offer_infallible_string_conversion() {
        let id = RadrootsOrderQuoteId::try_from(String::from("quote-1")).expect("quote id");
        assert_eq!(id.as_ref(), "quote-1");
        let parsed: RadrootsEventPointer = hex_64('d').parse().expect("event pointer");
        assert_eq!(parsed.as_str(), hex_64('d'));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_deserialization_validates_identifiers() {
        let encoded = format!("\"{}\"", hex_64('E'));
        let event_id: RadrootsEventId = serde_json::from_str(&encoded).expect("event id");
        assert_eq!(event_id.as_str(), hex_64('e'));

        let invalid = serde_json::from_str::<RadrootsOrderId>("\"bad id\"");
        assert!(invalid.is_err());
        assert_eq!(
            serde_json::to_string(&event_id).expect("json"),
            format!("\"{}\"", hex_64('e'))
        );
    }
}

#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, string::ToString, vec::Vec};

#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

use core::{
    fmt::{self, Write as _},
    str::FromStr,
};
use radroots_identity::PublicKey;
use url_nostd::Url;

use crate::envelope::kind::KIND_CLASSIFIED_LISTING;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    InvalidFormat,
    InvalidLength { expected: usize, actual: usize },
    InvalidCharacter,
    InvalidPublicKey,
    UnexpectedKind { expected: u32, actual: u32 },
    TooLong { max: usize, actual: usize },
}

impl fmt::Display for ParseError {
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
            Self::InvalidPublicKey => {
                write!(f, "identifier is not a valid secp256k1 x-only public key")
            }
            Self::UnexpectedKind { expected, actual } => {
                write!(
                    f,
                    "identifier kind {actual} does not match required kind {expected}"
                )
            }
            Self::TooLong { max, actual } => {
                write!(f, "identifier length {actual} exceeds maximum length {max}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {}

macro_rules! validated_string_id {
    ($name:ident, $validator:ident) => {
        #[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
        #[cfg_attr(all(test, feature = "std"), dto(as = "string"))]
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: impl AsRef<str>) -> Result<Self, ParseError> {
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

        impl From<$name> for String {
            #[inline]
            fn from(value: $name) -> Self {
                value.into_string()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = ParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = ParseError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::parse(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = ParseError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::parse(value)
            }
        }

        #[cfg(any(feature = "serde", test))]
        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        #[cfg(any(feature = "serde", test))]
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

macro_rules! validated_hex_id {
    ($name:ident, $byte_len:expr) => {
        #[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
        #[cfg_attr(all(test, feature = "std"), dto(as = "string"))]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; $byte_len]);

        impl $name {
            pub fn parse(value: impl AsRef<str>) -> Result<Self, ParseError> {
                decode_hex::<$byte_len>(value.as_ref()).map(Self)
            }

            #[inline]
            pub const fn from_bytes(bytes: [u8; $byte_len]) -> Self {
                Self(bytes)
            }

            #[inline]
            pub const fn as_bytes(&self) -> &[u8; $byte_len] {
                &self.0
            }

            #[inline]
            pub const fn into_bytes(self) -> [u8; $byte_len] {
                self.0
            }

            pub fn to_hex(&self) -> String {
                encode_hex(self.0.as_slice())
            }

            #[inline]
            pub fn into_string(self) -> String {
                self.to_hex()
            }
        }

        impl AsRef<[u8]> for $name {
            #[inline]
            fn as_ref(&self) -> &[u8] {
                self.0.as_slice()
            }
        }

        impl From<[u8; $byte_len]> for $name {
            #[inline]
            fn from(bytes: [u8; $byte_len]) -> Self {
                Self::from_bytes(bytes)
            }
        }

        impl From<$name> for [u8; $byte_len] {
            #[inline]
            fn from(value: $name) -> Self {
                value.into_bytes()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write_hex(formatter, self.0.as_slice())
            }
        }

        impl FromStr for $name {
            type Err = ParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = ParseError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::parse(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = ParseError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::parse(value)
            }
        }

        #[cfg(any(feature = "serde", test))]
        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.collect_str(self)
            }
        }

        #[cfg(any(feature = "serde", test))]
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

validated_hex_id!(EventId, 32);
validated_hex_id!(EventSignature, 64);
validated_hex_id!(TradeId, 16);
validated_hex_id!(CandidateId, 32);
validated_hex_id!(MutationId, 32);
validated_string_id!(DTag, validate_d_tag);
validated_string_id!(AddressableCoordinate, validate_addressable_coordinate);
validated_string_id!(
    ClassifiedListingAddress,
    validate_classified_listing_address
);
validated_string_id!(OrderId, validate_commercial_id);
validated_string_id!(OrderQuoteId, validate_commercial_id);
validated_string_id!(InventoryBinId, validate_commercial_id);
validated_string_id!(EconomicsDigest, validate_economics_digest);
validated_hex_id!(EventPointer, 32);
validated_string_id!(RelayUrl, validate_relay_url);

pub(crate) fn parse_public_key(value: impl AsRef<str>) -> Result<PublicKey, ParseError> {
    PublicKey::from_hex(value.as_ref()).map_err(|error| match error {
        radroots_identity::Error::InvalidHexLength { expected, actual }
        | radroots_identity::Error::InvalidByteLength { expected, actual } => {
            ParseError::InvalidLength { expected, actual }
        }
        radroots_identity::Error::InvalidHexCharacter { .. } => ParseError::InvalidCharacter,
        _ => ParseError::InvalidPublicKey,
    })
}

/// Radroots tag-element policy for a NIP-01 coordinate.
///
/// NIP-01 does not define this resource limit.
pub const RADROOTS_NIP01_COORDINATE_MAX_BYTES: usize =
    crate::wire::v1::DEFAULT_TAG_ELEMENT_MAX_BYTES;

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Nip01CoordinateParseError {
    Empty,
    InvalidFormat,
    Pubkey(ParseError),
    UnsupportedKind { actual: u32 },
    IdentifierMustBeEmpty { kind: u32 },
    TooLong { max: usize, actual: usize },
}

impl fmt::Display for Nip01CoordinateParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("NIP-01 coordinate is empty"),
            Self::InvalidFormat => formatter.write_str("NIP-01 coordinate has invalid format"),
            Self::Pubkey(error) => {
                write!(formatter, "NIP-01 coordinate pubkey is invalid: {error}")
            }
            Self::UnsupportedKind { actual } => write!(
                formatter,
                "NIP-01 coordinate kind {actual} is not replaceable or addressable"
            ),
            Self::IdentifierMustBeEmpty { kind } => write!(
                formatter,
                "NIP-01 coordinate identifier must be empty for replaceable kind {kind}"
            ),
            Self::TooLong { max, actual } => write!(
                formatter,
                "NIP-01 coordinate is {actual} bytes; Radroots tag-element max is {max}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Nip01CoordinateParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Pubkey(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Nip01CoordinateParts {
    pub kind: u32,
    pub pubkey: PublicKey,
    pub identifier: String,
}

impl Nip01CoordinateParts {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, Nip01CoordinateParseError> {
        Nip01Coordinate::parse(value).map(Nip01Coordinate::into_parts)
    }
}

/// A canonical NIP-01 replaceable or addressable event coordinate.
///
/// Parsing splits only the first two `:` delimiters. The remaining identifier
/// is opaque protocol data and is preserved byte-for-byte.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Nip01Coordinate {
    canonical: String,
    kind: u32,
    pubkey: PublicKey,
    identifier: String,
}

impl Nip01Coordinate {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, Nip01CoordinateParseError> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(Nip01CoordinateParseError::Empty);
        }
        if value.len() > RADROOTS_NIP01_COORDINATE_MAX_BYTES {
            return Err(Nip01CoordinateParseError::TooLong {
                max: RADROOTS_NIP01_COORDINATE_MAX_BYTES,
                actual: value.len(),
            });
        }

        let (kind, remainder) = value
            .split_once(':')
            .ok_or(Nip01CoordinateParseError::InvalidFormat)?;
        let (pubkey, identifier) = remainder
            .split_once(':')
            .ok_or(Nip01CoordinateParseError::InvalidFormat)?;
        let kind = kind
            .parse::<u32>()
            .map_err(|_| Nip01CoordinateParseError::InvalidFormat)?;
        let requires_empty_identifier = matches!(kind, 0 | 3) || (10_000..=19_999).contains(&kind);
        if !requires_empty_identifier && !(30_000..=39_999).contains(&kind) {
            return Err(Nip01CoordinateParseError::UnsupportedKind { actual: kind });
        }
        if requires_empty_identifier && !identifier.is_empty() {
            return Err(Nip01CoordinateParseError::IdentifierMustBeEmpty { kind });
        }

        let pubkey = parse_public_key(pubkey).map_err(Nip01CoordinateParseError::Pubkey)?;
        let identifier = identifier.to_string();
        let canonical = format!("{kind}:{pubkey}:{identifier}");
        Ok(Self {
            canonical,
            kind,
            pubkey,
            identifier,
        })
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        self.canonical.as_str()
    }

    #[inline]
    pub const fn kind(&self) -> u32 {
        self.kind
    }

    #[inline]
    pub const fn pubkey(&self) -> &PublicKey {
        &self.pubkey
    }

    #[inline]
    pub fn identifier(&self) -> &str {
        self.identifier.as_str()
    }

    #[inline]
    pub fn parts(&self) -> Nip01CoordinateParts {
        Nip01CoordinateParts {
            kind: self.kind,
            pubkey: self.pubkey,
            identifier: self.identifier.clone(),
        }
    }

    #[inline]
    pub fn into_parts(self) -> Nip01CoordinateParts {
        Nip01CoordinateParts {
            kind: self.kind,
            pubkey: self.pubkey,
            identifier: self.identifier,
        }
    }

    #[inline]
    pub fn into_string(self) -> String {
        self.canonical
    }
}

impl AsRef<str> for Nip01Coordinate {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<Nip01Coordinate> for String {
    #[inline]
    fn from(value: Nip01Coordinate) -> Self {
        value.into_string()
    }
}

impl fmt::Display for Nip01Coordinate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Nip01Coordinate {
    type Err = Nip01CoordinateParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<&str> for Nip01Coordinate {
    type Error = Nip01CoordinateParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for Nip01Coordinate {
    type Error = Nip01CoordinateParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

#[cfg(any(feature = "serde", test))]
impl serde::Serialize for Nip01Coordinate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for Nip01Coordinate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AddressableCoordinateParts {
    pub kind: u32,
    pub pubkey: PublicKey,
    pub d_tag: DTag,
}

impl AddressableCoordinateParts {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, ParseError> {
        parse_addressable_coordinate_parts(value.as_ref())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventEnvelopePointer {
    pub event_id: EventId,
    pub relays: Vec<String>,
}

impl EventEnvelopePointer {
    pub fn new<I, S>(event_id: EventId, relays: I) -> Result<Self, ParseError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut canonical_relays = Vec::new();
        for relay in relays {
            let relay = relay.into();
            RelayUrl::parse(relay.as_str())?;
            canonical_relays.push(relay);
        }
        Ok(Self {
            event_id,
            relays: canonical_relays,
        })
    }
}

fn decode_hex<const N: usize>(value: &str) -> Result<[u8; N], ParseError> {
    let expected = N * 2;
    if value.len() != expected {
        return Err(ParseError::InvalidLength {
            expected,
            actual: value.len(),
        });
    }

    let mut decoded = [0_u8; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        decoded[index] = (decode_hex_nibble(pair[0])? << 4) | decode_hex_nibble(pair[1])?;
    }
    Ok(decoded)
}

fn decode_hex_nibble(value: u8) -> Result<u8, ParseError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(ParseError::InvalidCharacter),
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const LOWER_HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(LOWER_HEX[usize::from(byte >> 4)] as char);
        encoded.push(LOWER_HEX[usize::from(byte & 0x0f)] as char);
    }
    encoded
}

fn write_hex(formatter: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for byte in bytes {
        formatter.write_char(lower_hex_digit(byte >> 4))?;
        formatter.write_char(lower_hex_digit(byte & 0x0f))?;
    }
    Ok(())
}

const fn lower_hex_digit(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        _ => (b'a' + value - 10) as char,
    }
}

fn validate_hex(value: &str, expected_len: usize) -> Result<String, ParseError> {
    if value.len() != expected_len {
        return Err(ParseError::InvalidLength {
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
            _ => return Err(ParseError::InvalidCharacter),
        }
    }
    Ok(canonical)
}

fn validate_d_tag(value: &str) -> Result<String, ParseError> {
    validate_visible_token(value, 512)
}

fn validate_commercial_id(value: &str) -> Result<String, ParseError> {
    validate_visible_token(value, 128)
}

fn validate_economics_digest(value: &str) -> Result<String, ParseError> {
    if let Some(hex) = value.strip_prefix("sha256:") {
        validate_hex(hex, 64)?;
        return Ok(value.to_string());
    }
    validate_visible_token(value, 128)
}

fn validate_addressable_coordinate(value: &str) -> Result<String, ParseError> {
    parse_addressable_coordinate_parts(value)?;
    Ok(value.to_string())
}

fn validate_classified_listing_address(value: &str) -> Result<String, ParseError> {
    let parts = parse_addressable_coordinate_parts(value)?;
    if parts.kind != KIND_CLASSIFIED_LISTING {
        return Err(ParseError::UnexpectedKind {
            expected: KIND_CLASSIFIED_LISTING,
            actual: parts.kind,
        });
    }
    Ok(format!(
        "{}:{}:{}",
        KIND_CLASSIFIED_LISTING, parts.pubkey, parts.d_tag
    ))
}

fn parse_addressable_coordinate_parts(
    value: &str,
) -> Result<AddressableCoordinateParts, ParseError> {
    let (kind, remainder) = value.split_once(':').ok_or(ParseError::InvalidFormat)?;
    let (pubkey, d_tag) = remainder.split_once(':').ok_or(ParseError::InvalidFormat)?;
    let kind = kind.parse::<u32>().map_err(|_| ParseError::InvalidFormat)?;
    let pubkey = parse_public_key(pubkey)?;
    let d_tag = DTag::parse(d_tag)?;
    Ok(AddressableCoordinateParts {
        kind,
        pubkey,
        d_tag,
    })
}

fn validate_visible_token(value: &str, max_len: usize) -> Result<String, ParseError> {
    if value.is_empty() {
        return Err(ParseError::Empty);
    }
    if value.len() > max_len {
        return Err(ParseError::TooLong {
            max: max_len,
            actual: value.len(),
        });
    }
    if value.trim() != value
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(ParseError::InvalidCharacter);
    }
    Ok(value.to_string())
}

pub fn relay_url_is_valid(value: &str) -> bool {
    validate_relay_url(value).is_ok()
}

fn validate_relay_url(value: &str) -> Result<String, ParseError> {
    if value.is_empty() {
        return Err(ParseError::Empty);
    }
    if value
        .chars()
        .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(ParseError::InvalidCharacter);
    }
    if !(value.starts_with("ws://") || value.starts_with("wss://")) {
        return Err(ParseError::InvalidFormat);
    }

    let parsed = Url::parse(value).map_err(|_| ParseError::InvalidFormat)?;
    if !matches!(parsed.scheme(), "ws" | "wss")
        || parsed.host_str().is_none_or(str::is_empty)
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
        || parsed.port() == Some(0)
    {
        return Err(ParseError::InvalidFormat);
    }

    // `Url::username` cannot distinguish absent userinfo from an empty userinfo field.
    let authority = value
        .split_once("://")
        .map(|(_, remainder)| remainder.split(['/', '?', '#']).next().unwrap_or(remainder))
        .ok_or(ParseError::InvalidFormat)?;
    if authority.contains('@') {
        return Err(ParseError::InvalidFormat);
    }

    Ok(value.to_string())
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    macro_rules! assert_string_identifier_impls {
        ($ty:ty, $value:expr) => {{
            let value = $value.to_owned();
            let value = value.as_str();
            let id = <$ty>::parse(value).expect("parse");

            assert_eq!(id.as_str(), value);
            assert_eq!(id.as_ref(), value);
            assert_eq!(id.to_string(), value);
            assert_eq!(
                <$ty as core::str::FromStr>::from_str(value).expect("from str"),
                id
            );
            assert_eq!(
                <$ty as TryFrom<&str>>::try_from(value).expect("try from str"),
                id
            );
            assert_eq!(
                <$ty as TryFrom<String>>::try_from(value.to_owned()).expect("try from string"),
                id
            );
            let id = <$ty>::parse(value).expect("parse");
            let converted: String = String::from(id.clone());
            assert_eq!(converted, value);
            assert_eq!(id.into_string(), value.to_owned());

            #[cfg(any(feature = "serde", test))]
            {
                let id = <$ty>::parse(value).expect("parse");
                let encoded = serde_json::to_string(&id).expect("serialize");
                let decoded: $ty = serde_json::from_str(&encoded).expect("deserialize");
                assert_eq!(decoded.as_str(), value);
            }
        }};
    }

    macro_rules! assert_hex_identifier_impls {
        ($ty:ty, $value:expr, $byte_len:expr) => {{
            let value = $value.to_owned();
            let value = value.as_str();
            let id = <$ty>::parse(value).expect("parse");

            assert_eq!(id.to_hex(), value);
            assert_eq!(id.as_bytes().len(), $byte_len);
            assert_eq!(id.as_ref(), id.as_bytes());
            assert_eq!(id.to_string(), value);
            assert_eq!(
                <$ty as core::str::FromStr>::from_str(value).expect("from str"),
                id
            );
            assert_eq!(
                <$ty as TryFrom<&str>>::try_from(value).expect("try from str"),
                id
            );
            assert_eq!(
                <$ty as TryFrom<String>>::try_from(value.to_owned()).expect("try from string"),
                id
            );

            let bytes = id.into_bytes();
            assert_eq!(<$ty>::from_bytes(bytes), id);
            assert_eq!(id.into_string(), value.to_owned());

            #[cfg(any(feature = "serde", test))]
            {
                let encoded = serde_json::to_string(&id).expect("serialize");
                let decoded: $ty = serde_json::from_str(&encoded).expect("deserialize");
                assert_eq!(decoded.to_hex(), value);
            }
        }};
    }

    fn hex_64(character: char) -> String {
        crate::test_valid_hex_64(character)
    }

    fn hex_32(character: char) -> String {
        core::iter::repeat_n(character, 32).collect()
    }

    fn hex_128(character: char) -> String {
        core::iter::repeat_n(character, 128).collect()
    }

    #[test]
    fn public_keys_and_event_ids_require_64_hex_chars() {
        let upper = "585591529DA0BAB31B3B1B1F986611CF5F435DCA84F978C89EE8A40CCA7103DF";
        let public_key = parse_public_key(upper).expect("public key");
        assert_eq!(
            public_key.to_hex(),
            "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df"
        );

        let event_id = EventId::parse(hex_64('f')).expect("event id");
        assert_eq!(event_id.to_hex(), hex_64('f'));
        assert_eq!(
            EventId::parse(" ".repeat(64)).unwrap_err(),
            ParseError::InvalidCharacter
        );
        assert_eq!(
            EventId::parse("a".repeat(63)).unwrap_err(),
            ParseError::InvalidLength {
                expected: 64,
                actual: 63
            }
        );
    }

    #[test]
    fn id_parse_errors_have_stable_display_messages() {
        let errors = [
            ParseError::Empty,
            ParseError::InvalidFormat,
            ParseError::InvalidLength {
                expected: 64,
                actual: 7,
            },
            ParseError::InvalidCharacter,
            ParseError::InvalidPublicKey,
            ParseError::UnexpectedKind {
                expected: KIND_CLASSIFIED_LISTING,
                actual: 30023,
            },
            ParseError::TooLong {
                max: 128,
                actual: 129,
            },
        ];

        for error in errors {
            assert!(!error.to_string().is_empty());
        }
        assert_eq!(
            ParseError::UnexpectedKind {
                expected: KIND_CLASSIFIED_LISTING,
                actual: 30023,
            }
            .to_string(),
            "identifier kind 30023 does not match required kind 30402"
        );
    }

    #[test]
    fn signatures_require_128_hex_chars() {
        let signature = EventSignature::parse(hex_128('B')).expect("signature");
        assert_eq!(signature.to_hex(), "b".repeat(128));
        assert_eq!(
            EventSignature::parse(hex_64('b')).unwrap_err(),
            ParseError::InvalidLength {
                expected: 128,
                actual: 64
            }
        );
    }

    #[test]
    fn trade_semantic_ids_use_protocol_sized_hex() {
        let trade_id = TradeId::parse(hex_32('A')).expect("trade id");
        assert_eq!(trade_id.to_hex(), hex_32('a'));
        assert_eq!(
            TradeId::parse(hex_64('a')).unwrap_err(),
            ParseError::InvalidLength {
                expected: 32,
                actual: 64
            }
        );
        assert_hex_identifier_impls!(TradeId, &hex_32('a'), 16);
        assert_hex_identifier_impls!(CandidateId, &hex_64('b'), 32);
        assert_hex_identifier_impls!(MutationId, &hex_64('c'), 32);
    }

    #[test]
    fn d_tags_reject_empty_control_and_whitespace() {
        assert_eq!(DTag::parse("").unwrap_err(), ParseError::Empty);
        assert_eq!(
            DTag::parse(" listing").unwrap_err(),
            ParseError::InvalidCharacter
        );
        assert_eq!(
            DTag::parse("listing\none").unwrap_err(),
            ParseError::InvalidCharacter
        );
        assert_eq!(
            DTag::parse("farm:farm-1:members").expect("d tag").as_str(),
            "farm:farm-1:members"
        );
    }

    #[test]
    fn addressable_coordinates_validate_kind_pubkey_and_d_tag() {
        let addr = format!("30402:{}:listing-1", hex_64('0'));
        assert_eq!(
            AddressableCoordinate::parse(&addr)
                .expect("coordinate")
                .as_str(),
            addr
        );
        assert_eq!(
            ClassifiedListingAddress::parse("30402:not_hex:listing-1").unwrap_err(),
            ParseError::InvalidLength {
                expected: 64,
                actual: 7
            }
        );
        assert_eq!(
            ClassifiedListingAddress::parse(format!("30023:{}:listing-1", hex_64('0')))
                .unwrap_err(),
            ParseError::UnexpectedKind {
                expected: KIND_CLASSIFIED_LISTING,
                actual: 30023,
            }
        );
        let canonical_pubkey = hex_64('a');
        for noncanonical_kind in ["030402", "+30402"] {
            assert_eq!(
                ClassifiedListingAddress::parse(format!(
                    "{noncanonical_kind}:{}:listing-1",
                    hex_64('A')
                ))
                .expect("classified listing coordinate")
                .as_str(),
                format!("30402:{canonical_pubkey}:listing-1")
            );
        }
        assert_eq!(
            AddressableCoordinate::parse("30402").unwrap_err(),
            ParseError::InvalidFormat
        );
        assert_eq!(
            AddressableCoordinate::parse(format!("bad:{}:listing-1", hex_64('a'))).unwrap_err(),
            ParseError::InvalidFormat
        );
        assert_eq!(
            AddressableCoordinate::parse(format!("30402:{}:bad d", hex_64('0'))).unwrap_err(),
            ParseError::InvalidCharacter
        );

        let noncanonical = format!("030402:{}:listing-1", hex_64('A'));
        assert_eq!(
            AddressableCoordinate::parse(&noncanonical)
                .expect("validated addressable coordinate")
                .as_str(),
            noncanonical
        );
    }

    #[test]
    fn binary_identifier_order_matches_canonical_hex_order() {
        let lower = EventId::from_bytes([0_u8; 32]);
        let mut upper_bytes = [0_u8; 32];
        upper_bytes[31] = 1;
        let upper = EventId::from_bytes(upper_bytes);

        assert!(lower < upper);
        assert!(lower.to_hex() < upper.to_hex());

        let mut identifiers = [upper, lower];
        identifiers.sort();
        assert_eq!(identifiers, [lower, upper]);
    }

    #[test]
    fn addressable_coordinate_parts_parse_kind_pubkey_and_d_tag() {
        let addr = format!("30402:{}:farm:farm-1:members", hex_64('A'));
        let parts = AddressableCoordinateParts::parse(&addr).expect("coordinate parts");
        assert_eq!(parts.kind, 30402);
        assert_eq!(parts.pubkey.to_hex(), hex_64('a'));
        assert_eq!(parts.d_tag.as_str(), "farm:farm-1:members");
    }

    #[test]
    fn nip01_coordinates_cover_replaceable_and_addressable_kinds() {
        for kind in [0, 3, 10_000, 19_999] {
            let coordinate = Nip01Coordinate::parse(format!("+0{kind}:{}:", hex_64('A')))
                .expect("replaceable coordinate");
            assert_eq!(coordinate.kind(), kind);
            assert_eq!(coordinate.pubkey().to_hex(), hex_64('a'));
            assert_eq!(coordinate.identifier(), "");
            assert_eq!(coordinate.as_str(), format!("{kind}:{}:", hex_64('a')));
        }

        for kind in [30_000, 39_999] {
            let coordinate = Nip01Coordinate::parse(format!("{kind}:{}:", hex_64('b')))
                .expect("empty addressable coordinate");
            assert_eq!(coordinate.kind(), kind);
            assert_eq!(coordinate.identifier(), "");
        }
    }

    #[test]
    fn nip01_coordinate_identifier_is_opaque_after_second_colon() {
        let identifier = "  victoria:\u{0000}seed:\u{2603}\n";
        let coordinate = Nip01Coordinate::parse(format!("030402:{}:{identifier}", hex_64('A')))
            .expect("opaque addressable coordinate");

        assert_eq!(coordinate.kind(), 30_402);
        assert_eq!(coordinate.pubkey().to_hex(), hex_64('a'));
        assert_eq!(coordinate.identifier().as_bytes(), identifier.as_bytes());
        assert_eq!(
            coordinate.as_str().as_bytes(),
            format!("30402:{}:{identifier}", hex_64('a')).as_bytes()
        );
        let parts = coordinate.parts();
        assert_eq!(parts.kind, 30_402);
        assert_eq!(parts.pubkey.to_hex(), hex_64('a'));
        assert_eq!(parts.identifier.as_bytes(), identifier.as_bytes());
        assert_eq!(
            Nip01CoordinateParts::parse(coordinate.as_str()).expect("parts"),
            parts
        );
    }

    #[test]
    fn nip01_coordinates_reject_unsupported_shapes_and_kinds() {
        let pubkey = hex_64('a');
        assert_eq!(
            Nip01Coordinate::parse("").unwrap_err(),
            Nip01CoordinateParseError::Empty
        );
        assert_eq!(
            Nip01Coordinate::parse("30000").unwrap_err(),
            Nip01CoordinateParseError::InvalidFormat
        );
        assert_eq!(
            Nip01Coordinate::parse("30000:bad:identifier").unwrap_err(),
            Nip01CoordinateParseError::Pubkey(ParseError::InvalidLength {
                expected: 64,
                actual: 3
            })
        );
        for kind in [1, 9_999, 20_000, 29_999, 40_000, u32::MAX] {
            assert_eq!(
                Nip01Coordinate::parse(format!("{kind}:{pubkey}:")).unwrap_err(),
                Nip01CoordinateParseError::UnsupportedKind { actual: kind }
            );
        }
        for kind in [0, 3, 10_000, 19_999] {
            assert_eq!(
                Nip01Coordinate::parse(format!("{kind}:{pubkey}:not-empty")).unwrap_err(),
                Nip01CoordinateParseError::IdentifierMustBeEmpty { kind }
            );
        }
        assert_eq!(
            Nip01Coordinate::parse(format!("30000:{pubkey}")).unwrap_err(),
            Nip01CoordinateParseError::InvalidFormat
        );
    }

    #[test]
    fn nip01_coordinate_errors_and_policy_limit_are_explicit() {
        assert_eq!(
            RADROOTS_NIP01_COORDINATE_MAX_BYTES,
            crate::wire::v1::DEFAULT_TAG_ELEMENT_MAX_BYTES
        );
        let errors = [
            Nip01CoordinateParseError::Empty,
            Nip01CoordinateParseError::InvalidFormat,
            Nip01CoordinateParseError::Pubkey(ParseError::InvalidCharacter),
            Nip01CoordinateParseError::UnsupportedKind { actual: 20_000 },
            Nip01CoordinateParseError::IdentifierMustBeEmpty { kind: 10_000 },
            Nip01CoordinateParseError::TooLong {
                max: RADROOTS_NIP01_COORDINATE_MAX_BYTES,
                actual: RADROOTS_NIP01_COORDINATE_MAX_BYTES + 1,
            },
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn nip01_coordinate_enforces_entire_element_byte_limit() {
        let prefix = format!("30000:{}:", hex_64('a'));
        let exact = format!(
            "{prefix}{}",
            "x".repeat(RADROOTS_NIP01_COORDINATE_MAX_BYTES - prefix.len())
        );
        let coordinate = Nip01Coordinate::parse(&exact).expect("exact coordinate byte limit");
        assert_eq!(
            coordinate.as_str().len(),
            RADROOTS_NIP01_COORDINATE_MAX_BYTES
        );

        assert_eq!(
            Nip01Coordinate::parse(format!("{exact}x")).unwrap_err(),
            Nip01CoordinateParseError::TooLong {
                max: RADROOTS_NIP01_COORDINATE_MAX_BYTES,
                actual: RADROOTS_NIP01_COORDINATE_MAX_BYTES + 1,
            }
        );
    }

    #[test]
    fn nip01_coordinate_enforces_multibyte_boundary_by_utf8_bytes() {
        let prefix = format!("30000:{}:", hex_64('a'));
        let remaining = RADROOTS_NIP01_COORDINATE_MAX_BYTES - prefix.len();
        let identifier = format!("{}x", "\u{00e9}".repeat((remaining - 1) / 2));
        let exact = format!("{prefix}{identifier}");
        assert_eq!(exact.len(), RADROOTS_NIP01_COORDINATE_MAX_BYTES);
        Nip01Coordinate::parse(&exact).expect("exact multibyte coordinate limit");

        let overflow = format!("{exact}x");
        assert_eq!(
            Nip01Coordinate::parse(&overflow).unwrap_err(),
            Nip01CoordinateParseError::TooLong {
                max: RADROOTS_NIP01_COORDINATE_MAX_BYTES,
                actual: RADROOTS_NIP01_COORDINATE_MAX_BYTES + 1,
            }
        );
    }

    #[test]
    fn nip01_coordinate_exposes_explicit_text_and_validating_serde() {
        let value = format!("30000:{}:farm:victoria", hex_64('a'));
        let coordinate = Nip01Coordinate::parse(&value).expect("coordinate");
        assert_eq!(coordinate.as_ref(), value);
        assert_eq!(coordinate.to_string(), value);
        assert_eq!(
            Nip01Coordinate::from_str(&value).expect("from str"),
            coordinate
        );
        assert_eq!(
            Nip01Coordinate::try_from(value.clone()).expect("try from string"),
            coordinate
        );
        assert_eq!(String::from(coordinate.clone()), value);

        let encoded = serde_json::to_string(&coordinate).expect("serialize");
        assert_eq!(
            serde_json::from_str::<Nip01Coordinate>(&encoded).expect("deserialize"),
            coordinate
        );
        assert!(serde_json::from_str::<Nip01Coordinate>("\"1:bad:\"").is_err());
    }

    #[test]
    fn commercial_ids_reject_empty_whitespace_control_and_long_values() {
        assert_eq!(
            OrderId::parse("order-1").expect("order id").as_str(),
            "order-1"
        );
        assert_eq!(
            InventoryBinId::parse("a".repeat(129)).unwrap_err(),
            ParseError::TooLong {
                max: 128,
                actual: 129
            }
        );
    }

    #[test]
    fn economics_digest_accepts_sha256_and_existing_wire_tokens() {
        let digest = format!("sha256:{}", hex_64('c'));
        assert_eq!(
            EconomicsDigest::parse(&digest).expect("digest").as_str(),
            digest
        );
        assert_eq!(
            EconomicsDigest::parse("digest-1")
                .expect("wire v1 digest")
                .as_str(),
            "digest-1"
        );
        assert_eq!(
            EconomicsDigest::parse("sha256:not-hex").unwrap_err(),
            ParseError::InvalidLength {
                expected: 64,
                actual: 7
            }
        );
    }

    #[test]
    fn validated_types_do_not_offer_infallible_string_conversion() {
        let id = OrderQuoteId::try_from(String::from("quote-1")).expect("quote id");
        assert_eq!(id.as_ref(), "quote-1");
        let parsed: EventPointer = hex_64('d').parse().expect("event pointer");
        assert_eq!(parsed.to_hex(), hex_64('d'));
    }

    #[test]
    fn validated_identifier_wrappers_expose_consistent_traits() {
        let addressable = format!("30402:{}:listing-1", hex_64('0'));

        assert_hex_identifier_impls!(EventId, hex_64('b').as_str(), 32);
        assert_hex_identifier_impls!(EventSignature, hex_128('c').as_str(), 64);
        assert_string_identifier_impls!(DTag, "listing-1");
        assert_string_identifier_impls!(AddressableCoordinate, addressable.as_str());
        assert_string_identifier_impls!(ClassifiedListingAddress, addressable.as_str());
        assert_string_identifier_impls!(OrderId, "order-1");
        assert_string_identifier_impls!(OrderQuoteId, "quote-1");
        assert_string_identifier_impls!(InventoryBinId, "bin-1");
        assert_string_identifier_impls!(EconomicsDigest, "digest-1");
        assert_hex_identifier_impls!(EventPointer, hex_64('d').as_str(), 32);
        assert_string_identifier_impls!(RelayUrl, "wss://relay.example.com");
    }

    #[test]
    fn relay_urls_require_valid_websocket_urls() {
        assert_eq!(
            RelayUrl::parse("ws://relay.example.com")
                .expect("relay url")
                .as_str(),
            "ws://relay.example.com"
        );
        assert_eq!(
            RelayUrl::parse("wss://relay.example.com")
                .expect("relay url")
                .as_str(),
            "wss://relay.example.com"
        );
        assert!(relay_url_is_valid("wss://relay.example.com"));
        assert_eq!(RelayUrl::parse("").unwrap_err(), ParseError::Empty);
        assert_eq!(
            RelayUrl::parse("http://relay.example.com").unwrap_err(),
            ParseError::InvalidFormat
        );
        assert_eq!(
            RelayUrl::parse("WSS://relay.example.com").unwrap_err(),
            ParseError::InvalidFormat
        );
        assert_eq!(
            RelayUrl::parse("ws://").unwrap_err(),
            ParseError::InvalidFormat
        );
        assert_eq!(
            RelayUrl::parse("wss://").unwrap_err(),
            ParseError::InvalidFormat
        );
        assert_eq!(
            RelayUrl::parse(" wss://relay.example.com").unwrap_err(),
            ParseError::InvalidCharacter
        );
        assert_eq!(
            RelayUrl::parse("wss://relay.example.com\nmiddle").unwrap_err(),
            ParseError::InvalidCharacter
        );
        assert_eq!(
            RelayUrl::parse("wss://user@relay.example.com").unwrap_err(),
            ParseError::InvalidFormat
        );
        assert_eq!(
            RelayUrl::parse("wss://user:secret@relay.example.com").unwrap_err(),
            ParseError::InvalidFormat
        );
        assert_eq!(
            RelayUrl::parse("wss://relay.example.com#read").unwrap_err(),
            ParseError::InvalidFormat
        );
        assert_eq!(
            RelayUrl::parse("wss://relay.example.com:0").unwrap_err(),
            ParseError::InvalidFormat
        );
        assert_eq!(
            RelayUrl::parse("wss://relay.example.com/nostr/v1?region=ca-bc")
                .expect("relay URL with path and query")
                .as_str(),
            "wss://relay.example.com/nostr/v1?region=ca-bc"
        );
    }

    #[test]
    fn nostr_event_pointers_validate_relay_values() {
        let event_id = EventId::parse(hex_64('e')).expect("event id");
        let pointer = EventEnvelopePointer::new(
            event_id,
            ["wss://relay.one.example", "wss://relay.two.example"],
        )
        .expect("pointer");

        assert_eq!(pointer.event_id, event_id);
        assert_eq!(
            pointer.relays,
            vec![
                "wss://relay.one.example".to_owned(),
                "wss://relay.two.example".to_owned()
            ]
        );

        assert_eq!(
            EventEnvelopePointer::new(EventId::parse(hex_64('e')).expect("event id"), [""],)
                .unwrap_err(),
            ParseError::Empty
        );
        assert_eq!(
            EventEnvelopePointer::new(
                EventId::parse(hex_64('e')).expect("event id"),
                ["http://relay.example"],
            )
            .unwrap_err(),
            ParseError::InvalidFormat
        );
        assert_eq!(
            EventEnvelopePointer::new(
                EventId::parse(hex_64('e')).expect("event id"),
                [" wss://relay.example"],
            )
            .unwrap_err(),
            ParseError::InvalidCharacter
        );
        assert_eq!(
            EventEnvelopePointer::new(
                EventId::parse(hex_64('e')).expect("event id"),
                ["wss://relay.example\n"],
            )
            .unwrap_err(),
            ParseError::InvalidCharacter
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_deserialization_validates_identifiers() {
        let encoded = format!("\"{}\"", hex_64('E'));
        let event_id: EventId = serde_json::from_str(&encoded).expect("event id");
        assert_eq!(event_id.to_hex(), hex_64('e'));

        let invalid = serde_json::from_str::<OrderId>("\"bad id\"");
        assert!(invalid.is_err());
        assert_eq!(
            serde_json::to_string(&event_id).expect("json"),
            format!("\"{}\"", hex_64('e'))
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_missing_fields_exercise_identifier_deserializers() {
        #[allow(dead_code)]
        #[derive(Debug, serde::Deserialize)]
        struct MissingPublicKey {
            value: PublicKey,
        }
        #[allow(dead_code)]
        #[derive(Debug, serde::Deserialize)]
        struct MissingEventId {
            value: EventId,
        }
        #[allow(dead_code)]
        #[derive(Debug, serde::Deserialize)]
        struct MissingEventSignature {
            value: EventSignature,
        }
        #[allow(dead_code)]
        #[derive(Debug, serde::Deserialize)]
        struct MissingTradeId {
            value: TradeId,
        }
        #[allow(dead_code)]
        #[derive(Debug, serde::Deserialize)]
        struct MissingTradeCandidateId {
            value: CandidateId,
        }
        #[allow(dead_code)]
        #[derive(Debug, serde::Deserialize)]
        struct MissingTradeMutationId {
            value: MutationId,
        }
        #[allow(dead_code)]
        #[derive(Debug, serde::Deserialize)]
        struct MissingDTag {
            value: DTag,
        }
        #[allow(dead_code)]
        #[derive(Debug, serde::Deserialize)]
        struct MissingClassifiedListingAddress {
            value: ClassifiedListingAddress,
        }
        #[allow(dead_code)]
        #[derive(Debug, serde::Deserialize)]
        struct MissingAddressableCoordinate {
            value: AddressableCoordinate,
        }
        #[allow(dead_code)]
        #[derive(Debug, serde::Deserialize)]
        struct MissingOrderId {
            value: OrderId,
        }
        #[allow(dead_code)]
        #[derive(Debug, serde::Deserialize)]
        struct MissingOrderQuoteId {
            value: OrderQuoteId,
        }
        #[allow(dead_code)]
        #[derive(Debug, serde::Deserialize)]
        struct MissingInventoryBinId {
            value: InventoryBinId,
        }

        fn missing_field_message<T>() -> String
        where
            T: serde::de::DeserializeOwned + core::fmt::Debug,
        {
            serde_json::from_str::<T>("{}")
                .expect_err("missing field")
                .to_string()
        }

        let missing = "missing field `value` at line 1 column 2";
        assert_eq!(missing_field_message::<MissingPublicKey>(), missing);
        assert_eq!(missing_field_message::<MissingEventId>(), missing);
        assert_eq!(missing_field_message::<MissingEventSignature>(), missing);
        assert_eq!(missing_field_message::<MissingTradeId>(), missing);
        assert_eq!(missing_field_message::<MissingTradeCandidateId>(), missing);
        assert_eq!(missing_field_message::<MissingTradeMutationId>(), missing);
        assert_eq!(missing_field_message::<MissingDTag>(), missing);
        assert_eq!(
            missing_field_message::<MissingClassifiedListingAddress>(),
            missing
        );
        assert_eq!(
            missing_field_message::<MissingAddressableCoordinate>(),
            missing
        );
        assert_eq!(missing_field_message::<MissingOrderId>(), missing);
        assert_eq!(missing_field_message::<MissingOrderQuoteId>(), missing);
        assert_eq!(missing_field_message::<MissingInventoryBinId>(), missing);

        let order: OrderId =
            serde_json::from_value(serde_json::json!("order-1")).expect("order from value");
        let listing: ClassifiedListingAddress = serde_json::from_value(serde_json::json!(format!(
            "30402:{}:listing-1",
            hex_64('a')
        )))
        .expect("listing from value");
        let quote: OrderQuoteId =
            serde_json::from_value(serde_json::json!("quote-1")).expect("quote from value");
        assert_eq!(order.as_str(), "order-1");
        assert_eq!(listing.as_str().split(':').next(), Some("30402"));
        assert_eq!(quote.as_str(), "quote-1");
    }
}

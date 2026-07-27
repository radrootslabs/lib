//! Canonical three-letter currency codes and currency-specific minor units.

use core::fmt;
use core::str::FromStr;

#[cfg(all(feature = "serde", not(feature = "std")))]
use alloc::string::String;
#[cfg(feature = "serde")]
#[cfg(feature = "std")]
use std::string::String;

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as DeError};

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(as = "string"))]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Currency([u8; 3]);

impl Currency {
    /// Builds a currency from its canonical three-byte ASCII code.
    #[inline]
    pub const fn from_const(bytes: [u8; 3]) -> Result<Self, ParseError> {
        if Self::is_ascii_upper(bytes[0])
            && Self::is_ascii_upper(bytes[1])
            && Self::is_ascii_upper(bytes[2])
        {
            Ok(Self(bytes))
        } else {
            Err(ParseError::InvalidFormat)
        }
    }

    const fn is_ascii_upper(byte: u8) -> bool {
        byte >= b'A' && byte <= b'Z'
    }

    #[inline]
    pub fn from_str_upper(s: &str) -> Result<Self, ParseError> {
        let b = s.as_bytes();
        if b.len() != 3 || b.iter().any(|c| !c.is_ascii_uppercase()) {
            return Err(ParseError::InvalidFormat);
        }
        Ok(Self([b[0], b[1], b[2]]))
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.0).unwrap_or("???")
    }

    #[inline]
    pub const fn as_bytes(&self) -> &[u8; 3] {
        &self.0
    }

    pub const USD: Currency = Currency(*b"USD");
    pub const EUR: Currency = Currency(*b"EUR");
    pub const GBP: Currency = Currency(*b"GBP");
    pub const JPY: Currency = Currency(*b"JPY");
    pub const CAD: Currency = Currency(*b"CAD");
    pub const AUD: Currency = Currency(*b"AUD");

    #[inline]
    pub const fn minor_unit_exponent(&self) -> u32 {
        match self.0 {
            [b'J', b'P', b'Y'] | [b'K', b'R', b'W'] | [b'V', b'N', b'D'] => 0,
            [b'B', b'H', b'D']
            | [b'I', b'Q', b'D']
            | [b'J', b'O', b'D']
            | [b'K', b'W', b'D']
            | [b'L', b'Y', b'D']
            | [b'O', b'M', b'R']
            | [b'T', b'N', b'D'] => 3,
            _ => 2,
        }
    }
}

impl fmt::Debug for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Currency").field(&self.as_str()).finish()
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for Currency {
    type Error = ParseError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl FromStr for Currency {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Currency input is canonicalized to uppercase ASCII. Serialization
        // and Display always emit that canonical representation.
        let s = s.trim();
        if s.len() != 3 || !s.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(ParseError::InvalidFormat);
        }
        let upper = s.to_ascii_uppercase();
        Self::from_str_upper(&upper)
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    InvalidFormat,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidFormat => {
                write!(f, "currency must be a 3-letter code")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {}

#[cfg(feature = "serde")]
impl Serialize for Currency {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Currency {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = String::deserialize(de)?;
        s.parse().map_err(D::Error::custom)
    }
}

#[deprecated(since = "0.1.0", note = "renamed to `Currency`")]
pub use self::Currency as RadrootsCoreCurrency;

#[deprecated(since = "0.1.0", note = "renamed to `currency::ParseError`")]
pub use self::ParseError as RadrootsCoreCurrencyParseError;

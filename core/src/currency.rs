use core::fmt;
use core::str::FromStr;

#[cfg(feature = "serde")]
use serde::{de::Error as DeError, Deserialize, Deserializer, Serialize, Serializer};

#[typeshare::typeshare]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsCoreCurrency([u8; 3]);

impl RadrootsCoreCurrency {
    #[inline]
    pub const fn from_const(bytes: [u8; 3]) -> Self {
        Self(bytes)
    }

    #[inline]
    pub fn from_str_upper(s: &str) -> Result<Self, RadrootsCoreCurrencyParseError> {
        let b = s.as_bytes();
        if b.len() != 3 || b.iter().any(|c| !c.is_ascii_uppercase()) {
            return Err(RadrootsCoreCurrencyParseError::InvalidFormat);
        }
        Ok(Self([b[0], b[1], b[2]]))
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.0).expect("currency bytes are validated on construction")
    }

    pub const USD: RadrootsCoreCurrency = RadrootsCoreCurrency(*b"USD");
    pub const EUR: RadrootsCoreCurrency = RadrootsCoreCurrency(*b"EUR");
    pub const GBP: RadrootsCoreCurrency = RadrootsCoreCurrency(*b"GBP");
    pub const JPY: RadrootsCoreCurrency = RadrootsCoreCurrency(*b"JPY");
    pub const CAD: RadrootsCoreCurrency = RadrootsCoreCurrency(*b"CAD");
    pub const AUD: RadrootsCoreCurrency = RadrootsCoreCurrency(*b"AUD");

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

impl fmt::Debug for RadrootsCoreCurrency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RadrootsCoreCurrency")
            .field(&self.as_str())
            .finish()
    }
}

impl fmt::Display for RadrootsCoreCurrency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for RadrootsCoreCurrency {
    type Error = RadrootsCoreCurrencyParseError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl FromStr for RadrootsCoreCurrency {
    type Err = RadrootsCoreCurrencyParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.len() != 3 || !s.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(RadrootsCoreCurrencyParseError::InvalidFormat);
        }
        let upper = s.to_ascii_uppercase();
        Self::from_str_upper(&upper)
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsCoreCurrencyParseError {
    InvalidFormat,
}

impl fmt::Display for RadrootsCoreCurrencyParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RadrootsCoreCurrencyParseError::InvalidFormat => {
                write!(f, "currency must be a 3-letter code")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsCoreCurrencyParseError {}

#[cfg(feature = "serde")]
impl Serialize for RadrootsCoreCurrency {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for RadrootsCoreCurrency {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = String::deserialize(de)?;
        s.parse().map_err(D::Error::custom)
    }
}

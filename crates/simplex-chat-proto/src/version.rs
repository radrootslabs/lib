use crate::error::RadrootsSimplexChatProtoError;
use alloc::string::{String, ToString};
use core::fmt;
use core::str::FromStr;

pub const RADROOTS_SIMPLEX_CHAT_INITIAL_VERSION: u16 = 1;
pub const RADROOTS_SIMPLEX_CHAT_COMPRESSION_VERSION: u16 = 8;
pub const RADROOTS_SIMPLEX_CHAT_CURRENT_VERSION: u16 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsSimplexChatVersionRange {
    pub min: u16,
    pub max: u16,
}

impl RadrootsSimplexChatVersionRange {
    pub const fn single(version: u16) -> Self {
        Self {
            min: version,
            max: version,
        }
    }

    pub fn new(min: u16, max: u16) -> Result<Self, RadrootsSimplexChatProtoError> {
        if min == 0 || max == 0 || min > max {
            return Err(RadrootsSimplexChatProtoError::InvalidVersionRange(
                alloc::format!("{min}-{max}"),
            ));
        }

        Ok(Self { min, max })
    }

    pub const fn supports_compression(&self) -> bool {
        self.max >= RADROOTS_SIMPLEX_CHAT_COMPRESSION_VERSION
    }
}

impl Default for RadrootsSimplexChatVersionRange {
    fn default() -> Self {
        Self::single(RADROOTS_SIMPLEX_CHAT_CURRENT_VERSION)
    }
}

impl fmt::Display for RadrootsSimplexChatVersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.min == self.max {
            write!(f, "{}", self.min)
        } else {
            write!(f, "{}-{}", self.min, self.max)
        }
    }
}

impl FromStr for RadrootsSimplexChatVersionRange {
    type Err = RadrootsSimplexChatProtoError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(RadrootsSimplexChatProtoError::InvalidVersionRange(
                String::new(),
            ));
        }

        if let Some((min, max)) = trimmed.split_once('-') {
            let min = parse_version(min, trimmed)?;
            let max = parse_version(max, trimmed)?;
            Self::new(min, max)
        } else {
            let version = parse_version(trimmed, trimmed)?;
            Self::new(version, version)
        }
    }
}

fn parse_version(value: &str, original: &str) -> Result<u16, RadrootsSimplexChatProtoError> {
    value
        .parse::<u16>()
        .map_err(|_| RadrootsSimplexChatProtoError::InvalidVersionRange(original.to_string()))
}

impl serde::Serialize for RadrootsSimplexChatVersionRange {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for RadrootsSimplexChatVersionRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_version() {
        let range = "8".parse::<RadrootsSimplexChatVersionRange>().unwrap();
        assert_eq!(range, RadrootsSimplexChatVersionRange::single(8));
        assert!(range.supports_compression());
    }

    #[test]
    fn parses_version_range() {
        let range = "1-16".parse::<RadrootsSimplexChatVersionRange>().unwrap();
        assert_eq!(range.min, 1);
        assert_eq!(range.max, 16);
        assert_eq!(range.to_string(), "1-16");
    }
}

use crate::error::RadrootsSimplexSmpProtoError;
use alloc::string::{String, ToString};
use core::fmt;
use core::str::FromStr;

pub const RADROOTS_SIMPLEX_SMP_INITIAL_CLIENT_VERSION: u16 = 1;
pub const RADROOTS_SIMPLEX_SMP_SERVER_HOSTNAMES_CLIENT_VERSION: u16 = 2;
pub const RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_CLIENT_VERSION: u16 = 3;
pub const RADROOTS_SIMPLEX_SMP_SHORT_LINKS_CLIENT_VERSION: u16 = 4;
pub const RADROOTS_SIMPLEX_SMP_CURRENT_CLIENT_VERSION: u16 =
    RADROOTS_SIMPLEX_SMP_SHORT_LINKS_CLIENT_VERSION;
pub const RADROOTS_SIMPLEX_SMP_INITIAL_TRANSPORT_VERSION: u16 = 6;
pub const RADROOTS_SIMPLEX_SMP_AUTH_COMMANDS_TRANSPORT_VERSION: u16 = 7;
pub const RADROOTS_SIMPLEX_SMP_SENDING_PROXY_TRANSPORT_VERSION: u16 = 8;
pub const RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_TRANSPORT_VERSION: u16 = 9;
pub const RADROOTS_SIMPLEX_SMP_DELETED_EVENT_TRANSPORT_VERSION: u16 = 10;
pub const RADROOTS_SIMPLEX_SMP_ENCRYPTED_BLOCK_TRANSPORT_VERSION: u16 = 11;
pub const RADROOTS_SIMPLEX_SMP_BLOCKED_ENTITY_TRANSPORT_VERSION: u16 = 12;
pub const RADROOTS_SIMPLEX_SMP_PROXY_SERVER_HANDSHAKE_TRANSPORT_VERSION: u16 = 14;
pub const RADROOTS_SIMPLEX_SMP_SHORT_LINKS_TRANSPORT_VERSION: u16 = 15;
pub const RADROOTS_SIMPLEX_SMP_SERVICE_CERTS_TRANSPORT_VERSION: u16 = 16;
pub const RADROOTS_SIMPLEX_SMP_NEW_NOTIFIER_CREDENTIALS_TRANSPORT_VERSION: u16 = 17;
pub const RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION: u16 =
    RADROOTS_SIMPLEX_SMP_NEW_NOTIFIER_CREDENTIALS_TRANSPORT_VERSION;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsSimplexSmpVersionRange {
    pub min: u16,
    pub max: u16,
}

impl RadrootsSimplexSmpVersionRange {
    pub const fn single(version: u16) -> Self {
        Self {
            min: version,
            max: version,
        }
    }

    pub fn new(min: u16, max: u16) -> Result<Self, RadrootsSimplexSmpProtoError> {
        if min == 0 || max == 0 || min > max {
            return Err(RadrootsSimplexSmpProtoError::InvalidVersionRange(
                alloc::format!("{min}-{max}"),
            ));
        }

        Ok(Self { min, max })
    }

    pub const fn contains(&self, version: u16) -> bool {
        version >= self.min && version <= self.max
    }
}

impl Default for RadrootsSimplexSmpVersionRange {
    fn default() -> Self {
        Self::single(RADROOTS_SIMPLEX_SMP_CURRENT_CLIENT_VERSION)
    }
}

impl fmt::Display for RadrootsSimplexSmpVersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.min == self.max {
            write!(f, "{}", self.min)
        } else {
            write!(f, "{}-{}", self.min, self.max)
        }
    }
}

impl FromStr for RadrootsSimplexSmpVersionRange {
    type Err = RadrootsSimplexSmpProtoError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(RadrootsSimplexSmpProtoError::InvalidVersionRange(
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

fn parse_version(value: &str, original: &str) -> Result<u16, RadrootsSimplexSmpProtoError> {
    value
        .parse::<u16>()
        .map_err(|_| RadrootsSimplexSmpProtoError::InvalidVersionRange(original.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_version_range() {
        let range = "9".parse::<RadrootsSimplexSmpVersionRange>().unwrap();
        assert_eq!(range, RadrootsSimplexSmpVersionRange::single(9));
        assert_eq!(range.to_string(), "9");
    }

    #[test]
    fn parses_bounded_version_range() {
        let range = "6-9".parse::<RadrootsSimplexSmpVersionRange>().unwrap();
        assert_eq!(range.min, 6);
        assert_eq!(range.max, 9);
        assert!(range.contains(7));
        assert_eq!(range.to_string(), "6-9");
    }
}

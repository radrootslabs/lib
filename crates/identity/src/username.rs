//! Canonical public usernames.
//!
//! [`Username`] trims surrounding whitespace, lowercases ASCII input, enforces
//! [`MIN_LENGTH`] and [`MAX_LENGTH`], and accepts only the documented public
//! character set and dot placement. Parsing and serde decoding use the same
//! validator and always produce canonical text.

use alloc::string::String;
use core::{fmt, str::FromStr};

use crate::Error;

/// Minimum canonical username length in ASCII bytes.
pub const MIN_LENGTH: usize = 3;

/// Maximum canonical username length in ASCII bytes.
pub const MAX_LENGTH: usize = 30;

/// A normalized public Radroots username.
///
/// Usernames are lowercase ASCII and may contain letters, digits, `.`, `_`,
/// and `-`. A dot cannot occur first, last, or consecutively. Parsing trims
/// surrounding whitespace and canonicalizes ASCII uppercase letters.
#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Username(String);

impl Username {
    /// Parses and normalizes a public username.
    pub fn parse(value: &str) -> Result<Self, Error> {
        let canonical = value.trim().to_ascii_lowercase();
        validate(&canonical)?;
        Ok(Self(canonical))
    }

    /// Borrows the canonical username text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the canonical username text.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

fn validate(value: &str) -> Result<(), Error> {
    let length = value.len();
    if !(MIN_LENGTH..=MAX_LENGTH).contains(&length) {
        return Err(Error::InvalidUsernameLength {
            min: MIN_LENGTH,
            max: MAX_LENGTH,
            actual: length,
        });
    }

    let bytes = value.as_bytes();
    if bytes.first() == Some(&b'.') || bytes.last() == Some(&b'.') {
        return Err(Error::InvalidUsernameDotPlacement);
    }

    let mut previous_dot = false;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if byte == b'.' {
            if previous_dot {
                return Err(Error::InvalidUsernameDotPlacement);
            }
            previous_dot = true;
            continue;
        }
        previous_dot = false;
        if !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')) {
            return Err(Error::InvalidUsernameCharacter { index });
        }
    }
    Ok(())
}

impl fmt::Debug for Username {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Username")
            .field(&self.as_str())
            .finish()
    }
}

impl fmt::Display for Username {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Username {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<&str> for Username {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for Username {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl AsRef<str> for Username {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Username {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Username {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct UsernameVisitor;

        impl serde::de::Visitor<'_> for UsernameVisitor {
            type Value = Username;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a valid public Radroots username")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Username::parse(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(UsernameVisitor)
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::*;

    #[test]
    fn usernames_normalize_to_one_canonical_form() {
        let username = Username::parse("  RadRoots.Test  ").unwrap();
        assert_eq!(username.as_str(), "radroots.test");
        assert_eq!(username.to_string(), "radroots.test");
        assert_eq!(Username::from_str("radroots.test").unwrap(), username);
    }

    #[test]
    fn usernames_reject_invalid_lengths_characters_and_dots() {
        assert!(matches!(
            Username::parse("rr"),
            Err(Error::InvalidUsernameLength { actual: 2, .. })
        ));
        assert!(matches!(
            Username::parse("rad roots"),
            Err(Error::InvalidUsernameCharacter { index: 3 })
        ));
        for value in [".radroots", "radroots.", "radroots..test"] {
            assert!(matches!(
                Username::parse(value),
                Err(Error::InvalidUsernameDotPlacement)
            ));
        }
        assert!(matches!(
            Username::parse("rädroots"),
            Err(Error::InvalidUsernameCharacter { index: 1 })
        ));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn username_serde_is_checked_and_canonical() {
        let username: Username = serde_json::from_str("\" RadRoots \"").unwrap();
        assert_eq!(username.as_str(), "radroots");
        assert_eq!(serde_json::to_string(&username).unwrap(), "\"radroots\"");
        assert!(serde_json::from_str::<Username>("\"rr\"").is_err());
    }
}

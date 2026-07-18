use alloc::string::{String, ToString};
use core::{fmt, str::FromStr};
use sha2::{Digest, Sha256};

use crate::RadrootsBlossomError;

const SHA256_BYTES: usize = 32;
const SHA256_HEX_LENGTH: usize = SHA256_BYTES * 2;
const LOWER_HEX: &[u8; 16] = b"0123456789abcdef";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsBlossomSha256([u8; SHA256_BYTES]);

impl RadrootsBlossomSha256 {
    pub fn digest(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        let mut value = [0_u8; SHA256_BYTES];
        value.copy_from_slice(&digest);
        Self(value)
    }

    pub fn from_hex(value: &str) -> Result<Self, RadrootsBlossomError> {
        if value.len() != SHA256_HEX_LENGTH {
            return Err(RadrootsBlossomError::InvalidSha256);
        }

        let bytes = value.as_bytes();
        let mut decoded = [0_u8; SHA256_BYTES];
        for (index, output) in decoded.iter_mut().enumerate() {
            let high = decode_nibble(bytes[index * 2])?;
            let low = decode_nibble(bytes[index * 2 + 1])?;
            *output = (high << 4) | low;
        }
        Ok(Self(decoded))
    }

    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        let mut output = String::with_capacity(SHA256_HEX_LENGTH);
        for byte in self.0 {
            output.push(char::from(LOWER_HEX[usize::from(byte >> 4)]));
            output.push(char::from(LOWER_HEX[usize::from(byte & 0x0f)]));
        }
        output
    }
}

impl fmt::Display for RadrootsBlossomSha256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl FromStr for RadrootsBlossomSha256 {
    type Err = RadrootsBlossomError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_hex(value)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for RadrootsBlossomSha256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsBlossomSha256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_hex(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsBlossomFileExtension(String);

impl RadrootsBlossomFileExtension {
    pub fn parse(value: &str) -> Result<Self, RadrootsBlossomError> {
        if value.is_empty()
            || value.split('.').any(str::is_empty)
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(RadrootsBlossomError::InvalidFileExtension);
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RadrootsBlossomFileExtension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RadrootsBlossomFileExtension {
    type Err = RadrootsBlossomError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for RadrootsBlossomFileExtension {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsBlossomFileExtension {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsBlossomHashPath {
    hash: RadrootsBlossomSha256,
    extension: Option<RadrootsBlossomFileExtension>,
}

impl RadrootsBlossomHashPath {
    pub fn new(
        hash: RadrootsBlossomSha256,
        extension: Option<RadrootsBlossomFileExtension>,
    ) -> Self {
        Self { hash, extension }
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsBlossomError> {
        let segment = value
            .strip_prefix('/')
            .ok_or(RadrootsBlossomError::InvalidHashPath)?;
        if segment.contains('/')
            || segment.contains('\\')
            || segment.contains('%')
            || segment.contains('?')
            || segment.contains('#')
        {
            return Err(RadrootsBlossomError::InvalidHashPath);
        }

        let hash = segment
            .get(..SHA256_HEX_LENGTH)
            .ok_or(RadrootsBlossomError::InvalidHashPath)
            .and_then(RadrootsBlossomSha256::from_hex)?;
        let suffix = &segment[SHA256_HEX_LENGTH..];
        let extension = if suffix.is_empty() {
            None
        } else {
            let value = suffix
                .strip_prefix('.')
                .ok_or(RadrootsBlossomError::InvalidHashPath)?;
            Some(RadrootsBlossomFileExtension::parse(value)?)
        };
        Ok(Self::new(hash, extension))
    }

    pub const fn hash(&self) -> RadrootsBlossomSha256 {
        self.hash
    }

    pub fn extension(&self) -> Option<&RadrootsBlossomFileExtension> {
        self.extension.as_ref()
    }

    pub fn to_path(&self) -> String {
        let mut value = String::with_capacity(
            1 + SHA256_HEX_LENGTH
                + self
                    .extension
                    .as_ref()
                    .map_or(0, |extension| 1 + extension.as_str().len()),
        );
        value.push('/');
        value.push_str(&self.hash.to_hex());
        if let Some(extension) = &self.extension {
            value.push('.');
            value.push_str(extension.as_str());
        }
        value
    }
}

impl fmt::Display for RadrootsBlossomHashPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_path())
    }
}

impl FromStr for RadrootsBlossomHashPath {
    type Err = RadrootsBlossomError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for RadrootsBlossomHashPath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_path())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsBlossomHashPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

const fn decode_nibble(byte: u8) -> Result<u8, RadrootsBlossomError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(RadrootsBlossomError::InvalidSha256),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn hash_round_trips_known_digest_and_serde() {
        let hash = RadrootsBlossomSha256::digest(b"");
        assert_eq!(hash.to_string(), EMPTY_SHA256);
        assert_eq!(hash.as_bytes().len(), SHA256_BYTES);
        assert_eq!(RadrootsBlossomSha256::from_str(EMPTY_SHA256), Ok(hash));
        let json = serde_json::to_string(&hash).unwrap();
        assert_eq!(
            serde_json::from_str::<RadrootsBlossomSha256>(&json).unwrap(),
            hash
        );
    }

    #[test]
    fn hash_rejects_wrong_length_case_and_alphabet() {
        for value in [
            "",
            "0",
            &"a".repeat(63),
            &"a".repeat(65),
            &"A".repeat(64),
            &"z".repeat(64),
        ] {
            assert_eq!(
                RadrootsBlossomSha256::from_hex(value),
                Err(RadrootsBlossomError::InvalidSha256)
            );
        }
        assert!(serde_json::from_str::<RadrootsBlossomSha256>("12").is_err());
    }

    #[test]
    fn extension_accepts_simple_and_compound_values() {
        for value in ["png", "tar.gz", "x-custom_2", "PNG"] {
            let extension = value.parse::<RadrootsBlossomFileExtension>().unwrap();
            assert_eq!(extension.as_str(), value);
            assert_eq!(extension.to_string(), value);
            let json = serde_json::to_string(&extension).unwrap();
            assert_eq!(
                serde_json::from_str::<RadrootsBlossomFileExtension>(&json).unwrap(),
                extension
            );
        }
    }

    #[test]
    fn extension_rejects_invalid_values() {
        for value in ["", ".png", "png.", "tar..gz", "a/b", "a b", "café"] {
            assert_eq!(
                RadrootsBlossomFileExtension::parse(value),
                Err(RadrootsBlossomError::InvalidFileExtension)
            );
        }
        assert!(serde_json::from_str::<RadrootsBlossomFileExtension>("false").is_err());
    }

    #[test]
    fn hash_path_round_trips_with_and_without_extension() {
        let bare = format!("/{EMPTY_SHA256}");
        let parsed = RadrootsBlossomHashPath::parse(&bare).unwrap();
        assert_eq!(parsed.hash().to_string(), EMPTY_SHA256);
        assert_eq!(parsed.extension(), None);
        assert_eq!(parsed.to_path(), bare);

        let extended = format!("/{EMPTY_SHA256}.webp");
        let parsed = RadrootsBlossomHashPath::from_str(&extended).unwrap();
        assert_eq!(parsed.extension().unwrap().as_str(), "webp");
        assert_eq!(parsed.to_string(), extended);
        let json = serde_json::to_string(&parsed).unwrap();
        assert_eq!(
            serde_json::from_str::<RadrootsBlossomHashPath>(&json).unwrap(),
            parsed
        );
    }

    #[test]
    fn hash_path_rejects_non_root_and_ambiguous_paths() {
        for path in [
            EMPTY_SHA256,
            "/",
            "/abc",
            &format!("/{EMPTY_SHA256}/x"),
            &format!("/{EMPTY_SHA256}%2epng"),
            &format!("/{EMPTY_SHA256}.png?download=1"),
            &format!("/{EMPTY_SHA256}.png#media"),
            &format!("/{EMPTY_SHA256}\\x"),
            &format!("/{EMPTY_SHA256}png"),
            &format!("/{EMPTY_SHA256}."),
        ] {
            assert!(RadrootsBlossomHashPath::parse(path).is_err(), "{path}");
        }
        assert!(serde_json::from_str::<RadrootsBlossomHashPath>("null").is_err());
    }
}

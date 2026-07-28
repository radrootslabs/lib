//! Canonical Blossom media-type values.

#[cfg(feature = "serde")]
use alloc::string::String;
use alloc::{collections::BTreeMap, vec::Vec};
use core::{fmt, str::FromStr};
use mediatype::{MediaTypeBuf, ReadParams};

use crate::Error;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MediaType(MediaTypeBuf);

impl MediaType {
    pub fn parse(value: &str) -> Result<Self, Error> {
        let parsed = MediaTypeBuf::from_str(value).map_err(|_| Error::InvalidMediaType)?;
        if parsed.as_str() != value || parsed.ty().as_str() == "*" || parsed.subty().as_str() == "*"
        {
            return Err(Error::InvalidMediaType);
        }
        let canonical = parsed.canonicalize();
        let mut unique_params = BTreeMap::new();
        for (name, value) in canonical.params() {
            if unique_params.insert(name, value).is_some() {
                return Err(Error::InvalidMediaType);
            }
        }
        let ordered_params = unique_params.into_iter().collect::<Vec<_>>();
        Ok(Self(MediaTypeBuf::from_parts(
            canonical.ty(),
            canonical.subty(),
            canonical.suffix(),
            &ordered_params,
        )))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for MediaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for MediaType {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for MediaType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for MediaType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

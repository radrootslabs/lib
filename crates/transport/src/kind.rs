use crate::RadrootsTransportError;
use alloc::string::{String, ToString};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsTransportKind {
    Nostr,
    Reticulum,
    Local,
}

impl RadrootsTransportKind {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsTransportError> {
        let canonical = value.as_ref().trim().to_ascii_lowercase();
        Self::from_canonical_str(canonical.as_str())
    }

    pub fn parse_canonical(value: impl AsRef<str>) -> Result<Self, RadrootsTransportError> {
        let raw = value.as_ref();
        if raw.is_empty() {
            return Err(RadrootsTransportError::EmptyTransportKind);
        }
        if raw != raw.trim() || raw != raw.to_ascii_lowercase() {
            return Err(RadrootsTransportError::InvalidTransportKind);
        }
        Self::from_canonical_str(raw)
    }

    fn from_canonical_str(canonical: &str) -> Result<Self, RadrootsTransportError> {
        match canonical {
            "nostr" => Ok(Self::Nostr),
            "reticulum" => Ok(Self::Reticulum),
            "local" => Ok(Self::Local),
            _ => Err(RadrootsTransportError::InvalidTransportKind),
        }
    }

    pub fn canonical_label(&self) -> String {
        match self {
            Self::Nostr => "nostr".to_string(),
            Self::Reticulum => "reticulum".to_string(),
            Self::Local => "local".to_string(),
        }
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for RadrootsTransportKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.canonical_label().as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsTransportKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::parse_canonical(value).map_err(serde::de::Error::custom)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsTransportImplementationState {
    Real,
    Mock,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsTransportCapabilityMaturity {
    Preview,
    Stable,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsTransportCapabilityAvailability {
    Available,
    Degraded,
    Unavailable,
}

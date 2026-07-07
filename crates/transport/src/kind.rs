use crate::RadrootsTransportError;
use alloc::string::{String, ToString};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsTransportKind {
    Nostr,
    Reticulum,
    Mesh,
    Local,
    Proxy,
    Custom(String),
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
            "mesh" => Ok(Self::Mesh),
            "local" => Ok(Self::Local),
            "proxy" => Ok(Self::Proxy),
            _ => Self::custom_canonical(canonical),
        }
    }

    pub fn custom(value: impl Into<String>) -> Result<Self, RadrootsTransportError> {
        let value = value.into();
        let canonical = value.trim().to_ascii_lowercase();
        Self::custom_canonical(canonical.as_str())
    }

    fn custom_canonical(canonical: &str) -> Result<Self, RadrootsTransportError> {
        if canonical.is_empty() {
            return Err(RadrootsTransportError::EmptyTransportKind);
        }
        if removed_first_party_kind(canonical) {
            return Err(RadrootsTransportError::InvalidTransportKind);
        }
        if canonical
            .chars()
            .any(|ch| ch.is_ascii_control() || ch.is_ascii_whitespace() || ch == ':' || ch == '/')
        {
            return Err(RadrootsTransportError::InvalidTransportKind);
        }
        Ok(Self::Custom(canonical.to_string()))
    }

    pub fn canonical_label(&self) -> String {
        match self {
            Self::Nostr => "nostr".to_string(),
            Self::Reticulum => "reticulum".to_string(),
            Self::Mesh => "mesh".to_string(),
            Self::Local => "local".to_string(),
            Self::Proxy => "proxy".to_string(),
            Self::Custom(value) => value.clone(),
        }
    }
}

fn removed_first_party_kind(canonical: &str) -> bool {
    const RADROOTSD_PROXY_PREFIX: &str = "radrootsd";
    const RADROOTSD_PROXY_SUFFIX: &str = "_proxy";
    canonical.len() == RADROOTSD_PROXY_PREFIX.len() + RADROOTSD_PROXY_SUFFIX.len()
        && canonical.starts_with(RADROOTSD_PROXY_PREFIX)
        && canonical.ends_with(RADROOTSD_PROXY_SUFFIX)
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsTransportImplementationState {
    Available,
    Disabled,
    Misconfigured,
    PreviewUnavailable,
}

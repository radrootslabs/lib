use crate::RadrootsTransportError;
use alloc::string::{String, ToString};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsTransportKind {
    Nostr,
    Reticulum,
    Mesh,
    Local,
    Custom(String),
}

impl RadrootsTransportKind {
    pub fn custom(value: impl Into<String>) -> Result<Self, RadrootsTransportError> {
        let value = value.into();
        let canonical = value.trim().to_ascii_lowercase();
        if canonical.is_empty() {
            return Err(RadrootsTransportError::EmptyTransportKind);
        }
        if canonical
            .chars()
            .any(|ch| ch.is_ascii_control() || ch.is_ascii_whitespace() || ch == ':' || ch == '/')
        {
            return Err(RadrootsTransportError::InvalidTransportKind);
        }
        Ok(Self::Custom(canonical))
    }

    pub fn canonical_label(&self) -> String {
        match self {
            Self::Nostr => "nostr".to_string(),
            Self::Reticulum => "reticulum".to_string(),
            Self::Mesh => "mesh".to_string(),
            Self::Local => "local".to_string(),
            Self::Custom(value) => value.clone(),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsTransportImplementationState {
    Available,
    Disabled,
    Misconfigured,
    PreviewUnavailable,
}

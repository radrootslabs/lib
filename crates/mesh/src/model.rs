use crate::RadrootsMeshError;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub const RADROOTS_MESH_FRAME_VERSION: u16 = 1;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsMeshScope {
    Local,
    Community,
    Custom(String),
}

impl RadrootsMeshScope {
    pub fn custom(value: impl Into<String>) -> Result<Self, RadrootsMeshError> {
        let value = value.into();
        let canonical = value.trim().to_ascii_lowercase();
        if canonical.is_empty() {
            return Err(RadrootsMeshError::EmptyCustomScope);
        }
        Ok(Self::Custom(canonical))
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Local => "local",
            Self::Community => "community",
            Self::Custom(value) => value.as_str(),
        }
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsMeshError> {
        match value {
            "local" => Ok(Self::Local),
            "community" => Ok(Self::Community),
            value if value.starts_with("custom:") => Self::custom(&value["custom:".len()..]),
            _ => Err(RadrootsMeshError::UnknownScope),
        }
    }

    pub fn cbor_label(&self) -> String {
        match self {
            Self::Custom(value) => {
                let mut label = String::from("custom:");
                label.push_str(value);
                label
            }
            _ => self.label().to_string(),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsMeshPayloadPolicy {
    PayloadTransmissionForbidden,
}

impl RadrootsMeshPayloadPolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::PayloadTransmissionForbidden => "payload-forbidden",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsMeshError> {
        match value {
            "payload-forbidden" => Ok(Self::PayloadTransmissionForbidden),
            _ => Err(RadrootsMeshError::UnknownPayloadPolicy),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsMeshEventHead {
    pub event_id: String,
    pub author: String,
    pub kind: u32,
    pub created_at: u64,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsMeshFrame {
    pub version: u16,
    pub scope: RadrootsMeshScope,
    pub payload_policy: RadrootsMeshPayloadPolicy,
    pub event_heads: Vec<RadrootsMeshEventHead>,
    pub payload: Option<Vec<u8>>,
}

impl RadrootsMeshFrame {
    pub fn new(scope: RadrootsMeshScope, event_heads: Vec<RadrootsMeshEventHead>) -> Self {
        Self {
            version: RADROOTS_MESH_FRAME_VERSION,
            scope,
            payload_policy: RadrootsMeshPayloadPolicy::PayloadTransmissionForbidden,
            event_heads,
            payload: None,
        }
    }

    pub fn validate(&self) -> Result<(), RadrootsMeshError> {
        if self.version != RADROOTS_MESH_FRAME_VERSION {
            return Err(RadrootsMeshError::UnsupportedVersion);
        }
        if self.payload.is_some() {
            return Err(RadrootsMeshError::PayloadTransmissionForbidden);
        }
        Ok(())
    }
}

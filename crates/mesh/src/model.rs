use crate::RadrootsMeshError;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use radroots_transport::{RadrootsTransportError, RadrootsTransportMeshScopeId};

pub const RADROOTS_MESH_FRAME_VERSION: u16 = 1;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsMeshFrameType {
    Hello,
    EventHeadAnnounce,
    EventRequest,
    EventChunk,
    EventAck,
    RouteProbe,
}

impl RadrootsMeshFrameType {
    pub fn code(self) -> u64 {
        match self {
            Self::Hello => 0,
            Self::EventHeadAnnounce => 1,
            Self::EventRequest => 2,
            Self::EventChunk => 3,
            Self::EventAck => 4,
            Self::RouteProbe => 5,
        }
    }

    pub fn parse_code(value: u64) -> Result<Self, RadrootsMeshError> {
        match value {
            0 => Ok(Self::Hello),
            1 => Ok(Self::EventHeadAnnounce),
            2 => Ok(Self::EventRequest),
            3 => Ok(Self::EventChunk),
            4 => Ok(Self::EventAck),
            5 => Ok(Self::RouteProbe),
            _ => Err(RadrootsMeshError::UnknownFrameType),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Hello => "hello",
            Self::EventHeadAnnounce => "event_head_announce",
            Self::EventRequest => "event_request",
            Self::EventChunk => "event_chunk",
            Self::EventAck => "event_ack",
            Self::RouteProbe => "route_probe",
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsMeshScope {
    Local,
    Community,
    Custom(String),
}

impl RadrootsMeshScope {
    pub fn custom(value: impl AsRef<str>) -> Result<Self, RadrootsMeshError> {
        let scope =
            RadrootsTransportMeshScopeId::parse(value.as_ref()).map_err(|err| match err {
                RadrootsTransportError::EmptyTargetScope => RadrootsMeshError::EmptyCustomScope,
                RadrootsTransportError::InvalidTargetScope => RadrootsMeshError::InvalidCustomScope,
                _ => RadrootsMeshError::InvalidCustomScope,
            })?;
        Ok(Self::Custom(scope.as_str().to_string()))
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsMeshPayload {
    EmptyMap,
    Bytes(Vec<u8>),
}

impl RadrootsMeshPayload {
    pub fn empty() -> Self {
        Self::EmptyMap
    }

    pub fn validate(&self) -> Result<(), RadrootsMeshError> {
        match self {
            Self::EmptyMap => Ok(()),
            Self::Bytes(_) => Err(RadrootsMeshError::PayloadTransmissionForbidden),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsMeshFrame {
    pub version: u16,
    pub frame_type: RadrootsMeshFrameType,
    pub scope_id: RadrootsMeshScope,
    pub message_id: String,
    pub created_at_ms: u64,
    pub ttl: u64,
    pub payload: RadrootsMeshPayload,
}

impl RadrootsMeshFrame {
    pub fn new(
        frame_type: RadrootsMeshFrameType,
        scope_id: RadrootsMeshScope,
        message_id: impl Into<String>,
        created_at_ms: u64,
        ttl: u64,
    ) -> Self {
        Self {
            version: RADROOTS_MESH_FRAME_VERSION,
            frame_type,
            scope_id,
            message_id: message_id.into(),
            created_at_ms,
            ttl,
            payload: RadrootsMeshPayload::empty(),
        }
    }

    pub fn validate(&self) -> Result<(), RadrootsMeshError> {
        if self.version != RADROOTS_MESH_FRAME_VERSION {
            return Err(RadrootsMeshError::UnsupportedVersion);
        }
        if self.message_id.trim().is_empty() {
            return Err(RadrootsMeshError::EmptyMessageId);
        }
        if self.ttl == 0 {
            return Err(RadrootsMeshError::InvalidTtl);
        }
        self.payload.validate()
    }
}

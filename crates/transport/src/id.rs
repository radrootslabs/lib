use crate::RadrootsTransportError;
use alloc::string::{String, ToString};
use core::{fmt, str::FromStr};
use radroots_protocol::capability::v1::{
    Error as ProtocolError, MAX_TRANSPORT_KIND_BYTES, TransportKind as ProtocolTransportKind,
};

/// Maximum encoded length of a transport identity.
pub const TRANSPORT_ID_MAX_BYTES: usize = MAX_TRANSPORT_KIND_BYTES;

/// Validated, extensible transport identity.
///
/// Identities contain 1-64 canonical lowercase ASCII bytes. They begin and
/// end with a letter or digit and may use single `-` separators internally.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TransportId(ProtocolTransportKind);

impl TransportId {
    /// Process-local transport.
    pub const LOCAL: Self = Self(ProtocolTransportKind::LOCAL);
    /// Nostr relay transport.
    pub const NOSTR: Self = Self(ProtocolTransportKind::NOSTR);
    /// Reticulum mesh transport.
    pub const RETICULUM: Self = Self(ProtocolTransportKind::RETICULUM);
    /// Daemon-mediated transport.
    pub const RADROOTSD: Self = Self(ProtocolTransportKind::RADROOTSD);

    // Compatibility spellings retained until the planned consumer cutover.
    #[allow(non_upper_case_globals)]
    pub const Local: Self = Self::LOCAL;
    #[allow(non_upper_case_globals)]
    pub const Nostr: Self = Self::NOSTR;
    #[allow(non_upper_case_globals)]
    pub const Reticulum: Self = Self::RETICULUM;

    /// Parses an exact canonical identity.
    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsTransportError> {
        ProtocolTransportKind::parse(value.as_ref())
            .map(Self)
            .map_err(map_protocol_error)
    }

    /// Parses an exact canonical identity.
    pub fn parse_canonical(value: impl AsRef<str>) -> Result<Self, RadrootsTransportError> {
        Self::parse(value)
    }

    /// Returns the canonical identity text.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Returns an owned canonical identity for compatibility with predecessor APIs.
    pub fn canonical_label(&self) -> String {
        self.as_str().to_string()
    }
}

fn map_protocol_error(error: ProtocolError) -> RadrootsTransportError {
    match error {
        ProtocolError::EmptyTransportKind => RadrootsTransportError::EmptyTransportKind,
        ProtocolError::InvalidTransportKind { .. } => RadrootsTransportError::InvalidTransportKind,
        _ => RadrootsTransportError::InvalidTransportKind,
    }
}

impl AsRef<str> for TransportId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for TransportId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for TransportId {
    type Err = RadrootsTransportError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<&str> for TransportId {
    type Error = RadrootsTransportError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for TransportId {
    type Error = RadrootsTransportError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<ProtocolTransportKind> for TransportId {
    fn from(value: ProtocolTransportKind) -> Self {
        Self(value)
    }
}

impl From<TransportId> for ProtocolTransportKind {
    fn from(value: TransportId) -> Self {
        value.0
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for TransportId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for TransportId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        <ProtocolTransportKind as serde::Deserialize>::deserialize(deserializer).map(Self)
    }
}

/// Compatibility name retained until the planned workspace consumer cutover.
#[doc(hidden)]
pub type RadrootsTransportKind = TransportId;

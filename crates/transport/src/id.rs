use crate::Error as TransportError;
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

    /// Parses an exact canonical identity.
    pub fn parse(value: impl AsRef<str>) -> Result<Self, TransportError> {
        ProtocolTransportKind::parse(value.as_ref())
            .map(Self)
            .map_err(map_protocol_error)
    }

    /// Parses an exact canonical identity.
    pub fn parse_canonical(value: impl AsRef<str>) -> Result<Self, TransportError> {
        Self::parse(value)
    }

    /// Returns the canonical identity text.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Returns an owned canonical identity label.
    pub fn canonical_label(&self) -> String {
        self.as_str().to_string()
    }
}

fn map_protocol_error(error: ProtocolError) -> TransportError {
    match error {
        ProtocolError::EmptyTransportKind => TransportError::EmptyTransportKind,
        ProtocolError::InvalidTransportKind { .. } => TransportError::InvalidTransportKind,
        _ => TransportError::InvalidTransportKind,
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
    type Err = TransportError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<&str> for TransportId {
    type Error = TransportError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for TransportId {
    type Error = TransportError;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_ids_cover_conversion_and_validation_surfaces() {
        let id = TransportId::parse_canonical("custom-transport").unwrap();
        assert_eq!(id.as_str(), "custom-transport");
        assert_eq!(id.as_ref(), "custom-transport");
        assert_eq!(id.to_string(), "custom-transport");
        assert_eq!(id.canonical_label(), "custom-transport");
        assert_eq!(TransportId::from_str("custom-transport").unwrap(), id);
        assert_eq!(TransportId::try_from("custom-transport").unwrap(), id);
        assert_eq!(
            TransportId::try_from(String::from("custom-transport")).unwrap(),
            id
        );
        let protocol_id: ProtocolTransportKind = id.into();
        assert_eq!(TransportId::from(protocol_id), id);
        assert_eq!(
            TransportId::parse(""),
            Err(TransportError::EmptyTransportKind)
        );
        assert_eq!(
            TransportId::parse("Invalid"),
            Err(TransportError::InvalidTransportKind)
        );

        #[cfg(feature = "serde")]
        {
            let encoded = serde_json::to_string(&id).unwrap();
            assert_eq!(serde_json::from_str::<TransportId>(&encoded).unwrap(), id);
            assert!(serde_json::from_str::<TransportId>("\"Invalid\"").is_err());
        }
    }
}

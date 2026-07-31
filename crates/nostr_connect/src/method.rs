use crate::error::RadrootsNostrConnectError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// Maximum UTF-8 byte length of a NIP-46 method identifier.
pub const METHOD_MAX_BYTES: usize = 64;

/// A validated extension method identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CustomMethod(String);

impl CustomMethod {
    fn new(value: String) -> Result<Self, RadrootsNostrConnectError> {
        validate_custom_method(&value)?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Method {
    Connect,
    GetPublicKey,
    GetSessionCapability,
    SignEvent,
    Nip04Encrypt,
    Nip04Decrypt,
    Nip44Encrypt,
    Nip44Decrypt,
    Ping,
    SwitchRelays,
    Logout,
    Custom(CustomMethod),
}

impl Method {
    /// Creates a bounded custom method identifier.
    pub fn custom(value: impl Into<String>) -> Result<Self, RadrootsNostrConnectError> {
        CustomMethod::new(value.into()).map(Self::Custom)
    }

    /// Returns the canonical wire identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Connect => "connect",
            Self::GetPublicKey => "get_public_key",
            Self::GetSessionCapability => "get_session_capability",
            Self::SignEvent => "sign_event",
            Self::Nip04Encrypt => "nip04_encrypt",
            Self::Nip04Decrypt => "nip04_decrypt",
            Self::Nip44Encrypt => "nip44_encrypt",
            Self::Nip44Decrypt => "nip44_decrypt",
            Self::Ping => "ping",
            Self::SwitchRelays => "switch_relays",
            Self::Logout => "logout",
            Self::Custom(value) => value.as_str(),
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Method {
    type Err = RadrootsNostrConnectError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "connect" => Ok(Self::Connect),
            "get_public_key" => Ok(Self::GetPublicKey),
            "get_session_capability" => Ok(Self::GetSessionCapability),
            "sign_event" => Ok(Self::SignEvent),
            "nip04_encrypt" => Ok(Self::Nip04Encrypt),
            "nip04_decrypt" => Ok(Self::Nip04Decrypt),
            "nip44_encrypt" => Ok(Self::Nip44Encrypt),
            "nip44_decrypt" => Ok(Self::Nip44Decrypt),
            "ping" => Ok(Self::Ping),
            "switch_relays" => Ok(Self::SwitchRelays),
            "logout" => Ok(Self::Logout),
            other => Self::custom(other),
        }
    }
}

impl Serialize for Method {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Method {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_str(&value).map_err(serde::de::Error::custom)
    }
}

fn validate_custom_method(value: &str) -> Result<(), RadrootsNostrConnectError> {
    if value.is_empty()
        || value.len() > METHOD_MAX_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(RadrootsNostrConnectError::InvalidMethod(value.to_owned()));
    }
    Ok(())
}

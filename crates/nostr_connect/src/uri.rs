//! Validated `nostrconnect://` and `bunker://` URI models.

use crate::error::RadrootsNostrConnectError;
use crate::permission::Permissions;
use radroots_identity::PublicKey;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use url::Url;

pub const URI_SCHEME: &str = "nostrconnect";
pub const BUNKER_URI_SCHEME: &str = "bunker";
pub const CLIENT_NAME_MAX_BYTES: usize = 128;
pub const CLIENT_URL_MAX_BYTES: usize = 2_048;
pub const CLIENT_METADATA_JSON_MAX_BYTES: usize = 4_352;
pub const URI_MAX_BYTES: usize = 16_384;
pub const RELAY_COUNT_MAX: usize = 32;
pub const SECRET_MAX_BYTES: usize = 1_024;

/// A validated WebSocket relay URL owned by the NIP-46 protocol package.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RelayUrl(nostr::RelayUrl);

impl RelayUrl {
    /// Parses a relay URL accepted by the underlying Nostr wire protocol.
    pub fn parse(value: &str) -> Result<Self, RadrootsNostrConnectError> {
        nostr::RelayUrl::parse(value).map(Self).map_err(|error| {
            RadrootsNostrConnectError::InvalidRelayUrl {
                value: value.to_owned(),
                reason: error.to_string(),
            }
        })
    }
}

impl fmt::Debug for RelayUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("RelayUrl").field(&self.0).finish()
    }
}

impl fmt::Display for RelayUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for RelayUrl {
    type Err = RadrootsNostrConnectError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Serialize for RelayUrl {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for RelayUrl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct BunkerUri {
    remote_signer_public_key: PublicKey,
    relays: Vec<RelayUrl>,
    secret: Option<String>,
}

impl BunkerUri {
    #[must_use]
    pub const fn remote_signer_public_key(&self) -> PublicKey {
        self.remote_signer_public_key
    }

    #[must_use]
    pub fn relays(&self) -> &[RelayUrl] {
        &self.relays
    }

    #[must_use]
    pub fn secret(&self) -> Option<&str> {
        self.secret.as_deref()
    }
}

impl fmt::Debug for BunkerUri {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BunkerUri")
            .field("remote_signer_public_key", &self.remote_signer_public_key)
            .field("relays", &self.relays)
            .field("secret", &self.secret.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientMetadata {
    #[doc(hidden)]
    pub requested_permissions: Permissions,
    #[doc(hidden)]
    pub name: Option<String>,
    #[doc(hidden)]
    pub url: Option<String>,
    #[doc(hidden)]
    pub image: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClientMetadataSerde {
    #[serde(default, skip_serializing_if = "Permissions::is_empty")]
    requested_permissions: Permissions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    image: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct ClientMetadataWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    image: Option<String>,
}

impl ClientMetadata {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn requested_permissions(&self) -> &Permissions {
        &self.requested_permissions
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    #[must_use]
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    #[must_use]
    pub fn image(&self) -> Option<&str> {
        self.image.as_deref()
    }

    #[must_use]
    pub fn with_requested_permissions(mut self, permissions: Permissions) -> Self {
        self.requested_permissions = permissions;
        self
    }

    pub fn with_name(
        mut self,
        value: impl Into<String>,
    ) -> Result<Self, RadrootsNostrConnectError> {
        self.name = Some(normalize_client_name(&value.into())?);
        Ok(self)
    }

    pub fn with_url(mut self, value: impl Into<String>) -> Result<Self, RadrootsNostrConnectError> {
        self.url = Some(normalize_client_url("url", &value.into())?);
        Ok(self)
    }

    pub fn with_image(
        mut self,
        value: impl Into<String>,
    ) -> Result<Self, RadrootsNostrConnectError> {
        self.image = Some(normalize_client_url("image", &value.into())?);
        Ok(self)
    }

    pub fn normalized(self) -> Result<Self, RadrootsNostrConnectError> {
        Ok(Self {
            requested_permissions: self.requested_permissions,
            name: self
                .name
                .map(|value| normalize_client_name(&value))
                .transpose()?,
            url: self
                .url
                .map(|value| normalize_client_url("url", &value))
                .transpose()?,
            image: self
                .image
                .map(|value| normalize_client_url("image", &value))
                .transpose()?,
        })
    }

    pub fn to_connect_param(&self) -> Result<String, RadrootsNostrConnectError> {
        let normalized = self.clone().normalized()?;
        let wire = ClientMetadataWire {
            name: normalized.name,
            url: normalized.url,
            image: normalized.image,
        };
        let value = serde_json::to_string(&wire)?;
        validate_metadata_size(&value)?;
        Ok(value)
    }

    pub fn from_connect_param(value: &str) -> Result<Self, RadrootsNostrConnectError> {
        validate_metadata_size(value)?;
        let wire: ClientMetadataWire = serde_json::from_str(value).map_err(|error| {
            RadrootsNostrConnectError::InvalidClientMetadata {
                field: "payload",
                reason: error.to_string(),
            }
        })?;
        Self {
            requested_permissions: Permissions::default(),
            name: wire.name,
            url: wire.url,
            image: wire.image,
        }
        .normalized()
    }

    pub fn is_display_empty(&self) -> bool {
        self.name.is_none() && self.url.is_none() && self.image.is_none()
    }
}

impl Serialize for ClientMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let normalized = self
            .clone()
            .normalized()
            .map_err(serde::ser::Error::custom)?;
        ClientMetadataSerde {
            requested_permissions: normalized.requested_permissions,
            name: normalized.name,
            url: normalized.url,
            image: normalized.image,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ClientMetadata {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let metadata = ClientMetadataSerde::deserialize(deserializer)?;
        Self {
            requested_permissions: metadata.requested_permissions,
            name: metadata.name,
            url: metadata.url,
            image: metadata.image,
        }
        .normalized()
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ClientUri {
    client_public_key: PublicKey,
    relays: Vec<RelayUrl>,
    secret: String,
    metadata: ClientMetadata,
}

impl ClientUri {
    #[must_use]
    pub const fn client_public_key(&self) -> PublicKey {
        self.client_public_key
    }

    #[must_use]
    pub fn relays(&self) -> &[RelayUrl] {
        &self.relays
    }

    #[must_use]
    pub fn secret(&self) -> &str {
        &self.secret
    }

    #[must_use]
    pub fn metadata(&self) -> &ClientMetadata {
        &self.metadata
    }
}

impl fmt::Debug for ClientUri {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClientUri")
            .field("client_public_key", &self.client_public_key)
            .field("relays", &self.relays)
            .field("secret", &"[redacted]")
            .field("metadata", &self.metadata)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Uri {
    Bunker(BunkerUri),
    Client(ClientUri),
}

impl Uri {
    pub fn parse(value: &str) -> Result<Self, RadrootsNostrConnectError> {
        if value.len() > URI_MAX_BYTES {
            return Err(RadrootsNostrConnectError::InvalidUri);
        }
        let url = Url::parse(value).map_err(|error| RadrootsNostrConnectError::InvalidUrl {
            value: "[redacted NIP-46 URI]".to_owned(),
            reason: error.to_string(),
        })?;
        let host = url
            .host_str()
            .ok_or(RadrootsNostrConnectError::MissingPublicKey)?;

        match url.scheme() {
            BUNKER_URI_SCHEME => {
                let remote_signer_public_key = parse_public_key(host)?;
                let mut relays = Vec::new();
                let mut secret = None;

                for (key, value) in url.query_pairs() {
                    match key.as_ref() {
                        "relay" => push_relay(&mut relays, value.as_ref())?,
                        "secret" => set_once(&mut secret, value.into_owned())?,
                        _ => {}
                    }
                }

                if relays.is_empty() {
                    return Err(RadrootsNostrConnectError::MissingRelay);
                }

                validate_optional_secret(secret.as_deref())?;
                Ok(Self::Bunker(BunkerUri {
                    remote_signer_public_key,
                    relays,
                    secret,
                }))
            }
            URI_SCHEME => {
                let client_public_key = parse_public_key(host)?;
                let mut relays = Vec::new();
                let mut secret = None;
                let mut metadata = ClientMetadata::default();
                let mut permissions_seen = false;

                for (key, value) in url.query_pairs() {
                    match key.as_ref() {
                        "relay" => push_relay(&mut relays, value.as_ref())?,
                        "secret" => set_once(&mut secret, value.into_owned())?,
                        "perms" => {
                            if permissions_seen {
                                return Err(RadrootsNostrConnectError::InvalidUri);
                            }
                            permissions_seen = true;
                            metadata.requested_permissions = Permissions::from_str(value.as_ref())?;
                        }
                        "name" => set_once(&mut metadata.name, value.into_owned())?,
                        "url" => set_once(&mut metadata.url, value.into_owned())?,
                        "image" => set_once(&mut metadata.image, value.into_owned())?,
                        _ => {}
                    }
                }

                if relays.is_empty() {
                    return Err(RadrootsNostrConnectError::MissingRelay);
                }

                let secret = secret.ok_or(RadrootsNostrConnectError::MissingSecret)?;
                if secret.is_empty() {
                    return Err(RadrootsNostrConnectError::MissingSecret);
                }
                validate_secret(&secret)?;
                let metadata = metadata.normalized()?;

                Ok(Self::Client(ClientUri {
                    client_public_key,
                    relays,
                    secret,
                    metadata,
                }))
            }
            scheme => Err(RadrootsNostrConnectError::InvalidUriScheme(
                scheme.to_owned(),
            )),
        }
    }
}

impl FromStr for Uri {
    type Err = RadrootsNostrConnectError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bunker(uri) => uri.fmt(f),
            Self::Client(uri) => uri.fmt(f),
        }
    }
}

impl fmt::Display for BunkerUri {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for relay in &self.relays {
            serializer.append_pair("relay", &relay.to_string());
        }
        if let Some(secret) = &self.secret {
            serializer.append_pair("secret", secret);
        }
        let query = serializer.finish();
        write!(
            formatter,
            "{BUNKER_URI_SCHEME}://{}?{query}",
            self.remote_signer_public_key
        )
    }
}

impl fmt::Display for ClientUri {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for relay in &self.relays {
            serializer.append_pair("relay", &relay.to_string());
        }
        serializer.append_pair("secret", &self.secret);
        if !self.metadata.requested_permissions.is_empty() {
            serializer.append_pair("perms", &self.metadata.requested_permissions.to_string());
        }
        if let Some(name) = &self.metadata.name {
            serializer.append_pair("name", name);
        }
        if let Some(url) = &self.metadata.url {
            serializer.append_pair("url", url);
        }
        if let Some(image) = &self.metadata.image {
            serializer.append_pair("image", image);
        }
        let query = serializer.finish();
        write!(
            formatter,
            "{URI_SCHEME}://{}?{query}",
            self.client_public_key
        )
    }
}

macro_rules! impl_uri_serde {
    ($type:ty, $variant:path) => {
        impl Serialize for $type {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $type {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                match Uri::parse(&value).map_err(serde::de::Error::custom)? {
                    $variant(uri) => Ok(uri),
                    _ => Err(serde::de::Error::custom("unexpected NIP-46 URI scheme")),
                }
            }
        }
    };
}

impl_uri_serde!(BunkerUri, Uri::Bunker);
impl_uri_serde!(ClientUri, Uri::Client);

impl Serialize for Uri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Uri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

fn parse_public_key(value: &str) -> Result<PublicKey, RadrootsNostrConnectError> {
    radroots_nostr::key::parse_public_key(value).map_err(|error| {
        RadrootsNostrConnectError::InvalidPublicKey {
            value: value.to_owned(),
            reason: error.to_string(),
        }
    })
}

fn parse_relay_url(value: &str) -> Result<RelayUrl, RadrootsNostrConnectError> {
    RelayUrl::parse(value)
}

fn push_relay(relays: &mut Vec<RelayUrl>, value: &str) -> Result<(), RadrootsNostrConnectError> {
    let relay = parse_relay_url(value)?;
    if relays.contains(&relay) {
        return Ok(());
    }
    if relays.len() == RELAY_COUNT_MAX {
        return Err(RadrootsNostrConnectError::InvalidUri);
    }
    relays.push(relay);
    Ok(())
}

fn set_once(slot: &mut Option<String>, value: String) -> Result<(), RadrootsNostrConnectError> {
    if slot.replace(value).is_some() {
        return Err(RadrootsNostrConnectError::InvalidUri);
    }
    Ok(())
}

fn validate_optional_secret(secret: Option<&str>) -> Result<(), RadrootsNostrConnectError> {
    match secret {
        Some("") => Err(RadrootsNostrConnectError::InvalidUri),
        Some(secret) => validate_secret(secret),
        None => Ok(()),
    }
}

fn validate_secret(secret: &str) -> Result<(), RadrootsNostrConnectError> {
    if secret.len() > SECRET_MAX_BYTES || secret.chars().any(char::is_control) {
        return Err(RadrootsNostrConnectError::InvalidUri);
    }
    Ok(())
}

fn normalize_client_name(value: &str) -> Result<String, RadrootsNostrConnectError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(invalid_client_metadata("name", "must not be empty"));
    }
    if normalized.len() > CLIENT_NAME_MAX_BYTES {
        return Err(invalid_client_metadata(
            "name",
            format!("must not exceed {CLIENT_NAME_MAX_BYTES} UTF-8 bytes"),
        ));
    }
    if normalized.chars().any(char::is_control) {
        return Err(invalid_client_metadata(
            "name",
            "must not contain control characters",
        ));
    }
    Ok(normalized.to_owned())
}

fn normalize_client_url(
    field: &'static str,
    value: &str,
) -> Result<String, RadrootsNostrConnectError> {
    if value.len() > CLIENT_URL_MAX_BYTES {
        return Err(invalid_client_metadata(
            field,
            format!("must not exceed {CLIENT_URL_MAX_BYTES} UTF-8 bytes"),
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(invalid_client_metadata(
            field,
            "must not contain control characters",
        ));
    }
    let parsed =
        Url::parse(value).map_err(|error| invalid_client_metadata(field, error.to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(invalid_client_metadata(
            field,
            "must use the http or https scheme",
        ));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(invalid_client_metadata(
            field,
            "must not contain credentials",
        ));
    }
    Ok(parsed.to_string())
}

fn validate_metadata_size(value: &str) -> Result<(), RadrootsNostrConnectError> {
    let received = value.len();
    if received > CLIENT_METADATA_JSON_MAX_BYTES {
        return Err(RadrootsNostrConnectError::ClientMetadataTooLarge {
            max: CLIENT_METADATA_JSON_MAX_BYTES,
            received,
        });
    }
    Ok(())
}

fn invalid_client_metadata(
    field: &'static str,
    reason: impl Into<String>,
) -> RadrootsNostrConnectError {
    RadrootsNostrConnectError::InvalidClientMetadata {
        field,
        reason: reason.into(),
    }
}

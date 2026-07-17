use crate::error::RadrootsNostrConnectError;
use crate::permission::RadrootsNostrConnectPermissions;
use nostr::{PublicKey, RelayUrl};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use url::Url;

pub const RADROOTS_NOSTR_CONNECT_URI_SCHEME: &str = "nostrconnect";
pub const RADROOTS_NOSTR_CONNECT_BUNKER_URI_SCHEME: &str = "bunker";
pub const RADROOTS_NOSTR_CONNECT_CLIENT_NAME_MAX_BYTES: usize = 128;
pub const RADROOTS_NOSTR_CONNECT_CLIENT_URL_MAX_BYTES: usize = 2_048;
pub const RADROOTS_NOSTR_CONNECT_CLIENT_METADATA_JSON_MAX_BYTES: usize = 4_352;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsNostrConnectBunkerUri {
    pub remote_signer_public_key: PublicKey,
    pub relays: Vec<RelayUrl>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsNostrConnectClientMetadata {
    #[serde(
        default,
        skip_serializing_if = "RadrootsNostrConnectPermissions::is_empty"
    )]
    pub requested_permissions: RadrootsNostrConnectPermissions,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct RadrootsNostrConnectClientMetadataWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    image: Option<String>,
}

impl RadrootsNostrConnectClientMetadata {
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
        let wire = RadrootsNostrConnectClientMetadataWire {
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
        let wire: RadrootsNostrConnectClientMetadataWire =
            serde_json::from_str(value).map_err(|error| {
                RadrootsNostrConnectError::InvalidClientMetadata {
                    field: "payload",
                    reason: error.to_string(),
                }
            })?;
        Self {
            requested_permissions: RadrootsNostrConnectPermissions::default(),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsNostrConnectClientUri {
    pub client_public_key: PublicKey,
    pub relays: Vec<RelayUrl>,
    pub secret: String,
    #[serde(default)]
    pub metadata: RadrootsNostrConnectClientMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsNostrConnectUri {
    Bunker(RadrootsNostrConnectBunkerUri),
    Client(RadrootsNostrConnectClientUri),
}

impl RadrootsNostrConnectUri {
    pub fn parse(value: &str) -> Result<Self, RadrootsNostrConnectError> {
        let url = Url::parse(value).map_err(|error| RadrootsNostrConnectError::InvalidUrl {
            value: value.to_owned(),
            reason: error.to_string(),
        })?;
        let host = url
            .host_str()
            .ok_or(RadrootsNostrConnectError::MissingPublicKey)?;

        match url.scheme() {
            RADROOTS_NOSTR_CONNECT_BUNKER_URI_SCHEME => {
                let remote_signer_public_key = parse_public_key(host)?;
                let mut relays = Vec::new();
                let mut secret = None;

                for (key, value) in url.query_pairs() {
                    match key.as_ref() {
                        "relay" => relays.push(parse_relay_url(value.as_ref())?),
                        "secret" => secret = Some(value.into_owned()),
                        _ => {}
                    }
                }

                if relays.is_empty() {
                    return Err(RadrootsNostrConnectError::MissingRelay);
                }

                Ok(Self::Bunker(RadrootsNostrConnectBunkerUri {
                    remote_signer_public_key,
                    relays,
                    secret,
                }))
            }
            RADROOTS_NOSTR_CONNECT_URI_SCHEME => {
                let client_public_key = parse_public_key(host)?;
                let mut relays = Vec::new();
                let mut secret = None;
                let mut metadata = RadrootsNostrConnectClientMetadata::default();

                for (key, value) in url.query_pairs() {
                    match key.as_ref() {
                        "relay" => relays.push(parse_relay_url(value.as_ref())?),
                        "secret" => secret = Some(value.into_owned()),
                        "perms" => {
                            metadata.requested_permissions =
                                RadrootsNostrConnectPermissions::from_str(value.as_ref())?;
                        }
                        "name" => metadata.name = Some(value.into_owned()),
                        "url" => metadata.url = Some(value.into_owned()),
                        "image" => metadata.image = Some(value.into_owned()),
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
                let metadata = metadata.normalized()?;

                Ok(Self::Client(RadrootsNostrConnectClientUri {
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

impl FromStr for RadrootsNostrConnectUri {
    type Err = RadrootsNostrConnectError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl fmt::Display for RadrootsNostrConnectUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bunker(uri) => {
                let mut serializer = url::form_urlencoded::Serializer::new(String::new());
                for relay in &uri.relays {
                    serializer.append_pair("relay", &relay.to_string());
                }
                if let Some(secret) = &uri.secret {
                    serializer.append_pair("secret", secret);
                }
                let query = serializer.finish();
                write!(
                    f,
                    "{RADROOTS_NOSTR_CONNECT_BUNKER_URI_SCHEME}://{}?{query}",
                    uri.remote_signer_public_key
                )
            }
            Self::Client(uri) => {
                let mut serializer = url::form_urlencoded::Serializer::new(String::new());
                for relay in &uri.relays {
                    serializer.append_pair("relay", &relay.to_string());
                }
                serializer.append_pair("secret", &uri.secret);
                if !uri.metadata.requested_permissions.is_empty() {
                    serializer
                        .append_pair("perms", &uri.metadata.requested_permissions.to_string());
                }
                if let Some(name) = &uri.metadata.name {
                    serializer.append_pair("name", name);
                }
                if let Some(url) = &uri.metadata.url {
                    serializer.append_pair("url", url);
                }
                if let Some(image) = &uri.metadata.image {
                    serializer.append_pair("image", image);
                }
                let query = serializer.finish();
                write!(
                    f,
                    "{RADROOTS_NOSTR_CONNECT_URI_SCHEME}://{}?{query}",
                    uri.client_public_key
                )
            }
        }
    }
}

fn parse_public_key(value: &str) -> Result<PublicKey, RadrootsNostrConnectError> {
    PublicKey::parse(value)
        .or_else(|_| PublicKey::from_hex(value))
        .map_err(|error| RadrootsNostrConnectError::InvalidPublicKey {
            value: value.to_owned(),
            reason: error.to_string(),
        })
}

fn parse_relay_url(value: &str) -> Result<RelayUrl, RadrootsNostrConnectError> {
    RelayUrl::parse(value).map_err(|error| RadrootsNostrConnectError::InvalidRelayUrl {
        value: value.to_owned(),
        reason: error.to_string(),
    })
}

fn normalize_client_name(value: &str) -> Result<String, RadrootsNostrConnectError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(invalid_client_metadata("name", "must not be empty"));
    }
    if normalized.len() > RADROOTS_NOSTR_CONNECT_CLIENT_NAME_MAX_BYTES {
        return Err(invalid_client_metadata(
            "name",
            format!("must not exceed {RADROOTS_NOSTR_CONNECT_CLIENT_NAME_MAX_BYTES} UTF-8 bytes"),
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
    if value.len() > RADROOTS_NOSTR_CONNECT_CLIENT_URL_MAX_BYTES {
        return Err(invalid_client_metadata(
            field,
            format!("must not exceed {RADROOTS_NOSTR_CONNECT_CLIENT_URL_MAX_BYTES} UTF-8 bytes"),
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
    if received > RADROOTS_NOSTR_CONNECT_CLIENT_METADATA_JSON_MAX_BYTES {
        return Err(RadrootsNostrConnectError::ClientMetadataTooLarge {
            max: RADROOTS_NOSTR_CONNECT_CLIENT_METADATA_JSON_MAX_BYTES,
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

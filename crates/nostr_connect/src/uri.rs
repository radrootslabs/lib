use crate::error::RadrootsNostrConnectError;
use crate::permission::RadrootsNostrConnectPermissions;
use nostr::{PublicKey, RelayUrl};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use url::Url;

pub const RADROOTS_NOSTR_CONNECT_URI_SCHEME: &str = "nostrconnect";
pub const RADROOTS_NOSTR_CONNECT_BUNKER_URI_SCHEME: &str = "bunker";

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
                        "url" => metadata.url = Some(validate_url(value.as_ref())?),
                        "image" => metadata.image = Some(validate_url(value.as_ref())?),
                        _ => {}
                    }
                }

                if relays.is_empty() {
                    return Err(RadrootsNostrConnectError::MissingRelay);
                }

                let secret = secret.ok_or(RadrootsNostrConnectError::MissingSecret)?;

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

fn validate_url(value: &str) -> Result<String, RadrootsNostrConnectError> {
    Url::parse(value)
        .map(|url| url.to_string())
        .map_err(|error| RadrootsNostrConnectError::InvalidUrl {
            value: value.to_owned(),
            reason: error.to_string(),
        })
}

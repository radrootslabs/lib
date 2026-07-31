use crate::error::RadrootsNostrConnectError;
use crate::method::Method;
use crate::permission::Permissions;
use crate::uri::ClientMetadata;
use nostr::{Event, JsonUtil, PublicKey, RelayUrl, UnsignedEvent};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Value, json};
use std::str::FromStr;
use url::Url;

pub const RADROOTS_NOSTR_CONNECT_RPC_KIND: u16 = 24_133;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsNostrConnectRemoteSessionCapability {
    pub user_public_key: PublicKey,
    pub relays: Vec<RelayUrl>,
    pub permissions: Permissions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsNostrConnectRequest {
    Connect {
        remote_signer_public_key: PublicKey,
        secret: Option<String>,
        requested_permissions: Permissions,
        client_metadata: Option<ClientMetadata>,
    },
    GetPublicKey,
    GetSessionCapability,
    SignEvent(UnsignedEvent),
    Nip04Encrypt {
        public_key: PublicKey,
        plaintext: String,
    },
    Nip04Decrypt {
        public_key: PublicKey,
        ciphertext: String,
    },
    Nip44Encrypt {
        public_key: PublicKey,
        plaintext: String,
    },
    Nip44Decrypt {
        public_key: PublicKey,
        ciphertext: String,
    },
    Ping,
    SwitchRelays,
    Logout,
    Custom {
        method: Method,
        params: Vec<String>,
    },
}

impl RadrootsNostrConnectRequest {
    pub fn method(&self) -> Method {
        match self {
            Self::Connect { .. } => Method::Connect,
            Self::GetPublicKey => Method::GetPublicKey,
            Self::GetSessionCapability => Method::GetSessionCapability,
            Self::SignEvent(_) => Method::SignEvent,
            Self::Nip04Encrypt { .. } => Method::Nip04Encrypt,
            Self::Nip04Decrypt { .. } => Method::Nip04Decrypt,
            Self::Nip44Encrypt { .. } => Method::Nip44Encrypt,
            Self::Nip44Decrypt { .. } => Method::Nip44Decrypt,
            Self::Ping => Method::Ping,
            Self::SwitchRelays => Method::SwitchRelays,
            Self::Logout => Method::Logout,
            Self::Custom { method, .. } => method.clone(),
        }
    }

    pub fn to_params(&self) -> Result<Vec<String>, RadrootsNostrConnectError> {
        let params = match self {
            Self::Connect {
                remote_signer_public_key,
                secret,
                requested_permissions,
                client_metadata,
            } => {
                let mut params = vec![remote_signer_public_key.to_hex()];
                let normalized_secret = secret.as_ref().filter(|value| !value.is_empty()).cloned();
                if normalized_secret.is_some()
                    || !requested_permissions.is_empty()
                    || client_metadata.is_some()
                {
                    params.push(normalized_secret.unwrap_or_default());
                }
                if !requested_permissions.is_empty() || client_metadata.is_some() {
                    params.push(requested_permissions.to_string());
                }
                if let Some(client_metadata) = client_metadata {
                    params.push(client_metadata.to_connect_param()?);
                }
                params
            }
            Self::GetPublicKey
            | Self::GetSessionCapability
            | Self::Ping
            | Self::SwitchRelays
            | Self::Logout => Vec::new(),
            Self::SignEvent(unsigned_event) => vec![unsigned_event.as_json()],
            Self::Nip04Encrypt {
                public_key,
                plaintext,
            }
            | Self::Nip44Encrypt {
                public_key,
                plaintext,
            } => vec![public_key.to_hex(), plaintext.clone()],
            Self::Nip04Decrypt {
                public_key,
                ciphertext,
            }
            | Self::Nip44Decrypt {
                public_key,
                ciphertext,
            } => vec![public_key.to_hex(), ciphertext.clone()],
            Self::Custom { params, .. } => params.clone(),
        };
        Ok(params)
    }

    pub fn from_parts(
        method: Method,
        params: Vec<String>,
    ) -> Result<Self, RadrootsNostrConnectError> {
        match method {
            Method::Connect => {
                if params.is_empty() || params.len() > 4 {
                    return Err(RadrootsNostrConnectError::InvalidParams {
                        method: method.to_string(),
                        expected: "1 to 4 params",
                        received: params.len(),
                    });
                }
                let remote_signer_public_key = parse_public_key(&params[0])?;
                let secret = params.get(1).cloned().filter(|value| !value.is_empty());
                let requested_permissions = match params.get(2) {
                    Some(value) => Permissions::from_str(value)?,
                    None => Permissions::default(),
                };
                let client_metadata = params
                    .get(3)
                    .map(|value| ClientMetadata::from_connect_param(value))
                    .transpose()?;
                Ok(Self::Connect {
                    remote_signer_public_key,
                    secret,
                    requested_permissions,
                    client_metadata,
                })
            }
            Method::GetPublicKey => {
                expect_param_count(&method, &params, 0)?;
                Ok(Self::GetPublicKey)
            }
            Method::GetSessionCapability => {
                expect_param_count(&method, &params, 0)?;
                Ok(Self::GetSessionCapability)
            }
            Method::SignEvent => {
                expect_param_count(&method, &params, 1)?;
                let unsigned_event = serde_json::from_str(&params[0]).map_err(|error| {
                    RadrootsNostrConnectError::InvalidRequestPayload {
                        method: method.to_string(),
                        reason: error.to_string(),
                    }
                })?;
                Ok(Self::SignEvent(unsigned_event))
            }
            Method::Nip04Encrypt => {
                expect_param_count(&method, &params, 2)?;
                Ok(Self::Nip04Encrypt {
                    public_key: parse_public_key(&params[0])?,
                    plaintext: params[1].clone(),
                })
            }
            Method::Nip04Decrypt => {
                expect_param_count(&method, &params, 2)?;
                Ok(Self::Nip04Decrypt {
                    public_key: parse_public_key(&params[0])?,
                    ciphertext: params[1].clone(),
                })
            }
            Method::Nip44Encrypt => {
                expect_param_count(&method, &params, 2)?;
                Ok(Self::Nip44Encrypt {
                    public_key: parse_public_key(&params[0])?,
                    plaintext: params[1].clone(),
                })
            }
            Method::Nip44Decrypt => {
                expect_param_count(&method, &params, 2)?;
                Ok(Self::Nip44Decrypt {
                    public_key: parse_public_key(&params[0])?,
                    ciphertext: params[1].clone(),
                })
            }
            Method::Ping => {
                expect_param_count(&method, &params, 0)?;
                Ok(Self::Ping)
            }
            Method::SwitchRelays => {
                expect_param_count(&method, &params, 0)?;
                Ok(Self::SwitchRelays)
            }
            Method::Logout => {
                expect_param_count(&method, &params, 0)?;
                Ok(Self::Logout)
            }
            custom => Ok(Self::Custom {
                method: custom,
                params,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsNostrConnectRequestMessage {
    pub id: String,
    pub request: RadrootsNostrConnectRequest,
}

impl RadrootsNostrConnectRequestMessage {
    pub fn new(id: impl Into<String>, request: RadrootsNostrConnectRequest) -> Self {
        Self {
            id: id.into(),
            request,
        }
    }

    fn into_raw(self) -> Result<RawRequestMessage, RadrootsNostrConnectError> {
        Ok(RawRequestMessage {
            id: self.id,
            method: self.request.method(),
            params: self.request.to_params()?,
        })
    }

    fn from_raw(raw: RawRequestMessage) -> Result<Self, RadrootsNostrConnectError> {
        Ok(Self {
            id: raw.id,
            request: RadrootsNostrConnectRequest::from_parts(raw.method, raw.params)?,
        })
    }
}

impl Serialize for RadrootsNostrConnectRequestMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.clone()
            .into_raw()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RadrootsNostrConnectRequestMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawRequestMessage::deserialize(deserializer)?;
        Self::from_raw(raw).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsNostrConnectResponseEnvelope {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub const RADROOTS_NOSTR_CONNECT_PENDING_CONNECTION_ERROR: &str = "connection is pending";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsNostrConnectPendingConnectionPollOutcome {
    PendingApproval,
    Approved(PublicKey),
    ApprovedCapability(RadrootsNostrConnectRemoteSessionCapability),
    Rejected { message: String },
    AuthChallenge { url: String },
    UnexpectedResponse { response: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsNostrConnectResponse {
    ConnectAcknowledged,
    ConnectSecretEcho(String),
    LogoutAcknowledged,
    PendingConnection,
    UserPublicKey(PublicKey),
    RemoteSessionCapability(RadrootsNostrConnectRemoteSessionCapability),
    SignedEvent(Event),
    Pong,
    Nip04Encrypt(String),
    Nip04Decrypt(String),
    Nip44Encrypt(String),
    Nip44Decrypt(String),
    RelayList(Vec<RelayUrl>),
    RelayListUnchanged,
    AuthUrl(String),
    Error {
        result: Option<Value>,
        error: String,
    },
    Custom {
        result: Option<Value>,
        error: Option<String>,
    },
}

impl RadrootsNostrConnectResponse {
    pub fn into_pending_connection_poll_outcome(
        self,
    ) -> RadrootsNostrConnectPendingConnectionPollOutcome {
        match self {
            Self::PendingConnection => {
                RadrootsNostrConnectPendingConnectionPollOutcome::PendingApproval
            }
            Self::UserPublicKey(public_key) => {
                RadrootsNostrConnectPendingConnectionPollOutcome::Approved(public_key)
            }
            Self::RemoteSessionCapability(capability) => {
                RadrootsNostrConnectPendingConnectionPollOutcome::ApprovedCapability(capability)
            }
            Self::Error { error, .. }
                if error == RADROOTS_NOSTR_CONNECT_PENDING_CONNECTION_ERROR =>
            {
                RadrootsNostrConnectPendingConnectionPollOutcome::PendingApproval
            }
            Self::Error { error, .. } => {
                RadrootsNostrConnectPendingConnectionPollOutcome::Rejected { message: error }
            }
            Self::AuthUrl(url) => {
                RadrootsNostrConnectPendingConnectionPollOutcome::AuthChallenge { url }
            }
            other => RadrootsNostrConnectPendingConnectionPollOutcome::UnexpectedResponse {
                response: format!("{other:?}"),
            },
        }
    }

    pub fn into_envelope(
        self,
        id: impl Into<String>,
    ) -> Result<RadrootsNostrConnectResponseEnvelope, RadrootsNostrConnectError> {
        let id = id.into();
        let envelope = match self {
            Self::ConnectAcknowledged | Self::LogoutAcknowledged => {
                RadrootsNostrConnectResponseEnvelope {
                    id,
                    result: Some(Value::String("ack".to_owned())),
                    error: None,
                }
            }
            Self::ConnectSecretEcho(secret) => RadrootsNostrConnectResponseEnvelope {
                id,
                result: Some(Value::String(secret)),
                error: None,
            },
            Self::PendingConnection => RadrootsNostrConnectResponseEnvelope {
                id,
                result: None,
                error: Some(RADROOTS_NOSTR_CONNECT_PENDING_CONNECTION_ERROR.to_owned()),
            },
            Self::UserPublicKey(public_key) => RadrootsNostrConnectResponseEnvelope {
                id,
                result: Some(Value::String(public_key.to_hex())),
                error: None,
            },
            Self::RemoteSessionCapability(capability) => RadrootsNostrConnectResponseEnvelope {
                id,
                result: Some(remote_session_capability_value(capability)),
                error: None,
            },
            Self::SignedEvent(event) => RadrootsNostrConnectResponseEnvelope {
                id,
                result: Some(Value::String(event.as_json())),
                error: None,
            },
            Self::Pong => RadrootsNostrConnectResponseEnvelope {
                id,
                result: Some(Value::String("pong".to_owned())),
                error: None,
            },
            Self::Nip04Encrypt(text)
            | Self::Nip04Decrypt(text)
            | Self::Nip44Encrypt(text)
            | Self::Nip44Decrypt(text) => RadrootsNostrConnectResponseEnvelope {
                id,
                result: Some(Value::String(text)),
                error: None,
            },
            Self::RelayList(relays) => {
                let relays = relays
                    .into_iter()
                    .map(|relay| relay.to_string())
                    .collect::<Vec<_>>();
                RadrootsNostrConnectResponseEnvelope {
                    id,
                    result: Some(Value::Array(
                        relays.into_iter().map(Value::String).collect(),
                    )),
                    error: None,
                }
            }
            Self::RelayListUnchanged => RadrootsNostrConnectResponseEnvelope {
                id,
                result: Some(Value::Null),
                error: None,
            },
            Self::AuthUrl(url) => {
                let normalized = validate_url(&url)?;
                RadrootsNostrConnectResponseEnvelope {
                    id,
                    result: Some(Value::String("auth_url".to_owned())),
                    error: Some(normalized),
                }
            }
            Self::Error { result, error } => RadrootsNostrConnectResponseEnvelope {
                id,
                result,
                error: Some(error),
            },
            Self::Custom { result, error } => {
                RadrootsNostrConnectResponseEnvelope { id, result, error }
            }
        };
        Ok(envelope)
    }

    pub fn from_envelope(
        method: &Method,
        envelope: RadrootsNostrConnectResponseEnvelope,
    ) -> Result<Self, RadrootsNostrConnectError> {
        if let (Some(Value::String(result)), Some(url)) = (&envelope.result, &envelope.error)
            && result == "auth_url"
        {
            return Ok(Self::AuthUrl(validate_url(url)?));
        }

        if let Some(error) = envelope.error {
            if matches!(method, Method::GetPublicKey | Method::GetSessionCapability)
                && envelope.result.is_none()
                && error == RADROOTS_NOSTR_CONNECT_PENDING_CONNECTION_ERROR
            {
                return Ok(Self::PendingConnection);
            }
            if let Method::Custom(_) = method {
                return Ok(Self::Custom {
                    result: envelope.result,
                    error: Some(error),
                });
            }
            return Ok(Self::Error {
                result: envelope.result,
                error,
            });
        }

        match method {
            Method::Connect => {
                let result = expect_string_result(method, envelope.result)?;
                if result == "ack" {
                    Ok(Self::ConnectAcknowledged)
                } else {
                    Ok(Self::ConnectSecretEcho(result))
                }
            }
            Method::GetPublicKey => {
                let result = expect_string_result(method, envelope.result)?;
                Ok(Self::UserPublicKey(parse_public_key(&result)?))
            }
            Method::GetSessionCapability => {
                let capability = parse_json_string_result(method, envelope.result)?;
                Ok(Self::RemoteSessionCapability(capability))
            }
            Method::SignEvent => {
                let event = parse_json_string_result::<Event>(method, envelope.result)?;
                Ok(Self::SignedEvent(event))
            }
            Method::Ping => {
                let result = expect_string_result(method, envelope.result)?;
                if result != "pong" {
                    return Err(RadrootsNostrConnectError::InvalidResponsePayload {
                        method: method.to_string(),
                        reason: format!("expected `pong`, got `{result}`"),
                    });
                }
                Ok(Self::Pong)
            }
            Method::Nip04Encrypt => Ok(Self::Nip04Encrypt(expect_string_result(
                method,
                envelope.result,
            )?)),
            Method::Nip04Decrypt => Ok(Self::Nip04Decrypt(expect_string_result(
                method,
                envelope.result,
            )?)),
            Method::Nip44Encrypt => Ok(Self::Nip44Encrypt(expect_string_result(
                method,
                envelope.result,
            )?)),
            Method::Nip44Decrypt => Ok(Self::Nip44Decrypt(expect_string_result(
                method,
                envelope.result,
            )?)),
            Method::SwitchRelays => parse_switch_relays_response(envelope.result),
            Method::Logout => {
                let result = expect_string_result(method, envelope.result)?;
                if result != "ack" {
                    return Err(RadrootsNostrConnectError::InvalidResponsePayload {
                        method: method.to_string(),
                        reason: format!("expected `ack`, got `{result}`"),
                    });
                }
                Ok(Self::LogoutAcknowledged)
            }
            Method::Custom(_) => Ok(Self::Custom {
                result: envelope.result,
                error: None,
            }),
        }
    }
}

fn remote_session_capability_value(
    capability: RadrootsNostrConnectRemoteSessionCapability,
) -> Value {
    json!({
        "user_public_key": capability.user_public_key,
        "relays": capability.relays,
        "permissions": capability.permissions,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RawRequestMessage {
    id: String,
    method: Method,
    params: Vec<String>,
}

fn expect_param_count(
    method: &Method,
    params: &[String],
    expected: usize,
) -> Result<(), RadrootsNostrConnectError> {
    if params.len() == expected {
        return Ok(());
    }

    Err(RadrootsNostrConnectError::InvalidParams {
        method: method.to_string(),
        expected: if expected == 0 {
            "no params"
        } else if expected == 1 {
            "exactly 1 param"
        } else {
            "exactly 2 params"
        },
        received: params.len(),
    })
}

fn parse_public_key(value: &str) -> Result<PublicKey, RadrootsNostrConnectError> {
    PublicKey::parse(value)
        .or_else(|_| PublicKey::from_hex(value))
        .map_err(|error| RadrootsNostrConnectError::InvalidPublicKey {
            value: value.to_owned(),
            reason: error.to_string(),
        })
}

fn expect_string_result(
    method: &Method,
    result: Option<Value>,
) -> Result<String, RadrootsNostrConnectError> {
    match result {
        Some(Value::String(value)) => Ok(value),
        Some(other) => Err(RadrootsNostrConnectError::InvalidResponsePayload {
            method: method.to_string(),
            reason: format!("expected string result, got {other}"),
        }),
        None => Err(RadrootsNostrConnectError::MissingResult),
    }
}

fn parse_json_string_result<T>(
    method: &Method,
    result: Option<Value>,
) -> Result<T, RadrootsNostrConnectError>
where
    T: for<'de> Deserialize<'de>,
{
    match result {
        Some(Value::String(value)) => serde_json::from_str(&value).map_err(|error| {
            RadrootsNostrConnectError::InvalidResponsePayload {
                method: method.to_string(),
                reason: error.to_string(),
            }
        }),
        Some(other) => serde_json::from_value(other).map_err(|error| {
            RadrootsNostrConnectError::InvalidResponsePayload {
                method: method.to_string(),
                reason: error.to_string(),
            }
        }),
        None => Err(RadrootsNostrConnectError::MissingResult),
    }
}

fn parse_switch_relays_response(
    result: Option<Value>,
) -> Result<RadrootsNostrConnectResponse, RadrootsNostrConnectError> {
    let method = Method::SwitchRelays;
    match result {
        None | Some(Value::Null) => Ok(RadrootsNostrConnectResponse::RelayListUnchanged),
        Some(Value::Array(values)) => {
            let relays = parse_relay_values(values)?;
            Ok(RadrootsNostrConnectResponse::RelayList(relays))
        }
        Some(Value::String(value)) if value == "null" => {
            Ok(RadrootsNostrConnectResponse::RelayListUnchanged)
        }
        Some(Value::String(value)) => {
            let parsed = serde_json::from_str::<Value>(&value).map_err(|error| {
                RadrootsNostrConnectError::InvalidResponsePayload {
                    method: method.to_string(),
                    reason: error.to_string(),
                }
            })?;
            parse_switch_relays_response(Some(parsed))
        }
        Some(other) => Err(RadrootsNostrConnectError::InvalidResponsePayload {
            method: method.to_string(),
            reason: format!("expected relay list or null, got {other}"),
        }),
    }
}

fn parse_relay_values(values: Vec<Value>) -> Result<Vec<RelayUrl>, RadrootsNostrConnectError> {
    values
        .into_iter()
        .map(|value| match value {
            Value::String(value) => RelayUrl::parse(&value).map_err(|error| {
                RadrootsNostrConnectError::InvalidRelayUrl {
                    value,
                    reason: error.to_string(),
                }
            }),
            other => Err(RadrootsNostrConnectError::InvalidResponsePayload {
                method: Method::SwitchRelays.to_string(),
                reason: format!("expected relay string, got {other}"),
            }),
        })
        .collect()
}

fn validate_url(value: &str) -> Result<String, RadrootsNostrConnectError> {
    Url::parse(value)
        .map(|url| url.to_string())
        .map_err(|error| RadrootsNostrConnectError::InvalidUrl {
            value: value.to_owned(),
            reason: error.to_string(),
        })
}

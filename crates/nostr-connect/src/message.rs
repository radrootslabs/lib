use crate::error::RadrootsNostrConnectError;
use crate::method::RadrootsNostrConnectMethod;
use crate::permission::RadrootsNostrConnectPermissions;
use nostr::{Event, JsonUtil, PublicKey, RelayUrl, UnsignedEvent};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::str::FromStr;
use url::Url;

pub const RADROOTS_NOSTR_CONNECT_RPC_KIND: u16 = 24_133;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsNostrConnectRemoteSessionCapability {
    pub user_public_key: PublicKey,
    pub relays: Vec<RelayUrl>,
    pub permissions: RadrootsNostrConnectPermissions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsNostrConnectRequest {
    Connect {
        remote_signer_public_key: PublicKey,
        secret: Option<String>,
        requested_permissions: RadrootsNostrConnectPermissions,
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
    Custom {
        method: RadrootsNostrConnectMethod,
        params: Vec<String>,
    },
}

impl RadrootsNostrConnectRequest {
    pub fn method(&self) -> RadrootsNostrConnectMethod {
        match self {
            Self::Connect { .. } => RadrootsNostrConnectMethod::Connect,
            Self::GetPublicKey => RadrootsNostrConnectMethod::GetPublicKey,
            Self::GetSessionCapability => RadrootsNostrConnectMethod::GetSessionCapability,
            Self::SignEvent(_) => RadrootsNostrConnectMethod::SignEvent,
            Self::Nip04Encrypt { .. } => RadrootsNostrConnectMethod::Nip04Encrypt,
            Self::Nip04Decrypt { .. } => RadrootsNostrConnectMethod::Nip04Decrypt,
            Self::Nip44Encrypt { .. } => RadrootsNostrConnectMethod::Nip44Encrypt,
            Self::Nip44Decrypt { .. } => RadrootsNostrConnectMethod::Nip44Decrypt,
            Self::Ping => RadrootsNostrConnectMethod::Ping,
            Self::SwitchRelays => RadrootsNostrConnectMethod::SwitchRelays,
            Self::Custom { method, .. } => method.clone(),
        }
    }

    pub fn to_params(&self) -> Vec<String> {
        match self {
            Self::Connect {
                remote_signer_public_key,
                secret,
                requested_permissions,
            } => {
                let mut params = vec![remote_signer_public_key.to_hex()];
                let normalized_secret = secret.as_ref().filter(|value| !value.is_empty()).cloned();
                if normalized_secret.is_some() || !requested_permissions.is_empty() {
                    params.push(normalized_secret.unwrap_or_default());
                }
                if !requested_permissions.is_empty() {
                    params.push(requested_permissions.to_string());
                }
                params
            }
            Self::GetPublicKey | Self::GetSessionCapability | Self::Ping | Self::SwitchRelays => {
                Vec::new()
            }
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
        }
    }

    pub fn from_parts(
        method: RadrootsNostrConnectMethod,
        params: Vec<String>,
    ) -> Result<Self, RadrootsNostrConnectError> {
        match method {
            RadrootsNostrConnectMethod::Connect => {
                if params.is_empty() || params.len() > 3 {
                    return Err(RadrootsNostrConnectError::InvalidParams {
                        method: method.to_string(),
                        expected: "1 to 3 params",
                        received: params.len(),
                    });
                }
                let remote_signer_public_key = parse_public_key(&params[0])?;
                let secret = params
                    .get(1)
                    .cloned()
                    .and_then(|value| if value.is_empty() { None } else { Some(value) });
                let requested_permissions = match params.get(2) {
                    Some(value) => RadrootsNostrConnectPermissions::from_str(value)?,
                    None => RadrootsNostrConnectPermissions::default(),
                };
                Ok(Self::Connect {
                    remote_signer_public_key,
                    secret,
                    requested_permissions,
                })
            }
            RadrootsNostrConnectMethod::GetPublicKey => {
                expect_param_count(&method, &params, 0)?;
                Ok(Self::GetPublicKey)
            }
            RadrootsNostrConnectMethod::GetSessionCapability => {
                expect_param_count(&method, &params, 0)?;
                Ok(Self::GetSessionCapability)
            }
            RadrootsNostrConnectMethod::SignEvent => {
                expect_param_count(&method, &params, 1)?;
                let unsigned_event = serde_json::from_str(&params[0]).map_err(|error| {
                    RadrootsNostrConnectError::InvalidRequestPayload {
                        method: method.to_string(),
                        reason: error.to_string(),
                    }
                })?;
                Ok(Self::SignEvent(unsigned_event))
            }
            RadrootsNostrConnectMethod::Nip04Encrypt => {
                expect_param_count(&method, &params, 2)?;
                Ok(Self::Nip04Encrypt {
                    public_key: parse_public_key(&params[0])?,
                    plaintext: params[1].clone(),
                })
            }
            RadrootsNostrConnectMethod::Nip04Decrypt => {
                expect_param_count(&method, &params, 2)?;
                Ok(Self::Nip04Decrypt {
                    public_key: parse_public_key(&params[0])?,
                    ciphertext: params[1].clone(),
                })
            }
            RadrootsNostrConnectMethod::Nip44Encrypt => {
                expect_param_count(&method, &params, 2)?;
                Ok(Self::Nip44Encrypt {
                    public_key: parse_public_key(&params[0])?,
                    plaintext: params[1].clone(),
                })
            }
            RadrootsNostrConnectMethod::Nip44Decrypt => {
                expect_param_count(&method, &params, 2)?;
                Ok(Self::Nip44Decrypt {
                    public_key: parse_public_key(&params[0])?,
                    ciphertext: params[1].clone(),
                })
            }
            RadrootsNostrConnectMethod::Ping => {
                expect_param_count(&method, &params, 0)?;
                Ok(Self::Ping)
            }
            RadrootsNostrConnectMethod::SwitchRelays => {
                expect_param_count(&method, &params, 0)?;
                Ok(Self::SwitchRelays)
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

    fn into_raw(self) -> RawRequestMessage {
        RawRequestMessage {
            id: self.id,
            method: self.request.method(),
            params: self.request.to_params(),
        }
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
        self.clone().into_raw().serialize(serializer)
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
            Self::ConnectAcknowledged => RadrootsNostrConnectResponseEnvelope {
                id,
                result: Some(Value::String("ack".to_owned())),
                error: None,
            },
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
                result: Some(serde_json::to_value(capability).map_err(|error| {
                    RadrootsNostrConnectError::InvalidResponsePayload {
                        method: RadrootsNostrConnectMethod::GetSessionCapability.to_string(),
                        reason: error.to_string(),
                    }
                })?),
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
        method: &RadrootsNostrConnectMethod,
        envelope: RadrootsNostrConnectResponseEnvelope,
    ) -> Result<Self, RadrootsNostrConnectError> {
        if let (Some(Value::String(result)), Some(url)) = (&envelope.result, &envelope.error) {
            if result == "auth_url" {
                return Ok(Self::AuthUrl(validate_url(url)?));
            }
        }

        if let Some(error) = envelope.error {
            if matches!(
                method,
                RadrootsNostrConnectMethod::GetPublicKey
                    | RadrootsNostrConnectMethod::GetSessionCapability
            ) && envelope.result.is_none()
                && error == RADROOTS_NOSTR_CONNECT_PENDING_CONNECTION_ERROR
            {
                return Ok(Self::PendingConnection);
            }
            if let RadrootsNostrConnectMethod::Custom(_) = method {
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
            RadrootsNostrConnectMethod::Connect => {
                let result = expect_string_result(method, envelope.result)?;
                if result == "ack" {
                    Ok(Self::ConnectAcknowledged)
                } else {
                    Ok(Self::ConnectSecretEcho(result))
                }
            }
            RadrootsNostrConnectMethod::GetPublicKey => {
                let result = expect_string_result(method, envelope.result)?;
                Ok(Self::UserPublicKey(parse_public_key(&result)?))
            }
            RadrootsNostrConnectMethod::GetSessionCapability => Ok(Self::RemoteSessionCapability(
                parse_json_string_result(method, envelope.result)?,
            )),
            RadrootsNostrConnectMethod::SignEvent => {
                let event = parse_json_string_result::<Event>(method, envelope.result)?;
                Ok(Self::SignedEvent(event))
            }
            RadrootsNostrConnectMethod::Ping => {
                let result = expect_string_result(method, envelope.result)?;
                if result != "pong" {
                    return Err(RadrootsNostrConnectError::InvalidResponsePayload {
                        method: method.to_string(),
                        reason: format!("expected `pong`, got `{result}`"),
                    });
                }
                Ok(Self::Pong)
            }
            RadrootsNostrConnectMethod::Nip04Encrypt => Ok(Self::Nip04Encrypt(
                expect_string_result(method, envelope.result)?,
            )),
            RadrootsNostrConnectMethod::Nip04Decrypt => Ok(Self::Nip04Decrypt(
                expect_string_result(method, envelope.result)?,
            )),
            RadrootsNostrConnectMethod::Nip44Encrypt => Ok(Self::Nip44Encrypt(
                expect_string_result(method, envelope.result)?,
            )),
            RadrootsNostrConnectMethod::Nip44Decrypt => Ok(Self::Nip44Decrypt(
                expect_string_result(method, envelope.result)?,
            )),
            RadrootsNostrConnectMethod::SwitchRelays => {
                parse_switch_relays_response(envelope.result)
            }
            RadrootsNostrConnectMethod::Custom(_) => Ok(Self::Custom {
                result: envelope.result,
                error: None,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RawRequestMessage {
    id: String,
    method: RadrootsNostrConnectMethod,
    params: Vec<String>,
}

fn expect_param_count(
    method: &RadrootsNostrConnectMethod,
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
    method: &RadrootsNostrConnectMethod,
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
    method: &RadrootsNostrConnectMethod,
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
    let method = RadrootsNostrConnectMethod::SwitchRelays;
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
                method: RadrootsNostrConnectMethod::SwitchRelays.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::method::RadrootsNostrConnectMethod;
    use crate::permission::RadrootsNostrConnectPermission;

    fn relay(value: &str) -> RelayUrl {
        RelayUrl::parse(value).expect("relay url")
    }

    fn public_key() -> PublicKey {
        PublicKey::from_hex("4f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa")
            .expect("public key")
    }

    #[test]
    fn get_session_capability_request_round_trips_without_params() {
        let request = RadrootsNostrConnectRequest::GetSessionCapability;
        let message = RadrootsNostrConnectRequestMessage::new("req-cap", request.clone());
        let encoded = serde_json::to_string(&message).expect("encode request");
        let decoded: RadrootsNostrConnectRequestMessage =
            serde_json::from_str(&encoded).expect("decode request");

        assert_eq!(decoded.request, request);
        assert_eq!(
            decoded.request.method(),
            RadrootsNostrConnectMethod::GetSessionCapability
        );
    }

    #[test]
    fn get_session_capability_response_round_trips() {
        let capability = RadrootsNostrConnectRemoteSessionCapability {
            user_public_key: public_key(),
            relays: vec![
                relay("wss://relay.example.com"),
                relay("wss://relay2.example.com"),
            ],
            permissions: vec![
                RadrootsNostrConnectPermission::with_parameter(
                    RadrootsNostrConnectMethod::SignEvent,
                    "kind:1",
                ),
                RadrootsNostrConnectPermission::new(RadrootsNostrConnectMethod::SwitchRelays),
            ]
            .into(),
        };
        let response = RadrootsNostrConnectResponse::RemoteSessionCapability(capability.clone());
        let envelope = response
            .into_envelope("resp-cap")
            .expect("encode response envelope");
        let decoded = RadrootsNostrConnectResponse::from_envelope(
            &RadrootsNostrConnectMethod::GetSessionCapability,
            envelope,
        )
        .expect("decode capability response");

        assert_eq!(
            decoded,
            RadrootsNostrConnectResponse::RemoteSessionCapability(capability.clone())
        );
        assert_eq!(
            RadrootsNostrConnectResponse::RemoteSessionCapability(capability.clone())
                .into_pending_connection_poll_outcome(),
            RadrootsNostrConnectPendingConnectionPollOutcome::ApprovedCapability(capability)
        );
    }
}

use crate::error::RadrootsNostrConnectError;
use crate::method::Method;
use crate::permission::Permissions;
use crate::uri::{ClientMetadata, RelayUrl};
use nostr::JsonUtil;
use radroots_identity::PublicKey;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;
use url::Url;

pub const RPC_KIND: u16 = 24_133;
pub const REQUEST_ID_MAX_BYTES: usize = 128;
pub const REQUEST_PARAM_COUNT_MAX: usize = 64;
pub const REQUEST_PARAM_MAX_BYTES: usize = 65_536;
pub const REQUEST_PARAMS_MAX_BYTES: usize = 262_144;
pub const RESPONSE_ERROR_MAX_BYTES: usize = 4_096;
pub const RESPONSE_RESULT_MAX_BYTES: usize = 262_144;
pub const REMOTE_CAPABILITY_RELAY_COUNT_MAX: usize = 32;

/// A validated NIP-46 unsigned-event payload with package-owned representation.
#[derive(Clone, PartialEq, Eq)]
pub struct UnsignedEvent(nostr::UnsignedEvent);

impl fmt::Debug for UnsignedEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("UnsignedEvent(<redacted>)")
    }
}

impl UnsignedEvent {
    pub fn from_json(value: &str) -> Result<Self, RadrootsNostrConnectError> {
        serde_json::from_str(value).map(Self).map_err(|error| {
            RadrootsNostrConnectError::InvalidRequestPayload {
                method: Method::SignEvent.to_string(),
                reason: error.to_string(),
            }
        })
    }

    #[must_use]
    pub fn as_json(&self) -> String {
        self.0.as_json()
    }

    #[must_use]
    pub fn kind(&self) -> u16 {
        self.0.kind.as_u16()
    }
}

/// A validated NIP-46 signed-event payload with package-owned representation.
#[derive(Clone, PartialEq, Eq)]
pub struct SignedEvent(nostr::Event);

impl fmt::Debug for SignedEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SignedEvent(<redacted>)")
    }
}

impl SignedEvent {
    pub fn from_json(value: &str) -> Result<Self, RadrootsNostrConnectError> {
        serde_json::from_str(value).map(Self).map_err(|error| {
            RadrootsNostrConnectError::InvalidResponsePayload {
                method: Method::SignEvent.to_string(),
                reason: error.to_string(),
            }
        })
    }

    #[must_use]
    pub fn as_json(&self) -> String {
        self.0.as_json()
    }
}

/// A bounded correlation identifier carried by a NIP-46 request and response.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(String);

impl RequestId {
    pub fn parse(value: impl Into<String>) -> Result<Self, RadrootsNostrConnectError> {
        let value = value.into();
        validate_request_id(&value)?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for RequestId {
    type Err = RadrootsNostrConnectError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Serialize for RequestId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RequestId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSessionCapability {
    #[doc(hidden)]
    pub user_public_key: PublicKey,
    #[doc(hidden)]
    pub relays: Vec<RelayUrl>,
    #[doc(hidden)]
    pub permissions: Permissions,
}

impl RemoteSessionCapability {
    pub fn try_new(
        user_public_key: PublicKey,
        relays: Vec<RelayUrl>,
        permissions: Permissions,
    ) -> Result<Self, RadrootsNostrConnectError> {
        let capability = Self {
            user_public_key,
            relays,
            permissions,
        };
        capability.validate()?;
        Ok(capability)
    }

    #[must_use]
    pub const fn user_public_key(&self) -> PublicKey {
        self.user_public_key
    }

    #[must_use]
    pub fn relays(&self) -> &[RelayUrl] {
        &self.relays
    }

    #[must_use]
    pub fn permissions(&self) -> &Permissions {
        &self.permissions
    }

    fn validate(&self) -> Result<(), RadrootsNostrConnectError> {
        if self.relays.len() > REMOTE_CAPABILITY_RELAY_COUNT_MAX {
            return Err(RadrootsNostrConnectError::InvalidResponsePayload {
                method: Method::GetSessionCapability.to_string(),
                reason: "remote capability relay count exceeds its limit".to_owned(),
            });
        }
        self.permissions
            .to_string()
            .parse::<Permissions>()
            .map(|_| ())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoteSessionCapabilitySerde {
    user_public_key: String,
    relays: Vec<RelayUrl>,
    permissions: Permissions,
}

impl Serialize for RemoteSessionCapability {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.validate().map_err(serde::ser::Error::custom)?;
        RemoteSessionCapabilitySerde {
            user_public_key: self.user_public_key.to_hex(),
            relays: self.relays.clone(),
            permissions: self.permissions.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RemoteSessionCapability {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RemoteSessionCapabilitySerde::deserialize(deserializer)?;
        let user_public_key =
            parse_public_key(&raw.user_public_key).map_err(serde::de::Error::custom)?;
        Self::try_new(user_public_key, raw.relays, raw.permissions)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum Request {
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

impl fmt::Debug for Request {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Request")
            .field("method", &self.method())
            .field("payload", &"<redacted>")
            .finish()
    }
}

impl Request {
    /// Returns the canonical NIP-46 method represented by this payload.
    #[must_use]
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
        validate_params(&params)?;
        Ok(params)
    }

    pub fn from_parts(
        method: Method,
        params: Vec<String>,
    ) -> Result<Self, RadrootsNostrConnectError> {
        validate_params(&params)?;
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
                let unsigned_event = UnsignedEvent::from_json(&params[0])?;
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
pub struct RequestMessage {
    #[doc(hidden)]
    pub id: String,
    #[doc(hidden)]
    pub request: Request,
}

impl RequestMessage {
    /// Creates and validates a serialized request envelope.
    pub fn try_new(
        id: impl Into<String>,
        request: Request,
    ) -> Result<Self, RadrootsNostrConnectError> {
        let id = id.into();
        validate_request_id(&id)?;
        request.to_params()?;
        Ok(Self { id, request })
    }

    /// Compatibility constructor retained until the Step 141 consumer cutover.
    #[doc(hidden)]
    #[must_use]
    pub fn new(id: impl Into<String>, request: Request) -> Self {
        Self {
            id: id.into(),
            request,
        }
    }

    pub fn request_id(&self) -> Result<RequestId, RadrootsNostrConnectError> {
        RequestId::parse(self.id.clone())
    }

    #[must_use]
    pub fn payload(&self) -> &Request {
        &self.request
    }

    /// Correlates and decodes a response using this request's method.
    pub fn correlate(
        &self,
        envelope: ResponseEnvelope,
    ) -> Result<Response, RadrootsNostrConnectError> {
        envelope.validate()?;
        if envelope.id != self.id {
            return Err(RadrootsNostrConnectError::WrongRequestId);
        }
        Response::from_envelope(&self.request.method(), envelope)
    }

    fn into_raw(self) -> Result<RawRequestMessage, RadrootsNostrConnectError> {
        validate_request_id(&self.id)?;
        Ok(RawRequestMessage {
            id: self.id,
            method: self.request.method(),
            params: self.request.to_params()?,
        })
    }

    fn from_raw(raw: RawRequestMessage) -> Result<Self, RadrootsNostrConnectError> {
        let request = Request::from_parts(raw.method, raw.params)?;
        Self::try_new(raw.id, request)
    }
}

impl Serialize for RequestMessage {
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

impl<'de> Deserialize<'de> for RequestMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawRequestMessage::deserialize(deserializer)?;
        Self::from_raw(raw).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ResponseEnvelope {
    #[doc(hidden)]
    pub id: String,
    #[doc(hidden)]
    pub result: Option<Value>,
    #[doc(hidden)]
    pub error: Option<String>,
}

impl fmt::Debug for ResponseEnvelope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResponseEnvelope")
            .field("id", &self.id)
            .field("has_result", &self.result.is_some())
            .field("has_error", &self.error.is_some())
            .finish()
    }
}

impl ResponseEnvelope {
    pub fn try_new(
        id: impl Into<String>,
        result: Option<Value>,
        error: Option<String>,
    ) -> Result<Self, RadrootsNostrConnectError> {
        let envelope = Self {
            id: id.into(),
            result,
            error,
        };
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn request_id(&self) -> Result<RequestId, RadrootsNostrConnectError> {
        RequestId::parse(self.id.clone())
    }

    #[must_use]
    pub fn result(&self) -> Option<&Value> {
        self.result.as_ref()
    }

    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn validate(&self) -> Result<(), RadrootsNostrConnectError> {
        validate_request_id(&self.id)?;
        if let Some(error) = self.error.as_deref()
            && (error.is_empty()
                || error.len() > RESPONSE_ERROR_MAX_BYTES
                || error.chars().any(char::is_control))
        {
            return Err(RadrootsNostrConnectError::InvalidResponseEnvelope {
                reason: "error must be non-empty, bounded, and control-free",
            });
        }
        if let Some(result) = self.result.as_ref()
            && serde_json::to_vec(result)
                .map_err(RadrootsNostrConnectError::from)?
                .len()
                > RESPONSE_RESULT_MAX_BYTES
        {
            return Err(RadrootsNostrConnectError::InvalidResponseEnvelope {
                reason: "result exceeds its byte limit",
            });
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResponseEnvelopeSerde {
    id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl Serialize for ResponseEnvelope {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.validate().map_err(serde::ser::Error::custom)?;
        ResponseEnvelopeSerde {
            id: self.id.clone(),
            result: self.result.clone(),
            error: self.error.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ResponseEnvelope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = ResponseEnvelopeSerde::deserialize(deserializer)?;
        Self::try_new(raw.id, raw.result, raw.error).map_err(serde::de::Error::custom)
    }
}

/// Correlation state supplied by a caller that owns response-event identity.
#[derive(Debug)]
pub struct ResponseValidator {
    request_id: RequestId,
    expected_signer: radroots_identity::PublicKey,
    seen_fingerprints: BTreeSet<String>,
}

impl ResponseValidator {
    #[must_use]
    pub fn new(request_id: RequestId, expected_signer: radroots_identity::PublicKey) -> Self {
        Self {
            request_id,
            expected_signer,
            seen_fingerprints: BTreeSet::new(),
        }
    }

    /// Validates signer and request correlation and rejects a repeated event fingerprint.
    pub fn validate(
        &mut self,
        signer: radroots_identity::PublicKey,
        response_fingerprint: impl Into<String>,
        envelope: &ResponseEnvelope,
    ) -> Result<(), RadrootsNostrConnectError> {
        if signer != self.expected_signer {
            return Err(RadrootsNostrConnectError::WrongResponseSigner);
        }
        envelope.validate()?;
        if envelope.id != self.request_id.as_str() {
            return Err(RadrootsNostrConnectError::WrongRequestId);
        }
        let fingerprint = response_fingerprint.into();
        validate_response_fingerprint(&fingerprint)?;
        if !self.seen_fingerprints.insert(fingerprint) {
            return Err(RadrootsNostrConnectError::ReplayedResponse);
        }
        Ok(())
    }
}

pub const PENDING_CONNECTION_ERROR: &str = "connection is pending";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingConnectionOutcome {
    PendingApproval,
    Approved(PublicKey),
    ApprovedCapability(RemoteSessionCapability),
    Rejected { message: String },
    AuthChallenge { url: String },
    UnexpectedResponse { response: String },
}

#[derive(Clone, PartialEq, Eq)]
pub enum Response {
    ConnectAcknowledged,
    ConnectSecretEcho(String),
    LogoutAcknowledged,
    PendingConnection,
    UserPublicKey(PublicKey),
    RemoteSessionCapability(RemoteSessionCapability),
    SignedEvent(SignedEvent),
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

impl fmt::Debug for Response {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Response")
            .field("kind", &self.kind_name())
            .field("payload", &"<redacted>")
            .finish()
    }
}

impl Response {
    const fn kind_name(&self) -> &'static str {
        match self {
            Self::ConnectAcknowledged => "connect_acknowledged",
            Self::ConnectSecretEcho(_) => "connect_secret_echo",
            Self::LogoutAcknowledged => "logout_acknowledged",
            Self::PendingConnection => "pending_connection",
            Self::UserPublicKey(_) => "user_public_key",
            Self::RemoteSessionCapability(_) => "remote_session_capability",
            Self::SignedEvent(_) => "signed_event",
            Self::Pong => "pong",
            Self::Nip04Encrypt(_) => "nip04_encrypt",
            Self::Nip04Decrypt(_) => "nip04_decrypt",
            Self::Nip44Encrypt(_) => "nip44_encrypt",
            Self::Nip44Decrypt(_) => "nip44_decrypt",
            Self::RelayList(_) => "relay_list",
            Self::RelayListUnchanged => "relay_list_unchanged",
            Self::AuthUrl(_) => "auth_url",
            Self::Error { .. } => "error",
            Self::Custom { .. } => "custom",
        }
    }

    pub fn into_pending_connection_poll_outcome(self) -> PendingConnectionOutcome {
        match self {
            Self::PendingConnection => PendingConnectionOutcome::PendingApproval,
            Self::UserPublicKey(public_key) => PendingConnectionOutcome::Approved(public_key),
            Self::RemoteSessionCapability(capability) => {
                PendingConnectionOutcome::ApprovedCapability(capability)
            }
            Self::Error { error, .. } if error == PENDING_CONNECTION_ERROR => {
                PendingConnectionOutcome::PendingApproval
            }
            Self::Error { error, .. } => PendingConnectionOutcome::Rejected { message: error },
            Self::AuthUrl(url) => PendingConnectionOutcome::AuthChallenge { url },
            other => PendingConnectionOutcome::UnexpectedResponse {
                response: other.kind_name().to_owned(),
            },
        }
    }

    pub fn into_envelope(
        self,
        id: impl Into<String>,
    ) -> Result<ResponseEnvelope, RadrootsNostrConnectError> {
        let id = id.into();
        let envelope = match self {
            Self::ConnectAcknowledged | Self::LogoutAcknowledged => ResponseEnvelope {
                id,
                result: Some(Value::String("ack".to_owned())),
                error: None,
            },
            Self::ConnectSecretEcho(secret) => ResponseEnvelope {
                id,
                result: Some(Value::String(secret)),
                error: None,
            },
            Self::PendingConnection => ResponseEnvelope {
                id,
                result: None,
                error: Some(PENDING_CONNECTION_ERROR.to_owned()),
            },
            Self::UserPublicKey(public_key) => ResponseEnvelope {
                id,
                result: Some(Value::String(public_key.to_hex())),
                error: None,
            },
            Self::RemoteSessionCapability(capability) => ResponseEnvelope {
                id,
                result: Some(remote_session_capability_value(capability)),
                error: None,
            },
            Self::SignedEvent(event) => ResponseEnvelope {
                id,
                result: Some(Value::String(event.as_json())),
                error: None,
            },
            Self::Pong => ResponseEnvelope {
                id,
                result: Some(Value::String("pong".to_owned())),
                error: None,
            },
            Self::Nip04Encrypt(text)
            | Self::Nip04Decrypt(text)
            | Self::Nip44Encrypt(text)
            | Self::Nip44Decrypt(text) => ResponseEnvelope {
                id,
                result: Some(Value::String(text)),
                error: None,
            },
            Self::RelayList(relays) => {
                let relays = relays
                    .into_iter()
                    .map(|relay| relay.to_string())
                    .collect::<Vec<_>>();
                ResponseEnvelope {
                    id,
                    result: Some(Value::Array(
                        relays.into_iter().map(Value::String).collect(),
                    )),
                    error: None,
                }
            }
            Self::RelayListUnchanged => ResponseEnvelope {
                id,
                result: Some(Value::Null),
                error: None,
            },
            Self::AuthUrl(url) => {
                let normalized = validate_url(&url)?;
                ResponseEnvelope {
                    id,
                    result: Some(Value::String("auth_url".to_owned())),
                    error: Some(normalized),
                }
            }
            Self::Error { result, error } => ResponseEnvelope {
                id,
                result,
                error: Some(error),
            },
            Self::Custom { result, error } => ResponseEnvelope { id, result, error },
        };
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn from_envelope(
        method: &Method,
        envelope: ResponseEnvelope,
    ) -> Result<Self, RadrootsNostrConnectError> {
        envelope.validate()?;
        if let (Some(Value::String(result)), Some(url)) = (&envelope.result, &envelope.error)
            && result == "auth_url"
        {
            return Ok(Self::AuthUrl(validate_url(url)?));
        }

        if let Some(error) = envelope.error {
            if matches!(method, Method::GetPublicKey | Method::GetSessionCapability)
                && envelope.result.is_none()
                && error == PENDING_CONNECTION_ERROR
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
                let value = expect_json_string_or_value(method, envelope.result)?;
                let event = SignedEvent::from_json(&value)?;
                Ok(Self::SignedEvent(event))
            }
            Method::Ping => {
                let result = expect_string_result(method, envelope.result)?;
                if result != "pong" {
                    return Err(RadrootsNostrConnectError::InvalidResponsePayload {
                        method: method.to_string(),
                        reason: "expected canonical `pong` result".to_owned(),
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
                        reason: "expected canonical `ack` result".to_owned(),
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

fn remote_session_capability_value(capability: RemoteSessionCapability) -> Value {
    json!({
        "user_public_key": capability.user_public_key.to_hex(),
        "relays": capability.relays,
        "permissions": capability.permissions,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRequestMessage {
    id: String,
    method: Method,
    params: Vec<String>,
}

fn validate_request_id(value: &str) -> Result<(), RadrootsNostrConnectError> {
    if value.is_empty() {
        return Err(RadrootsNostrConnectError::InvalidRequestId {
            reason: "request id cannot be empty",
        });
    }
    if value.len() > REQUEST_ID_MAX_BYTES {
        return Err(RadrootsNostrConnectError::InvalidRequestId {
            reason: "request id exceeds its byte limit",
        });
    }
    if value.trim() != value || value.chars().any(char::is_control) {
        return Err(RadrootsNostrConnectError::InvalidRequestId {
            reason: "request id must be canonical and control-free",
        });
    }
    Ok(())
}

fn validate_params(params: &[String]) -> Result<(), RadrootsNostrConnectError> {
    if params.len() > REQUEST_PARAM_COUNT_MAX {
        return Err(RadrootsNostrConnectError::InvalidRequestPayload {
            method: "custom".to_owned(),
            reason: "parameter count exceeds its limit".to_owned(),
        });
    }
    if params
        .iter()
        .any(|param| param.len() > REQUEST_PARAM_MAX_BYTES)
    {
        return Err(RadrootsNostrConnectError::InvalidRequestPayload {
            method: "unknown".to_owned(),
            reason: "a parameter exceeds its byte limit".to_owned(),
        });
    }
    if params.iter().map(String::len).sum::<usize>() > REQUEST_PARAMS_MAX_BYTES {
        return Err(RadrootsNostrConnectError::InvalidRequestPayload {
            method: "unknown".to_owned(),
            reason: "serialized parameters exceed their byte limit".to_owned(),
        });
    }
    Ok(())
}

fn validate_response_fingerprint(value: &str) -> Result<(), RadrootsNostrConnectError> {
    if value.is_empty() || value.len() > REQUEST_ID_MAX_BYTES || value.chars().any(char::is_control)
    {
        return Err(RadrootsNostrConnectError::InvalidResponseEnvelope {
            reason: "response fingerprint must be non-empty, bounded, and control-free",
        });
    }
    Ok(())
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
    radroots_nostr::key::parse_public_key(value).map_err(|error| {
        RadrootsNostrConnectError::InvalidPublicKey {
            value: value.to_owned(),
            reason: error.to_string(),
        }
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
            reason: format!("expected string result, got {}", json_type(&other)),
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

fn expect_json_string_or_value(
    method: &Method,
    result: Option<Value>,
) -> Result<String, RadrootsNostrConnectError> {
    match result {
        Some(Value::String(value)) => Ok(value),
        Some(value) => serde_json::to_string(&value).map_err(|error| {
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
) -> Result<Response, RadrootsNostrConnectError> {
    let method = Method::SwitchRelays;
    match result {
        None | Some(Value::Null) => Ok(Response::RelayListUnchanged),
        Some(Value::Array(values)) => {
            let relays = parse_relay_values(values)?;
            Ok(Response::RelayList(relays))
        }
        Some(Value::String(value)) if value == "null" => Ok(Response::RelayListUnchanged),
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
            reason: format!("expected relay list or null, got {}", json_type(&other)),
        }),
    }
}

fn parse_relay_values(values: Vec<Value>) -> Result<Vec<RelayUrl>, RadrootsNostrConnectError> {
    values
        .into_iter()
        .map(|value| match value {
            Value::String(value) => RelayUrl::parse(&value),
            other => Err(RadrootsNostrConnectError::InvalidResponsePayload {
                method: Method::SwitchRelays.to_string(),
                reason: format!("expected relay string, got {}", json_type(&other)),
            }),
        })
        .collect()
}

fn validate_url(value: &str) -> Result<String, RadrootsNostrConnectError> {
    if value.len() > crate::uri::CLIENT_URL_MAX_BYTES || value.chars().any(char::is_control) {
        return Err(RadrootsNostrConnectError::InvalidUrl {
            value: "[redacted auth URL]".to_owned(),
            reason: "auth URL is oversized or contains control characters".to_owned(),
        });
    }
    let url = Url::parse(value).map_err(|error| RadrootsNostrConnectError::InvalidUrl {
        value: "[redacted auth URL]".to_owned(),
        reason: error.to_string(),
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(RadrootsNostrConnectError::InvalidUrl {
            value: "[redacted auth URL]".to_owned(),
            reason: "auth URL scheme must be http or https".to_owned(),
        });
    }
    Ok(url.to_string())
}

fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

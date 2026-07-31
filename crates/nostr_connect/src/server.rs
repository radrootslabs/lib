//! Relay- and persistence-independent NIP-46 server state.

use crate::error::RadrootsNostrConnectError;
use crate::message::{Request, RequestId, RequestMessage, Response, ResponseEnvelope};
use crate::method::Method;
use crate::permission::{Permission, Permissions};
use std::collections::BTreeSet;

pub const SERVER_MESSAGE_MAX_BYTES: usize = 524_288;
pub const SERVER_REPLAY_WINDOW_MAX: usize = 4_096;

/// Protocol-only server state for one bounded replay window.
///
/// Approval UI, durable session state, encryption/signing keys, and relay
/// execution remain host responsibilities.
#[derive(Debug)]
pub struct Server {
    supported_extensions: BTreeSet<Method>,
    seen_request_ids: BTreeSet<RequestId>,
    seen_fingerprints: BTreeSet<String>,
}

impl Server {
    #[must_use]
    pub fn new() -> Self {
        Self {
            supported_extensions: BTreeSet::new(),
            seen_request_ids: BTreeSet::new(),
            seen_fingerprints: BTreeSet::new(),
        }
    }

    pub fn with_supported_extensions(
        extensions: impl IntoIterator<Item = Method>,
    ) -> Result<Self, RadrootsNostrConnectError> {
        let mut server = Self::new();
        for extension in extensions {
            if !matches!(extension, Method::Custom(_)) {
                return Err(RadrootsNostrConnectError::InvalidServerState {
                    reason: "server extensions must use custom methods",
                });
            }
            server.supported_extensions.insert(extension);
        }
        Ok(server)
    }

    /// Parses and admits one decrypted request after the host verifies its event.
    pub fn parse(
        &mut self,
        fingerprint: impl Into<String>,
        message_json: &str,
    ) -> Result<ServerRequest, RadrootsNostrConnectError> {
        if message_json.len() > SERVER_MESSAGE_MAX_BYTES {
            return Err(RadrootsNostrConnectError::InvalidServerRequest {
                reason: "request message exceeds its byte limit",
            });
        }
        let fingerprint = fingerprint.into();
        if fingerprint.is_empty()
            || fingerprint.len() > 128
            || fingerprint.chars().any(char::is_control)
        {
            return Err(RadrootsNostrConnectError::InvalidServerRequest {
                reason: "request fingerprint must be non-empty, bounded, and control-free",
            });
        }
        if self.seen_fingerprints.len() >= SERVER_REPLAY_WINDOW_MAX
            || self.seen_request_ids.len() >= SERVER_REPLAY_WINDOW_MAX
        {
            return Err(RadrootsNostrConnectError::InvalidServerState {
                reason: "server replay window is full",
            });
        }

        let message: RequestMessage =
            serde_json::from_str(message_json).map_err(RadrootsNostrConnectError::from)?;
        let request_id = message.request_id()?;
        let method = message.payload().method();
        if matches!(&method, Method::Custom(_)) && !self.supported_extensions.contains(&method) {
            return Err(RadrootsNostrConnectError::UnsupportedMethod(method));
        }
        if self.seen_fingerprints.contains(&fingerprint)
            || self.seen_request_ids.contains(&request_id)
        {
            return Err(RadrootsNostrConnectError::ReplayedRequest);
        }
        self.seen_fingerprints.insert(fingerprint);
        self.seen_request_ids.insert(request_id.clone());

        Ok(ServerRequest {
            request_id,
            required_permission: required_permission(message.payload()),
            request: message.request,
        })
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerRequest {
    request_id: RequestId,
    request: Request,
    required_permission: Option<Permission>,
}

impl ServerRequest {
    #[must_use]
    pub fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    #[must_use]
    pub fn request(&self) -> &Request {
        &self.request
    }

    #[must_use]
    pub fn required_permission(&self) -> Option<&Permission> {
        self.required_permission.as_ref()
    }

    #[must_use]
    pub fn is_allowed_by(&self, granted: &Permissions) -> bool {
        self.required_permission.as_ref().is_none_or(|permission| {
            granted.allows_request(permission.method(), permission.parameter())
        })
    }

    /// Constructs a correlated plaintext response for host encryption/signing.
    pub fn respond(self, response: Response) -> Result<ServerResponse, RadrootsNostrConnectError> {
        let envelope = response.into_envelope(self.request_id.to_string())?;
        let json = serde_json::to_string(&envelope).map_err(RadrootsNostrConnectError::from)?;
        Ok(ServerResponse { envelope, json })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ServerResponse {
    envelope: ResponseEnvelope,
    json: String,
}

impl ServerResponse {
    #[must_use]
    pub fn envelope(&self) -> &ResponseEnvelope {
        &self.envelope
    }

    #[must_use]
    pub fn as_json(&self) -> &str {
        &self.json
    }
}

impl std::fmt::Debug for ServerResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ServerResponse(<redacted>)")
    }
}

#[must_use]
pub fn required_permission(request: &Request) -> Option<Permission> {
    match request {
        Request::Connect { .. }
        | Request::GetPublicKey
        | Request::GetSessionCapability
        | Request::Ping
        | Request::Logout => None,
        Request::SignEvent(event) => Some(Permission::with_parameter(
            Method::SignEvent,
            format!("kind:{}", event.kind()),
        )),
        Request::Nip04Encrypt { .. } => Some(Permission::new(Method::Nip04Encrypt)),
        Request::Nip04Decrypt { .. } => Some(Permission::new(Method::Nip04Decrypt)),
        Request::Nip44Encrypt { .. } => Some(Permission::new(Method::Nip44Encrypt)),
        Request::Nip44Decrypt { .. } => Some(Permission::new(Method::Nip44Decrypt)),
        Request::SwitchRelays => Some(Permission::new(Method::SwitchRelays)),
        Request::Custom { method, .. } => Some(Permission::new(method.clone())),
    }
}

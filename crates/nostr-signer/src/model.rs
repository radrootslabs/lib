use crate::error::RadrootsNostrSignerError;
use hex::encode as hex_encode;
use nostr::{PublicKey, RelayUrl};
use radroots_identity::RadrootsIdentityPublic;
use radroots_nostr_connect::prelude::{
    RadrootsNostrConnectMethod, RadrootsNostrConnectPermission, RadrootsNostrConnectPermissions,
    RadrootsNostrConnectRequestMessage,
};
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::str::FromStr;
use url::Url;
use uuid::Uuid;

pub const RADROOTS_NOSTR_SIGNER_STORE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RadrootsNostrSignerConnectionId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RadrootsNostrSignerRequestId(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RadrootsNostrSignerApprovalRequirement {
    NotRequired,
    ExplicitUser,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RadrootsNostrSignerApprovalState {
    NotRequired,
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RadrootsNostrSignerConnectionStatus {
    Pending,
    Active,
    Rejected,
    Revoked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RadrootsNostrSignerRequestDecision {
    Allowed,
    Denied,
    Challenged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RadrootsNostrSignerAuthState {
    NotRequired,
    Pending,
    Authorized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsNostrSignerSecretDigestAlgorithm {
    Sha256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsNostrSignerConnectSecretHash {
    pub algorithm: RadrootsNostrSignerSecretDigestAlgorithm,
    pub digest_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RadrootsNostrSignerAuthChallenge {
    pub auth_url: String,
    pub required_at_unix: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorized_at_unix: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsNostrSignerPendingRequest {
    pub request_message: RadrootsNostrConnectRequestMessage,
    pub created_at_unix: u64,
}

#[derive(Debug, Clone)]
pub struct RadrootsNostrSignerAuthorizationOutcome {
    pub connection: RadrootsNostrSignerConnectionRecord,
    pub pending_request: Option<RadrootsNostrSignerPendingRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsNostrSignerPermissionGrant {
    #[serde(
        serialize_with = "serialize_permission",
        deserialize_with = "deserialize_permission"
    )]
    pub permission: RadrootsNostrConnectPermission,
    pub granted_at_unix: u64,
}

#[derive(Debug, Clone)]
pub struct RadrootsNostrSignerConnectionDraft {
    pub client_public_key: PublicKey,
    pub user_identity: RadrootsIdentityPublic,
    pub connect_secret: Option<String>,
    pub requested_permissions: RadrootsNostrConnectPermissions,
    pub relays: Vec<RelayUrl>,
    pub approval_requirement: RadrootsNostrSignerApprovalRequirement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadrootsNostrSignerConnectionRecord {
    pub connection_id: RadrootsNostrSignerConnectionId,
    pub client_public_key: PublicKey,
    pub signer_identity: RadrootsIdentityPublic,
    pub user_identity: RadrootsIdentityPublic,
    #[serde(
        default,
        alias = "connect_secret",
        deserialize_with = "deserialize_connect_secret_hash_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub connect_secret_hash: Option<RadrootsNostrSignerConnectSecretHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connect_secret_consumed_at_unix: Option<u64>,
    pub requested_permissions: RadrootsNostrConnectPermissions,
    #[serde(default)]
    pub granted_permissions: Vec<RadrootsNostrSignerPermissionGrant>,
    #[serde(default)]
    pub relays: Vec<RelayUrl>,
    pub approval_requirement: RadrootsNostrSignerApprovalRequirement,
    pub approval_state: RadrootsNostrSignerApprovalState,
    #[serde(default)]
    pub auth_state: RadrootsNostrSignerAuthState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_challenge: Option<RadrootsNostrSignerAuthChallenge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_request: Option<RadrootsNostrSignerPendingRequest>,
    pub status: RadrootsNostrSignerConnectionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_reason: Option<String>,
    pub created_at_unix: u64,
    pub updated_at_unix: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_authenticated_at_unix: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_request_at_unix: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsNostrSignerRequestAuditRecord {
    pub request_id: RadrootsNostrSignerRequestId,
    pub connection_id: RadrootsNostrSignerConnectionId,
    pub method: RadrootsNostrConnectMethod,
    pub decision: RadrootsNostrSignerRequestDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub created_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadrootsNostrSignerStoreState {
    pub version: u32,
    pub signer_identity: Option<RadrootsIdentityPublic>,
    pub connections: Vec<RadrootsNostrSignerConnectionRecord>,
    pub audit_records: Vec<RadrootsNostrSignerRequestAuditRecord>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RadrootsNostrSignerConnectSecretHashRepr {
    Hash(RadrootsNostrSignerConnectSecretHash),
    LegacyPlaintext(String),
}

impl RadrootsNostrSignerConnectionId {
    pub fn new_v7() -> Self {
        Self(Uuid::now_v7().to_string())
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsNostrSignerError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(RadrootsNostrSignerError::InvalidConnectionId(
                value.to_owned(),
            ));
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for RadrootsNostrSignerConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for RadrootsNostrSignerConnectionId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for RadrootsNostrSignerConnectionId {
    type Err = RadrootsNostrSignerError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl RadrootsNostrSignerRequestId {
    pub fn new_v7() -> Self {
        Self(Uuid::now_v7().to_string())
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsNostrSignerError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(RadrootsNostrSignerError::InvalidRequestId(value.to_owned()));
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for RadrootsNostrSignerRequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for RadrootsNostrSignerRequestId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for RadrootsNostrSignerRequestId {
    type Err = RadrootsNostrSignerError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl RadrootsNostrSignerConnectSecretHash {
    pub fn from_secret(secret: &str) -> Option<Self> {
        normalize_optional_string(secret).map(|normalized| {
            let mut hasher = Sha256::new();
            hasher.update(normalized.as_bytes());
            Self {
                algorithm: RadrootsNostrSignerSecretDigestAlgorithm::Sha256,
                digest_hex: hex_encode(hasher.finalize()),
            }
        })
    }

    pub fn matches_secret(&self, secret: &str) -> bool {
        Self::from_secret(secret).as_ref() == Some(self)
    }

    fn normalize(self) -> Result<Self, String> {
        let digest_hex = self.digest_hex.trim().to_ascii_lowercase();
        if digest_hex.len() != 64 || !digest_hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err("invalid connect secret digest".into());
        }
        Ok(Self {
            algorithm: self.algorithm,
            digest_hex,
        })
    }
}

impl RadrootsNostrSignerAuthChallenge {
    pub fn new(auth_url: &str, required_at_unix: u64) -> Result<Self, RadrootsNostrSignerError> {
        let auth_url = normalize_optional_string(auth_url)
            .ok_or_else(|| RadrootsNostrSignerError::InvalidAuthUrl(auth_url.to_owned()))?;
        let auth_url: String = Url::parse(&auth_url)
            .map_err(|_| RadrootsNostrSignerError::InvalidAuthUrl(auth_url.clone()))?
            .into();
        Ok(Self {
            auth_url,
            required_at_unix,
            authorized_at_unix: None,
        })
    }

    pub fn mark_authorized(&mut self, authorized_at_unix: u64) {
        self.authorized_at_unix = Some(authorized_at_unix);
    }
}

impl<'de> Deserialize<'de> for RadrootsNostrSignerAuthChallenge {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawAuthChallenge {
            auth_url: String,
            required_at_unix: u64,
            #[serde(default)]
            authorized_at_unix: Option<u64>,
        }

        let raw = RawAuthChallenge::deserialize(deserializer)?;
        let mut challenge =
            Self::new(&raw.auth_url, raw.required_at_unix).map_err(serde::de::Error::custom)?;
        challenge.authorized_at_unix = raw.authorized_at_unix;
        Ok(challenge)
    }
}

impl RadrootsNostrSignerPendingRequest {
    pub fn new(
        request_message: RadrootsNostrConnectRequestMessage,
        created_at_unix: u64,
    ) -> Result<Self, RadrootsNostrSignerError> {
        let normalized_id = RadrootsNostrSignerRequestId::parse(&request_message.id)?;
        Ok(Self {
            request_message: RadrootsNostrConnectRequestMessage::new(
                normalized_id.as_str(),
                request_message.request,
            ),
            created_at_unix,
        })
    }

    pub fn request_message(&self) -> RadrootsNostrConnectRequestMessage {
        self.request_message.clone()
    }

    pub fn request_id(&self) -> RadrootsNostrSignerRequestId {
        RadrootsNostrSignerRequestId::parse(&self.request_message.id)
            .expect("pending request ids are validated on construction")
    }
}

impl RadrootsNostrSignerAuthorizationOutcome {
    pub fn new(
        connection: RadrootsNostrSignerConnectionRecord,
        pending_request: Option<RadrootsNostrSignerPendingRequest>,
    ) -> Self {
        Self {
            connection,
            pending_request,
        }
    }
}

impl Default for RadrootsNostrSignerAuthState {
    fn default() -> Self {
        Self::NotRequired
    }
}

impl RadrootsNostrSignerPermissionGrant {
    pub fn new(permission: RadrootsNostrConnectPermission, granted_at_unix: u64) -> Self {
        Self {
            permission,
            granted_at_unix,
        }
    }
}

impl RadrootsNostrSignerConnectionDraft {
    pub fn new(client_public_key: PublicKey, user_identity: RadrootsIdentityPublic) -> Self {
        Self {
            client_public_key,
            user_identity,
            connect_secret: None,
            requested_permissions: RadrootsNostrConnectPermissions::default(),
            relays: Vec::new(),
            approval_requirement: RadrootsNostrSignerApprovalRequirement::NotRequired,
        }
    }

    pub fn with_connect_secret(mut self, connect_secret: impl Into<String>) -> Self {
        self.connect_secret = Some(connect_secret.into());
        self
    }

    pub fn with_requested_permissions(
        mut self,
        requested_permissions: RadrootsNostrConnectPermissions,
    ) -> Self {
        self.requested_permissions = requested_permissions;
        self
    }

    pub fn with_relays(mut self, relays: Vec<RelayUrl>) -> Self {
        self.relays = relays;
        self
    }

    pub fn with_approval_requirement(
        mut self,
        approval_requirement: RadrootsNostrSignerApprovalRequirement,
    ) -> Self {
        self.approval_requirement = approval_requirement;
        self
    }
}

impl RadrootsNostrSignerConnectionRecord {
    pub fn new(
        connection_id: RadrootsNostrSignerConnectionId,
        signer_identity: RadrootsIdentityPublic,
        draft: RadrootsNostrSignerConnectionDraft,
        created_at_unix: u64,
    ) -> Self {
        let (approval_state, status) = match draft.approval_requirement {
            RadrootsNostrSignerApprovalRequirement::NotRequired => (
                RadrootsNostrSignerApprovalState::NotRequired,
                RadrootsNostrSignerConnectionStatus::Active,
            ),
            RadrootsNostrSignerApprovalRequirement::ExplicitUser => (
                RadrootsNostrSignerApprovalState::Pending,
                RadrootsNostrSignerConnectionStatus::Pending,
            ),
        };

        Self {
            connection_id,
            client_public_key: draft.client_public_key,
            signer_identity,
            user_identity: draft.user_identity,
            connect_secret_hash: draft
                .connect_secret
                .as_deref()
                .and_then(RadrootsNostrSignerConnectSecretHash::from_secret),
            connect_secret_consumed_at_unix: None,
            requested_permissions: draft.requested_permissions,
            granted_permissions: Vec::new(),
            relays: draft.relays,
            approval_requirement: draft.approval_requirement,
            approval_state,
            auth_state: RadrootsNostrSignerAuthState::NotRequired,
            auth_challenge: None,
            pending_request: None,
            status,
            status_reason: None,
            created_at_unix,
            updated_at_unix: created_at_unix,
            last_authenticated_at_unix: None,
            last_request_at_unix: None,
        }
    }

    pub fn granted_permissions(&self) -> RadrootsNostrConnectPermissions {
        self.granted_permissions
            .iter()
            .map(|grant| grant.permission.clone())
            .collect::<Vec<_>>()
            .into()
    }

    pub fn effective_permissions(&self) -> RadrootsNostrConnectPermissions {
        let granted_permissions = self.granted_permissions();
        if !granted_permissions.is_empty() {
            granted_permissions
        } else if self.approval_state == RadrootsNostrSignerApprovalState::NotRequired {
            self.requested_permissions.clone()
        } else {
            RadrootsNostrConnectPermissions::default()
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            RadrootsNostrSignerConnectionStatus::Rejected
                | RadrootsNostrSignerConnectionStatus::Revoked
        )
    }

    pub fn connect_secret_is_consumed(&self) -> bool {
        self.connect_secret_hash.is_some() && self.connect_secret_consumed_at_unix.is_some()
    }

    pub fn touch_updated(&mut self, updated_at_unix: u64) {
        self.updated_at_unix = updated_at_unix;
    }

    pub fn mark_authenticated(&mut self, authenticated_at_unix: u64) {
        self.last_authenticated_at_unix = Some(authenticated_at_unix);
        self.updated_at_unix = authenticated_at_unix;
    }

    pub fn mark_request(&mut self, request_at_unix: u64) {
        self.last_request_at_unix = Some(request_at_unix);
        self.updated_at_unix = request_at_unix;
    }

    pub fn mark_connect_secret_consumed(&mut self, consumed_at_unix: u64) {
        if self.connect_secret_hash.is_none() || self.connect_secret_consumed_at_unix.is_some() {
            return;
        }
        self.connect_secret_consumed_at_unix = Some(consumed_at_unix);
        self.updated_at_unix = consumed_at_unix;
    }

    pub fn require_auth_challenge(&mut self, auth_challenge: RadrootsNostrSignerAuthChallenge) {
        self.auth_state = RadrootsNostrSignerAuthState::Pending;
        self.auth_challenge = Some(auth_challenge.clone());
        self.pending_request = None;
        self.updated_at_unix = auth_challenge.required_at_unix;
    }

    pub fn set_pending_request(&mut self, pending_request: RadrootsNostrSignerPendingRequest) {
        self.pending_request = Some(pending_request.clone());
        self.updated_at_unix = pending_request.created_at_unix;
    }

    pub fn authorize_auth_challenge(
        &mut self,
        authorized_at_unix: u64,
    ) -> Option<RadrootsNostrSignerPendingRequest> {
        self.auth_state = RadrootsNostrSignerAuthState::Authorized;
        if let Some(auth_challenge) = self.auth_challenge.as_mut() {
            auth_challenge.mark_authorized(authorized_at_unix);
        }
        self.last_authenticated_at_unix = Some(authorized_at_unix);
        self.updated_at_unix = authorized_at_unix;
        self.pending_request.take()
    }
}

impl RadrootsNostrSignerRequestAuditRecord {
    pub fn new(
        request_id: RadrootsNostrSignerRequestId,
        connection_id: RadrootsNostrSignerConnectionId,
        method: RadrootsNostrConnectMethod,
        decision: RadrootsNostrSignerRequestDecision,
        message: Option<String>,
        created_at_unix: u64,
    ) -> Self {
        Self {
            request_id,
            connection_id,
            method,
            decision,
            message,
            created_at_unix,
        }
    }
}

impl Default for RadrootsNostrSignerStoreState {
    fn default() -> Self {
        Self {
            version: RADROOTS_NOSTR_SIGNER_STORE_VERSION,
            signer_identity: None,
            connections: Vec::new(),
            audit_records: Vec::new(),
        }
    }
}

fn serialize_permission<S>(
    permission: &RadrootsNostrConnectPermission,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&permission.to_string())
}

fn deserialize_permission<'de, D>(
    deserializer: D,
) -> Result<RadrootsNostrConnectPermission, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    value.parse().map_err(serde::de::Error::custom)
}

fn deserialize_connect_secret_hash_option<'de, D>(
    deserializer: D,
) -> Result<Option<RadrootsNostrSignerConnectSecretHash>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<RadrootsNostrSignerConnectSecretHashRepr>::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(RadrootsNostrSignerConnectSecretHashRepr::Hash(hash)) => {
            hash.normalize().map(Some).map_err(serde::de::Error::custom)
        }
        Some(RadrootsNostrSignerConnectSecretHashRepr::LegacyPlaintext(secret)) => {
            Ok(RadrootsNostrSignerConnectSecretHash::from_secret(&secret))
        }
    }
}

fn normalize_optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::{Keys, SecretKey};
    use radroots_identity::RadrootsIdentity;
    use serde_json::json;
    use std::str::FromStr;
    use tempfile::tempdir;

    fn public_identity(secret_hex: &str) -> RadrootsIdentityPublic {
        RadrootsIdentity::from_secret_key_str(secret_hex)
            .expect("identity")
            .to_public()
    }

    fn public_key(secret_hex: &str) -> PublicKey {
        let secret = SecretKey::from_hex(secret_hex).expect("secret");
        Keys::new(secret).public_key()
    }

    fn request_message(id: &str) -> RadrootsNostrConnectRequestMessage {
        RadrootsNostrConnectRequestMessage::new(
            id,
            radroots_nostr_connect::prelude::RadrootsNostrConnectRequest::Ping,
        )
    }

    #[test]
    fn connection_and_request_ids_parse_and_display() {
        let connection_id = RadrootsNostrSignerConnectionId::parse("conn-1").expect("connection");
        let request_id = RadrootsNostrSignerRequestId::parse("req-1").expect("request");

        assert_eq!(connection_id.as_str(), "conn-1");
        assert_eq!(request_id.as_str(), "req-1");
        assert_eq!(connection_id.as_ref(), "conn-1");
        assert_eq!(request_id.as_ref(), "req-1");
        assert_eq!(connection_id.to_string(), "conn-1");
        assert_eq!(request_id.to_string(), "req-1");
        assert_eq!(connection_id.clone().into_string(), "conn-1");
        assert_eq!(request_id.clone().into_string(), "req-1");

        let parsed_connection =
            RadrootsNostrSignerConnectionId::from_str("conn-1").expect("from_str connection");
        let parsed_request =
            RadrootsNostrSignerRequestId::from_str("req-1").expect("from_str request");
        assert_eq!(parsed_connection, connection_id);
        assert_eq!(parsed_request, request_id);
    }

    #[test]
    fn generated_ids_are_non_empty() {
        let connection_id = RadrootsNostrSignerConnectionId::new_v7();
        let request_id = RadrootsNostrSignerRequestId::new_v7();

        assert!(!connection_id.as_ref().is_empty());
        assert!(!request_id.as_ref().is_empty());
    }

    #[test]
    fn ids_reject_empty_values() {
        let connection_err =
            RadrootsNostrSignerConnectionId::parse("   ").expect_err("empty connection");
        let request_err = RadrootsNostrSignerRequestId::parse("").expect_err("empty request");

        assert!(connection_err.to_string().contains("invalid connection id"));
        assert!(request_err.to_string().contains("invalid request id"));
    }

    #[test]
    fn connection_draft_builders_apply_values() {
        let permission = RadrootsNostrConnectPermission::with_parameter(
            RadrootsNostrConnectMethod::SignEvent,
            "kind:1",
        );
        let relay = RelayUrl::parse("wss://relay.example").expect("relay");
        let draft = RadrootsNostrSignerConnectionDraft::new(
            public_key("0000000000000000000000000000000000000000000000000000000000000001"),
            public_identity("0000000000000000000000000000000000000000000000000000000000000002"),
        )
        .with_connect_secret(" secret ")
        .with_requested_permissions(vec![permission.clone()].into())
        .with_relays(vec![relay.clone()])
        .with_approval_requirement(RadrootsNostrSignerApprovalRequirement::ExplicitUser);

        assert_eq!(draft.connect_secret.as_deref(), Some(" secret "));
        assert_eq!(draft.requested_permissions.as_slice(), &[permission]);
        assert_eq!(draft.relays, vec![relay]);
        assert_eq!(
            draft.approval_requirement,
            RadrootsNostrSignerApprovalRequirement::ExplicitUser
        );
    }

    #[test]
    fn connection_record_defaults_follow_approval_requirement_and_tracking_helpers() {
        let signer_identity =
            public_identity("0000000000000000000000000000000000000000000000000000000000000003");
        let user_identity =
            public_identity("0000000000000000000000000000000000000000000000000000000000000004");
        let connection_id = RadrootsNostrSignerConnectionId::parse("conn-1").expect("id");
        let draft = RadrootsNostrSignerConnectionDraft::new(
            public_key("0000000000000000000000000000000000000000000000000000000000000005"),
            user_identity,
        )
        .with_connect_secret(" secret ")
        .with_approval_requirement(RadrootsNostrSignerApprovalRequirement::ExplicitUser);
        let mut record =
            RadrootsNostrSignerConnectionRecord::new(connection_id, signer_identity, draft, 10);

        assert_eq!(record.status, RadrootsNostrSignerConnectionStatus::Pending);
        assert_eq!(
            record.approval_state,
            RadrootsNostrSignerApprovalState::Pending
        );
        assert_eq!(record.auth_state, RadrootsNostrSignerAuthState::NotRequired);
        assert!(
            record
                .connect_secret_hash
                .as_ref()
                .expect("connect secret hash")
                .matches_secret("secret")
        );
        assert!(!record.connect_secret_is_consumed());
        assert!(!record.is_terminal());

        record.touch_updated(12);
        record.mark_authenticated(14);
        record.mark_request(16);
        record.mark_connect_secret_consumed(17);
        record.require_auth_challenge(
            RadrootsNostrSignerAuthChallenge::new("https://auth.example/path", 18)
                .expect("auth challenge"),
        );
        record.set_pending_request(
            RadrootsNostrSignerPendingRequest::new(request_message("req-1"), 20)
                .expect("pending request"),
        );
        let replay = record.authorize_auth_challenge(22).expect("replay");
        let no_challenge_replay = RadrootsNostrSignerConnectionRecord::new(
            RadrootsNostrSignerConnectionId::parse("conn-1b").expect("id"),
            public_identity("0000000000000000000000000000000000000000000000000000000000000009"),
            RadrootsNostrSignerConnectionDraft::new(
                public_key("0000000000000000000000000000000000000000000000000000000000000010"),
                public_identity("0000000000000000000000000000000000000000000000000000000000000011"),
            ),
            24,
        )
        .authorize_auth_challenge(25);

        assert_eq!(record.updated_at_unix, 22);
        assert_eq!(record.connect_secret_consumed_at_unix, Some(17));
        assert!(record.connect_secret_is_consumed());
        assert_eq!(record.auth_state, RadrootsNostrSignerAuthState::Authorized);
        assert_eq!(
            record
                .auth_challenge
                .as_ref()
                .expect("auth challenge")
                .authorized_at_unix,
            Some(22)
        );
        assert!(record.pending_request.is_none());
        assert_eq!(record.last_authenticated_at_unix, Some(22));
        assert_eq!(record.last_request_at_unix, Some(16));
        assert_eq!(replay.request_id().as_str(), "req-1");
        assert!(no_challenge_replay.is_none());
    }

    #[test]
    fn granted_permissions_and_request_audit_build_correctly() {
        let permission = RadrootsNostrConnectPermission::new(RadrootsNostrConnectMethod::Ping);
        let grant = RadrootsNostrSignerPermissionGrant::new(permission.clone(), 42);
        let mut record = RadrootsNostrSignerConnectionRecord::new(
            RadrootsNostrSignerConnectionId::parse("conn-2").expect("id"),
            public_identity("0000000000000000000000000000000000000000000000000000000000000006"),
            RadrootsNostrSignerConnectionDraft::new(
                public_key("0000000000000000000000000000000000000000000000000000000000000007"),
                public_identity("0000000000000000000000000000000000000000000000000000000000000008"),
            ),
            20,
        );
        record.granted_permissions = vec![grant];
        let audit = RadrootsNostrSignerRequestAuditRecord::new(
            RadrootsNostrSignerRequestId::parse("req-2").expect("request"),
            RadrootsNostrSignerConnectionId::parse("conn-2").expect("id"),
            RadrootsNostrConnectMethod::Ping,
            RadrootsNostrSignerRequestDecision::Allowed,
            Some("ok".into()),
            25,
        );

        assert_eq!(record.granted_permissions().as_slice(), &[permission]);
        assert_eq!(audit.message.as_deref(), Some("ok"));
        assert_eq!(audit.created_at_unix, 25);

        let json = serde_json::to_string(&record.granted_permissions[0]).expect("serialize grant");
        let decoded: RadrootsNostrSignerPermissionGrant =
            serde_json::from_str(&json).expect("deserialize grant");
        assert_eq!(
            decoded.permission,
            RadrootsNostrConnectPermission::new(RadrootsNostrConnectMethod::Ping)
        );
    }

    #[test]
    fn effective_permissions_prefers_grants_then_auto_requested_then_empty() {
        let requested: RadrootsNostrConnectPermissions = vec![RadrootsNostrConnectPermission::new(
            RadrootsNostrConnectMethod::Nip04Encrypt,
        )]
        .into();
        let auto_record = RadrootsNostrSignerConnectionRecord::new(
            RadrootsNostrSignerConnectionId::new_v7(),
            public_identity("0000000000000000000000000000000000000000000000000000000000000031"),
            RadrootsNostrSignerConnectionDraft::new(
                public_key("0000000000000000000000000000000000000000000000000000000000000032"),
                public_identity("0000000000000000000000000000000000000000000000000000000000000033"),
            )
            .with_requested_permissions(requested.clone()),
            1,
        );
        assert_eq!(auto_record.effective_permissions(), requested);

        let mut granted_record = auto_record.clone();
        granted_record.granted_permissions = vec![RadrootsNostrSignerPermissionGrant::new(
            RadrootsNostrConnectPermission::new(RadrootsNostrConnectMethod::Ping),
            2,
        )];
        assert_eq!(
            granted_record.effective_permissions(),
            vec![RadrootsNostrConnectPermission::new(
                RadrootsNostrConnectMethod::Ping
            )]
            .into()
        );

        let mut approved_without_grants = auto_record;
        approved_without_grants.approval_state = RadrootsNostrSignerApprovalState::Approved;
        assert!(approved_without_grants.effective_permissions().is_empty());
    }

    #[test]
    fn permission_serde_helpers_round_trip_through_wrapper() {
        #[derive(Debug, Serialize, Deserialize)]
        struct PermissionWrapper {
            #[serde(
                serialize_with = "serialize_permission",
                deserialize_with = "deserialize_permission"
            )]
            permission: RadrootsNostrConnectPermission,
        }

        let wrapper = PermissionWrapper {
            permission: RadrootsNostrConnectPermission::with_parameter(
                RadrootsNostrConnectMethod::SignEvent,
                "kind:1",
            ),
        };

        let json = serde_json::to_vec_pretty(&wrapper).expect("serialize wrapper");
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("permission.json");
        std::fs::write(&path, &json).expect("write permission");
        let file = std::fs::File::open(&path).expect("open permission");
        let reader = std::io::BufReader::new(file);
        let decoded: PermissionWrapper =
            serde_json::from_reader(reader).expect("deserialize wrapper");

        assert_eq!(decoded.permission, wrapper.permission);

        let value = serde_json::to_value(&wrapper).expect("serialize wrapper to value");
        let decoded_from_value: PermissionWrapper =
            serde_json::from_value(value).expect("deserialize wrapper from value");
        assert_eq!(decoded_from_value.permission, wrapper.permission);

        let invalid = serde_json::from_str::<PermissionWrapper>(r#"{"permission":1}"#)
            .expect_err("invalid permission type");
        assert!(invalid.to_string().contains("invalid type"));

        let invalid_from_value =
            serde_json::from_value::<PermissionWrapper>(json!({ "permission": 1 }))
                .expect_err("invalid permission type from value");
        assert!(invalid_from_value.to_string().contains("invalid type"));

        let invalid_path = temp.path().join("invalid-permission.json");
        std::fs::write(&invalid_path, br#"{"permission":1}"#).expect("write invalid permission");
        let invalid_file = std::fs::File::open(&invalid_path).expect("open invalid permission");
        let invalid_reader = std::io::BufReader::new(invalid_file);
        let invalid_from_reader = serde_json::from_reader::<_, PermissionWrapper>(invalid_reader)
            .expect_err("invalid permission type from reader");
        assert!(invalid_from_reader.to_string().contains("invalid type"));
    }

    #[test]
    fn connect_secret_hash_and_pending_request_helpers_validate_inputs() {
        let hash =
            RadrootsNostrSignerConnectSecretHash::from_secret(" secret ").expect("secret hash");
        assert!(hash.matches_secret("secret"));
        assert!(!hash.matches_secret("other"));
        assert!(RadrootsNostrSignerConnectSecretHash::from_secret("   ").is_none());

        let pending = RadrootsNostrSignerPendingRequest::new(request_message("req-2"), 30)
            .expect("pending request");
        assert_eq!(pending.request_id().as_str(), "req-2");
        assert_eq!(pending.request_message().id, "req-2");

        let invalid_pending = RadrootsNostrSignerPendingRequest::new(request_message("   "), 30)
            .expect_err("invalid pending request id");
        assert!(invalid_pending.to_string().contains("invalid request id"));

        let challenge =
            RadrootsNostrSignerAuthChallenge::new(" https://auth.example ", 31).expect("challenge");
        assert_eq!(challenge.auth_url, "https://auth.example/");

        let invalid_challenge =
            RadrootsNostrSignerAuthChallenge::new("not-a-url", 31).expect_err("invalid challenge");
        assert!(invalid_challenge.to_string().contains("invalid auth url"));

        let empty_challenge =
            RadrootsNostrSignerAuthChallenge::new("   ", 31).expect_err("empty challenge");
        assert!(empty_challenge.to_string().contains("invalid auth url"));
    }

    #[test]
    fn auth_challenge_deserialize_rejects_invalid_urls_across_entrypoints() {
        let invalid_json = json!({
            "auth_url": "   ",
            "required_at_unix": 44
        });

        let invalid_from_value =
            serde_json::from_value::<RadrootsNostrSignerAuthChallenge>(invalid_json.clone())
                .expect_err("invalid auth challenge from value");
        assert!(invalid_from_value.to_string().contains("invalid auth url"));

        let invalid_from_str =
            serde_json::from_str::<RadrootsNostrSignerAuthChallenge>(&invalid_json.to_string())
                .expect_err("invalid auth challenge from str");
        assert!(invalid_from_str.to_string().contains("invalid auth url"));

        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("invalid-auth-challenge.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&invalid_json).expect("serialize invalid auth challenge"),
        )
        .expect("write invalid auth challenge");
        let file = std::fs::File::open(&path).expect("open invalid auth challenge");
        let reader = std::io::BufReader::new(file);
        let invalid_from_reader =
            serde_json::from_reader::<_, RadrootsNostrSignerAuthChallenge>(reader)
                .expect_err("invalid auth challenge from reader");
        assert!(invalid_from_reader.to_string().contains("invalid auth url"));

        let invalid_shape_json = json!({
            "auth_url": 1,
            "required_at_unix": 44
        });
        let invalid_shape_from_value =
            serde_json::from_value::<RadrootsNostrSignerAuthChallenge>(invalid_shape_json.clone())
                .expect_err("invalid auth challenge shape from value");
        assert!(
            invalid_shape_from_value
                .to_string()
                .contains("invalid type")
        );

        let invalid_shape_from_str = serde_json::from_str::<RadrootsNostrSignerAuthChallenge>(
            &invalid_shape_json.to_string(),
        )
        .expect_err("invalid auth challenge shape from str");
        assert!(invalid_shape_from_str.to_string().contains("invalid type"));

        let invalid_shape_path = temp.path().join("invalid-auth-challenge-shape.json");
        std::fs::write(
            &invalid_shape_path,
            serde_json::to_vec(&invalid_shape_json)
                .expect("serialize invalid auth challenge shape"),
        )
        .expect("write invalid auth challenge shape");
        let invalid_shape_file =
            std::fs::File::open(&invalid_shape_path).expect("open invalid auth challenge shape");
        let invalid_shape_reader = std::io::BufReader::new(invalid_shape_file);
        let invalid_shape_from_reader =
            serde_json::from_reader::<_, RadrootsNostrSignerAuthChallenge>(invalid_shape_reader)
                .expect_err("invalid auth challenge shape from reader");
        assert!(
            invalid_shape_from_reader
                .to_string()
                .contains("invalid type")
        );
    }

    #[test]
    fn connection_record_serde_migrates_legacy_connect_secret_and_validates_new_fields() {
        let record_json = json!({
            "connection_id": "conn-legacy",
            "client_public_key": public_key("0000000000000000000000000000000000000000000000000000000000000009").to_hex(),
            "signer_identity": public_identity("0000000000000000000000000000000000000000000000000000000000000010"),
            "user_identity": public_identity("0000000000000000000000000000000000000000000000000000000000000011"),
            "connect_secret": " legacy-secret ",
            "requested_permissions": "",
            "granted_permissions": [],
            "relays": [],
            "approval_requirement": "NotRequired",
            "approval_state": "NotRequired",
            "status": "Active",
            "status_reason": null,
            "created_at_unix": 1,
            "updated_at_unix": 1,
            "last_authenticated_at_unix": null,
            "last_request_at_unix": null
        });

        let decoded_without_secret: RadrootsNostrSignerConnectionRecord = serde_json::from_value(
            json!({
                "connection_id": "conn-no-secret",
                "client_public_key": public_key("0000000000000000000000000000000000000000000000000000000000000008").to_hex(),
                "signer_identity": public_identity("0000000000000000000000000000000000000000000000000000000000000007"),
                "user_identity": public_identity("0000000000000000000000000000000000000000000000000000000000000006"),
                "requested_permissions": "",
                "granted_permissions": [],
                "relays": [],
                "approval_requirement": "NotRequired",
                "approval_state": "NotRequired",
                "status": "Active",
                "created_at_unix": 0,
                "updated_at_unix": 0,
                "last_authenticated_at_unix": null,
                "last_request_at_unix": null
            }),
        )
        .expect("deserialize record without secret");
        assert!(decoded_without_secret.connect_secret_hash.is_none());
        assert!(
            decoded_without_secret
                .connect_secret_consumed_at_unix
                .is_none()
        );

        let decoded_with_null_secret: RadrootsNostrSignerConnectionRecord = serde_json::from_value(
            json!({
                "connection_id": "conn-null-secret",
                "client_public_key": public_key("0000000000000000000000000000000000000000000000000000000000000005").to_hex(),
                "signer_identity": public_identity("0000000000000000000000000000000000000000000000000000000000000004"),
                "user_identity": public_identity("0000000000000000000000000000000000000000000000000000000000000003"),
                "connect_secret_hash": null,
                "requested_permissions": "",
                "granted_permissions": [],
                "relays": [],
                "approval_requirement": "NotRequired",
                "approval_state": "NotRequired",
                "status": "Active",
                "created_at_unix": 0,
                "updated_at_unix": 0,
                "last_authenticated_at_unix": null,
                "last_request_at_unix": null
            }),
        )
        .expect("deserialize record with null secret");
        assert!(decoded_with_null_secret.connect_secret_hash.is_none());
        assert!(
            decoded_with_null_secret
                .connect_secret_consumed_at_unix
                .is_none()
        );

        let decoded: RadrootsNostrSignerConnectionRecord =
            serde_json::from_value(record_json).expect("deserialize legacy record");
        assert!(
            decoded
                .connect_secret_hash
                .as_ref()
                .expect("connect secret hash")
                .matches_secret("legacy-secret")
        );

        let encoded = serde_json::to_value(&decoded).expect("serialize record");
        assert!(encoded.get("connect_secret").is_none());
        assert!(encoded.get("connect_secret_hash").is_some());
        assert!(encoded.get("connect_secret_consumed_at_unix").is_none());
        assert_eq!(
            encoded
                .get("auth_state")
                .and_then(serde_json::Value::as_str),
            Some("NotRequired")
        );

        let valid_hash = RadrootsNostrSignerConnectSecretHash::from_secret("explicit-secret")
            .expect("valid hash");
        let decoded_new_format: RadrootsNostrSignerConnectionRecord = serde_json::from_value(
            json!({
                "connection_id": "conn-new",
                "client_public_key": public_key("0000000000000000000000000000000000000000000000000000000000000015").to_hex(),
                "signer_identity": public_identity("0000000000000000000000000000000000000000000000000000000000000016"),
                "user_identity": public_identity("0000000000000000000000000000000000000000000000000000000000000017"),
                "connect_secret_hash": {
                    "algorithm": "sha256",
                    "digest_hex": valid_hash.digest_hex
                },
                "connect_secret_consumed_at_unix": 23,
                "requested_permissions": "",
                "granted_permissions": [],
                "relays": [],
                "approval_requirement": "NotRequired",
                "approval_state": "NotRequired",
                "status": "Active",
                "created_at_unix": 3,
                "updated_at_unix": 3,
                "last_authenticated_at_unix": null,
                "last_request_at_unix": null
            }),
        )
        .expect("deserialize new-format record");
        assert!(
            decoded_new_format
                .connect_secret_hash
                .as_ref()
                .expect("new-format hash")
                .matches_secret("explicit-secret")
        );
        assert_eq!(decoded_new_format.connect_secret_consumed_at_unix, Some(23));
        assert!(decoded_new_format.connect_secret_is_consumed());

        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("connection-record.json");
        let reader_json = json!({
            "connection_id": "conn-reader",
            "client_public_key": public_key("0000000000000000000000000000000000000000000000000000000000000021").to_hex(),
            "signer_identity": public_identity("0000000000000000000000000000000000000000000000000000000000000022"),
            "user_identity": public_identity("0000000000000000000000000000000000000000000000000000000000000023"),
            "connect_secret_hash": {
                "algorithm": "sha256",
                "digest_hex": RadrootsNostrSignerConnectSecretHash::from_secret("reader-secret")
                    .expect("reader hash")
                    .digest_hex
            },
            "requested_permissions": "",
            "granted_permissions": [],
            "relays": [],
            "approval_requirement": "NotRequired",
            "approval_state": "NotRequired",
            "auth_state": "Pending",
            "auth_challenge": {
                "auth_url": "https://auth.example/reader",
                "required_at_unix": 5
            },
            "status": "Active",
            "created_at_unix": 5,
            "updated_at_unix": 5,
            "last_authenticated_at_unix": null,
            "last_request_at_unix": null
        });
        std::fs::write(
            &path,
            serde_json::to_vec(&reader_json).expect("serialize reader json"),
        )
        .expect("write reader json");
        let file = std::fs::File::open(&path).expect("open reader json");
        let reader = std::io::BufReader::new(file);
        let decoded_from_reader: RadrootsNostrSignerConnectionRecord =
            serde_json::from_reader(reader).expect("deserialize reader record");
        assert!(
            decoded_from_reader
                .connect_secret_hash
                .as_ref()
                .expect("reader hash")
                .matches_secret("reader-secret")
        );
        assert_eq!(
            decoded_from_reader
                .auth_challenge
                .as_ref()
                .expect("reader auth challenge")
                .auth_url,
            "https://auth.example/reader"
        );

        let invalid_hash_json = json!({
            "connection_id": "conn-invalid",
            "client_public_key": public_key("0000000000000000000000000000000000000000000000000000000000000012").to_hex(),
            "signer_identity": public_identity("0000000000000000000000000000000000000000000000000000000000000013"),
            "user_identity": public_identity("0000000000000000000000000000000000000000000000000000000000000014"),
            "connect_secret_hash": {
                "algorithm": "sha256",
                "digest_hex": "not-hex"
            },
            "requested_permissions": "",
            "granted_permissions": [],
            "relays": [],
            "approval_requirement": "NotRequired",
            "approval_state": "NotRequired",
            "status": "Active",
            "auth_state": "Authorized",
            "auth_challenge": {
                "auth_url": "https://auth.example",
                "required_at_unix": 2
            },
            "status_reason": null,
            "created_at_unix": 2,
            "updated_at_unix": 2,
            "last_authenticated_at_unix": null,
            "last_request_at_unix": null
        });
        let invalid_hash =
            serde_json::from_value::<RadrootsNostrSignerConnectionRecord>(invalid_hash_json)
                .expect_err("invalid hash");
        assert!(
            invalid_hash
                .to_string()
                .contains("invalid connect secret digest")
        );

        let invalid_nonhex_hash = serde_json::from_value::<RadrootsNostrSignerConnectionRecord>(
            json!({
                "connection_id": "conn-invalid-nonhex",
                "client_public_key": public_key("0000000000000000000000000000000000000000000000000000000000000018").to_hex(),
                "signer_identity": public_identity("0000000000000000000000000000000000000000000000000000000000000019"),
                "user_identity": public_identity("0000000000000000000000000000000000000000000000000000000000000020"),
                "connect_secret_hash": {
                    "algorithm": "sha256",
                    "digest_hex": "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"
                },
                "requested_permissions": "",
                "granted_permissions": [],
                "relays": [],
                "approval_requirement": "NotRequired",
                "approval_state": "NotRequired",
                "status": "Active",
                "created_at_unix": 4,
                "updated_at_unix": 4,
                "last_authenticated_at_unix": null,
                "last_request_at_unix": null
            }),
        )
        .expect_err("invalid nonhex hash");
        assert!(
            invalid_nonhex_hash
                .to_string()
                .contains("invalid connect secret digest")
        );

        let invalid_connect_secret_hash_type =
            serde_json::from_value::<RadrootsNostrSignerConnectionRecord>(json!({
                "connection_id": "conn-invalid-type",
                "client_public_key": public_key("0000000000000000000000000000000000000000000000000000000000000024").to_hex(),
                "signer_identity": public_identity("0000000000000000000000000000000000000000000000000000000000000025"),
                "user_identity": public_identity("0000000000000000000000000000000000000000000000000000000000000026"),
                "connect_secret_hash": 7,
                "requested_permissions": "",
                "granted_permissions": [],
                "relays": [],
                "approval_requirement": "NotRequired",
                "approval_state": "NotRequired",
                "status": "Active",
                "created_at_unix": 6,
                "updated_at_unix": 6,
                "last_authenticated_at_unix": null,
                "last_request_at_unix": null
            }))
            .expect_err("invalid connect secret hash type");
        assert!(!invalid_connect_secret_hash_type.to_string().is_empty());

        let invalid_connect_secret_hash_path = temp.path().join("invalid-connect-secret-type.json");
        std::fs::write(
            &invalid_connect_secret_hash_path,
            serde_json::to_vec(&json!({
                "connection_id": "conn-invalid-type-reader",
                "client_public_key": public_key("0000000000000000000000000000000000000000000000000000000000000027").to_hex(),
                "signer_identity": public_identity("0000000000000000000000000000000000000000000000000000000000000028"),
                "user_identity": public_identity("0000000000000000000000000000000000000000000000000000000000000029"),
                "connect_secret_hash": 9,
                "requested_permissions": "",
                "granted_permissions": [],
                "relays": [],
                "approval_requirement": "NotRequired",
                "approval_state": "NotRequired",
                "status": "Active",
                "created_at_unix": 7,
                "updated_at_unix": 7,
                "last_authenticated_at_unix": null,
                "last_request_at_unix": null
            }))
            .expect("serialize invalid connect secret hash type"),
        )
        .expect("write invalid connect secret hash type");
        let invalid_connect_secret_hash_file =
            std::fs::File::open(&invalid_connect_secret_hash_path)
                .expect("open invalid connect secret hash type");
        let invalid_connect_secret_hash_reader =
            std::io::BufReader::new(invalid_connect_secret_hash_file);
        let invalid_connect_secret_hash_from_reader = serde_json::from_reader::<
            _,
            RadrootsNostrSignerConnectionRecord,
        >(invalid_connect_secret_hash_reader)
        .expect_err("invalid connect secret hash type from reader");
        assert!(
            !invalid_connect_secret_hash_from_reader
                .to_string()
                .is_empty()
        );
    }

    #[test]
    fn store_state_default_is_empty() {
        let state = RadrootsNostrSignerStoreState::default();
        assert_eq!(state.version, RADROOTS_NOSTR_SIGNER_STORE_VERSION);
        assert!(state.signer_identity.is_none());
        assert!(state.connections.is_empty());
        assert!(state.audit_records.is_empty());
    }
}

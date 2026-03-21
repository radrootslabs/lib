use crate::error::RadrootsNostrSignerError;
use nostr::{PublicKey, RelayUrl};
use radroots_identity::RadrootsIdentityPublic;
use radroots_nostr_connect::prelude::{
    RadrootsNostrConnectMethod, RadrootsNostrConnectPermission, RadrootsNostrConnectPermissions,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connect_secret: Option<String>,
    pub requested_permissions: RadrootsNostrConnectPermissions,
    #[serde(default)]
    pub granted_permissions: Vec<RadrootsNostrSignerPermissionGrant>,
    #[serde(default)]
    pub relays: Vec<RelayUrl>,
    pub approval_requirement: RadrootsNostrSignerApprovalRequirement,
    pub approval_state: RadrootsNostrSignerApprovalState,
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
            connect_secret: draft.connect_secret,
            requested_permissions: draft.requested_permissions,
            granted_permissions: Vec::new(),
            relays: draft.relays,
            approval_requirement: draft.approval_requirement,
            approval_state,
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

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            RadrootsNostrSignerConnectionStatus::Rejected
                | RadrootsNostrSignerConnectionStatus::Revoked
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::{Keys, SecretKey};
    use radroots_identity::RadrootsIdentity;
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
        .with_approval_requirement(RadrootsNostrSignerApprovalRequirement::ExplicitUser);
        let mut record =
            RadrootsNostrSignerConnectionRecord::new(connection_id, signer_identity, draft, 10);

        assert_eq!(record.status, RadrootsNostrSignerConnectionStatus::Pending);
        assert_eq!(
            record.approval_state,
            RadrootsNostrSignerApprovalState::Pending
        );
        assert!(!record.is_terminal());

        record.touch_updated(12);
        record.mark_authenticated(14);
        record.mark_request(16);

        assert_eq!(record.updated_at_unix, 16);
        assert_eq!(record.last_authenticated_at_unix, Some(14));
        assert_eq!(record.last_request_at_unix, Some(16));
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

        let invalid = serde_json::from_str::<PermissionWrapper>(r#"{"permission":1}"#)
            .expect_err("invalid permission type");
        assert!(invalid.to_string().contains("invalid type"));
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

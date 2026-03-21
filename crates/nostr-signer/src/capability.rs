use crate::model::{RadrootsNostrSignerConnectionId, RadrootsNostrSignerConnectionRecord};
use nostr::RelayUrl;
use radroots_identity::{RadrootsIdentityId, RadrootsIdentityPublic};
use radroots_nostr_connect::prelude::RadrootsNostrConnectPermissions;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RadrootsNostrLocalSignerAvailability {
    PublicOnly,
    SecretBacked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadrootsNostrLocalSignerCapability {
    pub account_id: RadrootsIdentityId,
    pub public_identity: RadrootsIdentityPublic,
    pub availability: RadrootsNostrLocalSignerAvailability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadrootsNostrRemoteSessionSignerCapability {
    pub connection_id: RadrootsNostrSignerConnectionId,
    pub signer_identity: RadrootsIdentityPublic,
    pub user_identity: RadrootsIdentityPublic,
    pub relays: Vec<RelayUrl>,
    pub permissions: RadrootsNostrConnectPermissions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RadrootsNostrSignerCapability {
    LocalAccount(RadrootsNostrLocalSignerCapability),
    RemoteSession(RadrootsNostrRemoteSessionSignerCapability),
}

fn public_identity_eq(left: &RadrootsIdentityPublic, right: &RadrootsIdentityPublic) -> bool {
    left.id == right.id
        && left.public_key_hex == right.public_key_hex
        && left.public_key_npub == right.public_key_npub
}

impl RadrootsNostrLocalSignerCapability {
    pub fn new(
        account_id: RadrootsIdentityId,
        public_identity: RadrootsIdentityPublic,
        availability: RadrootsNostrLocalSignerAvailability,
    ) -> Self {
        Self {
            account_id,
            public_identity,
            availability,
        }
    }

    pub fn is_secret_backed(&self) -> bool {
        self.availability == RadrootsNostrLocalSignerAvailability::SecretBacked
    }
}

impl RadrootsNostrRemoteSessionSignerCapability {
    pub fn new(
        connection_id: RadrootsNostrSignerConnectionId,
        signer_identity: RadrootsIdentityPublic,
        user_identity: RadrootsIdentityPublic,
    ) -> Self {
        Self {
            connection_id,
            signer_identity,
            user_identity,
            relays: Vec::new(),
            permissions: RadrootsNostrConnectPermissions::default(),
        }
    }

    pub fn with_relays(mut self, relays: Vec<RelayUrl>) -> Self {
        self.relays = relays;
        self
    }

    pub fn with_permissions(mut self, permissions: RadrootsNostrConnectPermissions) -> Self {
        self.permissions = permissions;
        self
    }
}

impl RadrootsNostrSignerCapability {
    pub fn public_identity(&self) -> &RadrootsIdentityPublic {
        match self {
            Self::LocalAccount(capability) => &capability.public_identity,
            Self::RemoteSession(capability) => &capability.user_identity,
        }
    }

    pub fn local_account(&self) -> Option<&RadrootsNostrLocalSignerCapability> {
        match self {
            Self::LocalAccount(capability) => Some(capability),
            Self::RemoteSession(_) => None,
        }
    }

    pub fn remote_session(&self) -> Option<&RadrootsNostrRemoteSessionSignerCapability> {
        match self {
            Self::RemoteSession(capability) => Some(capability),
            Self::LocalAccount(_) => None,
        }
    }
}

impl PartialEq for RadrootsNostrLocalSignerCapability {
    fn eq(&self, other: &Self) -> bool {
        self.account_id == other.account_id
            && self.availability == other.availability
            && public_identity_eq(&self.public_identity, &other.public_identity)
    }
}

impl Eq for RadrootsNostrLocalSignerCapability {}

impl PartialEq for RadrootsNostrRemoteSessionSignerCapability {
    fn eq(&self, other: &Self) -> bool {
        self.connection_id == other.connection_id
            && self.relays == other.relays
            && self.permissions == other.permissions
            && public_identity_eq(&self.signer_identity, &other.signer_identity)
            && public_identity_eq(&self.user_identity, &other.user_identity)
    }
}

impl Eq for RadrootsNostrRemoteSessionSignerCapability {}

impl PartialEq for RadrootsNostrSignerCapability {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::LocalAccount(left), Self::LocalAccount(right)) => left == right,
            (Self::RemoteSession(left), Self::RemoteSession(right)) => left == right,
            _ => false,
        }
    }
}

impl Eq for RadrootsNostrSignerCapability {}

impl From<&RadrootsNostrSignerConnectionRecord> for RadrootsNostrRemoteSessionSignerCapability {
    fn from(value: &RadrootsNostrSignerConnectionRecord) -> Self {
        Self {
            connection_id: value.connection_id.clone(),
            signer_identity: value.signer_identity.clone(),
            user_identity: value.user_identity.clone(),
            relays: value.relays.clone(),
            permissions: value.effective_permissions(),
        }
    }
}

impl RadrootsNostrSignerConnectionRecord {
    pub fn remote_session_capability(&self) -> RadrootsNostrSignerCapability {
        RadrootsNostrSignerCapability::RemoteSession(
            RadrootsNostrRemoteSessionSignerCapability::from(self),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{RadrootsNostrSignerConnectionDraft, RadrootsNostrSignerConnectionRecord};
    use nostr::{Keys, PublicKey, RelayUrl, SecretKey};
    use radroots_identity::{RadrootsIdentity, RadrootsIdentityPublic};
    use radroots_nostr_connect::prelude::{
        RadrootsNostrConnectMethod, RadrootsNostrConnectPermission,
    };

    fn public_identity(secret_hex: &str) -> RadrootsIdentityPublic {
        RadrootsIdentity::from_secret_key_str(secret_hex)
            .expect("identity")
            .to_public()
    }

    fn public_key(secret_hex: &str) -> PublicKey {
        let secret = SecretKey::from_hex(secret_hex).expect("secret");
        Keys::new(secret).public_key()
    }

    fn assert_public_identity_matches(
        actual: &RadrootsIdentityPublic,
        expected: &RadrootsIdentityPublic,
    ) {
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.public_key_hex, expected.public_key_hex);
        assert_eq!(actual.public_key_npub, expected.public_key_npub);
    }

    #[test]
    fn local_capability_reports_secret_backing_and_public_identity() {
        let public_identity =
            public_identity("0000000000000000000000000000000000000000000000000000000000000001");
        let capability =
            RadrootsNostrSignerCapability::LocalAccount(RadrootsNostrLocalSignerCapability::new(
                public_identity.id.clone(),
                public_identity.clone(),
                RadrootsNostrLocalSignerAvailability::SecretBacked,
            ));

        assert_public_identity_matches(capability.public_identity(), &public_identity);
        assert!(
            capability
                .local_account()
                .expect("local capability")
                .is_secret_backed()
        );
        assert!(capability.remote_session().is_none());
    }

    #[test]
    fn remote_session_capability_reflects_connection_effective_permissions() {
        let signer_identity =
            public_identity("0000000000000000000000000000000000000000000000000000000000000002");
        let user_identity =
            public_identity("0000000000000000000000000000000000000000000000000000000000000003");
        let record = RadrootsNostrSignerConnectionRecord::new(
            RadrootsNostrSignerConnectionId::new_v7(),
            signer_identity.clone(),
            RadrootsNostrSignerConnectionDraft::new(
                public_key("0000000000000000000000000000000000000000000000000000000000000004"),
                user_identity.clone(),
            )
            .with_requested_permissions(
                vec![RadrootsNostrConnectPermission::new(
                    RadrootsNostrConnectMethod::Ping,
                )]
                .into(),
            )
            .with_relays(vec![RelayUrl::parse("wss://relay.example").expect("relay")]),
            1,
        );

        let capability = record.remote_session_capability();
        assert_public_identity_matches(capability.public_identity(), &user_identity);
        let remote = capability.remote_session().expect("remote capability");
        assert_eq!(remote.connection_id, record.connection_id);
        assert_public_identity_matches(&remote.signer_identity, &signer_identity);
        assert_public_identity_matches(&remote.user_identity, &user_identity);
        assert_eq!(remote.permissions, record.effective_permissions());
        assert_eq!(remote.relays, record.relays);
    }

    #[test]
    fn remote_session_builder_helpers_replace_default_fields() {
        let capability = RadrootsNostrRemoteSessionSignerCapability::new(
            RadrootsNostrSignerConnectionId::new_v7(),
            public_identity("0000000000000000000000000000000000000000000000000000000000000005"),
            public_identity("0000000000000000000000000000000000000000000000000000000000000006"),
        )
        .with_permissions(
            vec![RadrootsNostrConnectPermission::new(
                RadrootsNostrConnectMethod::SwitchRelays,
            )]
            .into(),
        )
        .with_relays(vec![RelayUrl::parse("wss://relay.example").expect("relay")]);

        assert_eq!(capability.permissions.as_slice().len(), 1);
        assert_eq!(capability.relays.len(), 1);
    }
}

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
    use crate::test_support::{
        fixture_alice_identity, fixture_bob_identity, fixture_carol_identity,
        fixture_diego_public_key, primary_relay, secondary_relay,
    };
    use radroots_identity::RadrootsIdentityPublic;
    use radroots_nostr_connect::prelude::{
        RadrootsNostrConnectMethod, RadrootsNostrConnectPermission,
    };

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
        let public_identity = fixture_alice_identity();
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
        let signer_identity = fixture_bob_identity();
        let user_identity = fixture_carol_identity();
        let record = RadrootsNostrSignerConnectionRecord::new(
            RadrootsNostrSignerConnectionId::new_v7(),
            signer_identity.clone(),
            RadrootsNostrSignerConnectionDraft::new(
                fixture_diego_public_key(),
                user_identity.clone(),
            )
            .with_requested_permissions(
                vec![RadrootsNostrConnectPermission::new(
                    RadrootsNostrConnectMethod::Ping,
                )]
                .into(),
            )
            .with_relays(vec![primary_relay()]),
            1,
        );

        let capability = record.remote_session_capability();
        assert_public_identity_matches(capability.public_identity(), &user_identity);
        assert!(capability.local_account().is_none());
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
            fixture_alice_identity(),
            fixture_bob_identity(),
        )
        .with_permissions(
            vec![RadrootsNostrConnectPermission::new(
                RadrootsNostrConnectMethod::SwitchRelays,
            )]
            .into(),
        )
        .with_relays(vec![primary_relay()]);

        assert_eq!(capability.permissions.as_slice().len(), 1);
        assert_eq!(capability.relays.len(), 1);
    }

    #[test]
    fn capability_equality_accounts_for_identity_fields_and_variant_kind() {
        let alice = fixture_alice_identity();
        let mut alice_with_different_hex = alice.clone();
        alice_with_different_hex.public_key_hex = fixture_bob_identity().public_key_hex;
        let mut alice_with_different_npub = alice.clone();
        alice_with_different_npub.public_key_npub = fixture_bob_identity().public_key_npub;

        let local = RadrootsNostrLocalSignerCapability::new(
            alice.id.clone(),
            alice.clone(),
            RadrootsNostrLocalSignerAvailability::SecretBacked,
        );
        let local_same = RadrootsNostrLocalSignerCapability::new(
            alice.id.clone(),
            alice.clone(),
            RadrootsNostrLocalSignerAvailability::SecretBacked,
        );
        let local_changed_account = RadrootsNostrLocalSignerCapability::new(
            fixture_bob_identity().id,
            alice.clone(),
            RadrootsNostrLocalSignerAvailability::SecretBacked,
        );
        let local_changed_availability = RadrootsNostrLocalSignerCapability::new(
            alice.id.clone(),
            alice.clone(),
            RadrootsNostrLocalSignerAvailability::PublicOnly,
        );
        let local_changed_hex = RadrootsNostrLocalSignerCapability::new(
            alice.id.clone(),
            alice_with_different_hex,
            RadrootsNostrLocalSignerAvailability::SecretBacked,
        );
        let local_changed = RadrootsNostrLocalSignerCapability::new(
            alice.id.clone(),
            alice_with_different_npub,
            RadrootsNostrLocalSignerAvailability::SecretBacked,
        );
        assert_eq!(local, local_same);
        assert_ne!(local, local_changed_account);
        assert_ne!(local, local_changed_availability);
        assert_ne!(local, local_changed_hex);
        assert_ne!(local, local_changed);

        let remote = RadrootsNostrRemoteSessionSignerCapability::new(
            RadrootsNostrSignerConnectionId::new_v7(),
            fixture_bob_identity(),
            fixture_carol_identity(),
        )
        .with_relays(vec![primary_relay()]);
        let remote_same = remote.clone();
        let remote_changed_connection = RadrootsNostrRemoteSessionSignerCapability::new(
            RadrootsNostrSignerConnectionId::new_v7(),
            remote.signer_identity.clone(),
            remote.user_identity.clone(),
        )
        .with_relays(remote.relays.clone())
        .with_permissions(remote.permissions.clone());
        let remote_changed_relays = remote.clone().with_relays(vec![secondary_relay()]);
        let remote_changed_permissions = remote.clone().with_permissions(
            vec![RadrootsNostrConnectPermission::new(
                RadrootsNostrConnectMethod::Ping,
            )]
            .into(),
        );
        let mut remote_changed_signer = remote.clone();
        remote_changed_signer.signer_identity.public_key_hex =
            fixture_alice_identity().public_key_hex;
        let mut remote_changed = remote.clone();
        remote_changed.user_identity.public_key_npub = fixture_alice_identity().public_key_npub;
        assert_eq!(remote, remote_same);
        assert_ne!(remote, remote_changed_connection);
        assert_ne!(remote, remote_changed_relays);
        assert_ne!(remote, remote_changed_permissions);
        assert_ne!(remote, remote_changed_signer);
        assert_ne!(remote, remote_changed);

        assert_eq!(
            RadrootsNostrSignerCapability::LocalAccount(local.clone()),
            RadrootsNostrSignerCapability::LocalAccount(local_same)
        );
        assert_eq!(
            RadrootsNostrSignerCapability::RemoteSession(remote.clone()),
            RadrootsNostrSignerCapability::RemoteSession(remote)
        );
        assert_ne!(
            RadrootsNostrSignerCapability::LocalAccount(local),
            RadrootsNostrSignerCapability::RemoteSession(remote_changed)
        );
    }
}

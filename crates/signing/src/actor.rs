//! Actor provenance and authorization context.
//!
//! This module binds identity-owned public values to event-owned author roles.
//! It describes host selection and provenance but does not select accounts,
//! acquire keys, or prove that a host granted a role.

use radroots_event::contract::AuthorRole;
use radroots_identity::{AccountId, PublicKey};

use crate::{Error, error::Kind};

#[cfg(not(feature = "std"))]
use alloc::collections::BTreeSet;
#[cfg(feature = "std")]
use std::collections::BTreeSet;

/// Why a host supplied an actor to a signing operation.
///
/// Account-backed variants carry the canonical public [`AccountId`]. The
/// account identifier must represent the same public bytes as the actor key.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorSource {
    /// The host resolved its explicitly selected local account.
    LocalAccount(AccountId),
    /// The caller supplied a public key without selecting an account.
    ExplicitPublicKey,
    /// The host resolved an account backed by a remote signer.
    RemoteSigner(AccountId),
    /// A service account was selected by explicit host policy.
    Service(AccountId),
}

impl ActorSource {
    /// Returns the selected account identifier for account-backed provenance.
    #[must_use]
    pub const fn account_id(self) -> Option<AccountId> {
        match self {
            Self::LocalAccount(account_id)
            | Self::RemoteSigner(account_id)
            | Self::Service(account_id) => Some(account_id),
            Self::ExplicitPublicKey => None,
        }
    }
}

/// Declarative host selection for resolving an actor.
///
/// Resolution is deliberately outside this package; this value carries no
/// database, keyring, UI, or process-global selection authority.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorSelector {
    /// Resolve the account already selected by explicit host policy.
    SelectedAccount,
    /// Resolve one canonical public account.
    Account(AccountId),
    /// Use one explicit canonical public key.
    PublicKey(PublicKey),
    /// Resolve the public key frozen into the authored event plan.
    PlanAuthorPublicKey,
}

/// Inputs a host must satisfy when resolving an actor for an authored plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActorResolutionRequest {
    selector: ActorSelector,
    required_role: AuthorRole,
    expected_public_key: PublicKey,
}

impl ActorResolutionRequest {
    /// Creates an actor-resolution request from canonical public values.
    #[must_use]
    pub const fn new(
        selector: ActorSelector,
        required_role: AuthorRole,
        expected_public_key: PublicKey,
    ) -> Self {
        Self {
            selector,
            required_role,
            expected_public_key,
        }
    }

    /// Returns the host-owned selection instruction.
    #[must_use]
    pub const fn selector(&self) -> ActorSelector {
        self.selector
    }

    /// Returns the event-contract role the resolved actor must hold.
    #[must_use]
    pub const fn required_role(&self) -> AuthorRole {
        self.required_role
    }

    /// Returns the exact public key frozen into the plan.
    #[must_use]
    pub const fn expected_public_key(&self) -> PublicKey {
        self.expected_public_key
    }
}

/// An actor-role claim and its public provenance.
///
/// Construction validates that account-backed provenance and the public key
/// describe the same identity. Role authorization against an event contract
/// is performed by the signing boundary before invoking a signer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Actor {
    public_key: PublicKey,
    roles: BTreeSet<AuthorRole>,
    source: ActorSource,
}

impl Actor {
    /// Creates actor provenance from canonical public values.
    pub fn new<I>(public_key: PublicKey, source: ActorSource, roles: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = AuthorRole>,
    {
        if let Some(account_id) = source.account_id()
            && account_id.as_bytes() != public_key.as_bytes()
        {
            return Err(Error::new(Kind::InvalidArgument));
        }
        Ok(Self {
            public_key,
            roles: roles.into_iter().collect(),
            source,
        })
    }

    /// Parses a canonical public key and creates validated actor provenance.
    pub fn from_public_key_hex<I>(
        public_key: &str,
        source: ActorSource,
        roles: I,
    ) -> Result<Self, Error>
    where
        I: IntoIterator<Item = AuthorRole>,
    {
        let public_key =
            PublicKey::from_hex(public_key).map_err(|_| Error::new(Kind::InvalidArgument))?;
        Self::new(public_key, source, roles)
    }

    /// Returns the canonical expected author key.
    #[must_use]
    pub const fn public_key(&self) -> PublicKey {
        self.public_key
    }

    /// Returns the claimed event-author roles.
    #[must_use]
    pub const fn roles(&self) -> &BTreeSet<AuthorRole> {
        &self.roles
    }

    /// Returns the host-supplied actor provenance.
    #[must_use]
    pub const fn source(&self) -> ActorSource {
        self.source
    }

    /// Returns the selected public account, when provenance is account-backed.
    #[must_use]
    pub const fn account_id(&self) -> Option<AccountId> {
        self.source.account_id()
    }

    /// Reports whether the actor claims the required event-author role.
    #[must_use]
    pub fn satisfies(&self, required_role: AuthorRole) -> bool {
        required_role == AuthorRole::Any || self.roles.contains(&required_role)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
    const BOB: &str = "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";

    fn public_key(value: &str) -> PublicKey {
        PublicKey::from_hex(value).expect("valid public key")
    }

    fn account_id(value: &str) -> AccountId {
        AccountId::from_hex(value).expect("valid account ID")
    }

    #[test]
    fn roles_and_explicit_provenance_are_preserved() {
        let actor = Actor::new(
            public_key(ALICE),
            ActorSource::ExplicitPublicKey,
            [AuthorRole::Farmer, AuthorRole::Seller],
        )
        .expect("actor");

        assert_eq!(actor.public_key(), public_key(ALICE));
        assert_eq!(actor.source(), ActorSource::ExplicitPublicKey);
        assert_eq!(actor.account_id(), None);
        assert!(actor.satisfies(AuthorRole::Any));
        assert!(actor.satisfies(AuthorRole::Farmer));
        assert!(actor.satisfies(AuthorRole::Seller));
        assert!(!actor.satisfies(AuthorRole::Buyer));
    }

    #[test]
    fn account_provenance_requires_the_same_public_identity() {
        for source in [
            ActorSource::LocalAccount(account_id(ALICE)),
            ActorSource::RemoteSigner(account_id(ALICE)),
            ActorSource::Service(account_id(ALICE)),
        ] {
            let actor = Actor::new(public_key(ALICE), source, [AuthorRole::Service])
                .expect("matching account provenance");
            assert_eq!(actor.account_id(), Some(account_id(ALICE)));
        }

        let error = Actor::new(
            public_key(ALICE),
            ActorSource::LocalAccount(account_id(BOB)),
            [AuthorRole::Farmer],
        )
        .expect_err("mismatched account must fail");
        assert_eq!(error.kind(), Kind::InvalidArgument);
    }

    #[test]
    fn invalid_public_key_text_is_rejected() {
        let error =
            Actor::from_public_key_hex("not-a-public-key", ActorSource::ExplicitPublicKey, [])
                .expect_err("invalid public key must fail");
        assert_eq!(error.kind(), Kind::InvalidArgument);
    }

    #[test]
    fn resolution_request_preserves_selection_role_and_expected_key() {
        let request = ActorResolutionRequest::new(
            ActorSelector::Account(account_id(ALICE)),
            AuthorRole::Seller,
            public_key(ALICE),
        );

        assert_eq!(
            request.selector(),
            ActorSelector::Account(account_id(ALICE))
        );
        assert_eq!(request.required_role(), AuthorRole::Seller);
        assert_eq!(request.expected_public_key(), public_key(ALICE));
    }
}

#![forbid(unsafe_code)]

use crate::RadrootsAuthorityError;
use radroots_events::contract::RadrootsActorRole;
use radroots_events::ids::RadrootsPublicKey;

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeSet, string::String};
#[cfg(feature = "std")]
use std::{collections::BTreeSet, string::String};

pub const MAX_ACTOR_ACCOUNT_ID_LEN: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsActorSource {
    LocalAccount,
    ExplicitPubkey,
    RemoteSigner,
    Service,
    Test,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsActorSelector {
    SelectedAccount,
    AccountId(String),
    PublicKey(RadrootsPublicKey),
    DraftExpectedPubkey,
}

impl RadrootsActorSelector {
    pub fn account_id(account_id: impl Into<String>) -> Result<Self, RadrootsAuthorityError> {
        Ok(Self::AccountId(validate_account_id(account_id)?))
    }

    pub fn public_key(pubkey: impl AsRef<str>) -> Result<Self, RadrootsAuthorityError> {
        let pubkey = RadrootsPublicKey::parse(pubkey.as_ref())
            .map_err(|_| RadrootsAuthorityError::InvalidActorPubkey)?;
        Ok(Self::PublicKey(pubkey))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsActorResolutionRequest {
    pub selector: RadrootsActorSelector,
    pub required_role: RadrootsActorRole,
    pub contract_id: Option<String>,
}

impl RadrootsActorResolutionRequest {
    pub fn new(
        selector: RadrootsActorSelector,
        required_role: RadrootsActorRole,
        contract_id: Option<String>,
    ) -> Self {
        Self {
            selector,
            required_role,
            contract_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsActorContext {
    pub pubkey: RadrootsPublicKey,
    pub roles: BTreeSet<RadrootsActorRole>,
    pub account_id: Option<String>,
    pub source: RadrootsActorSource,
}

impl RadrootsActorContext {
    pub fn explicit_pubkey<I>(
        pubkey: impl AsRef<str>,
        roles: I,
    ) -> Result<Self, RadrootsAuthorityError>
    where
        I: IntoIterator<Item = RadrootsActorRole>,
    {
        Self::with_provenance(pubkey, None, RadrootsActorSource::ExplicitPubkey, roles)
    }

    pub fn local_account<I>(
        pubkey: impl AsRef<str>,
        account_id: impl Into<String>,
        roles: I,
    ) -> Result<Self, RadrootsAuthorityError>
    where
        I: IntoIterator<Item = RadrootsActorRole>,
    {
        Self::with_provenance(
            pubkey,
            Some(validate_account_id(account_id)?),
            RadrootsActorSource::LocalAccount,
            roles,
        )
    }

    pub fn remote_signer<I>(
        pubkey: impl AsRef<str>,
        account_id: impl Into<String>,
        roles: I,
    ) -> Result<Self, RadrootsAuthorityError>
    where
        I: IntoIterator<Item = RadrootsActorRole>,
    {
        Self::with_provenance(
            pubkey,
            Some(validate_account_id(account_id)?),
            RadrootsActorSource::RemoteSigner,
            roles,
        )
    }

    pub fn service<I>(
        pubkey: impl AsRef<str>,
        account_id: impl Into<String>,
        roles: I,
    ) -> Result<Self, RadrootsAuthorityError>
    where
        I: IntoIterator<Item = RadrootsActorRole>,
    {
        Self::with_provenance(
            pubkey,
            Some(validate_account_id(account_id)?),
            RadrootsActorSource::Service,
            roles,
        )
    }

    pub fn test<I>(pubkey: impl AsRef<str>, roles: I) -> Result<Self, RadrootsAuthorityError>
    where
        I: IntoIterator<Item = RadrootsActorRole>,
    {
        Self::with_provenance(pubkey, None, RadrootsActorSource::Test, roles)
    }

    fn with_provenance<I>(
        pubkey: impl AsRef<str>,
        account_id: Option<String>,
        source: RadrootsActorSource,
        roles: I,
    ) -> Result<Self, RadrootsAuthorityError>
    where
        I: IntoIterator<Item = RadrootsActorRole>,
    {
        let pubkey = RadrootsPublicKey::parse(pubkey.as_ref())
            .map_err(|_| RadrootsAuthorityError::InvalidActorPubkey)?;
        Ok(Self {
            pubkey,
            roles: roles.into_iter().collect(),
            account_id,
            source,
        })
    }

    pub fn pubkey(&self) -> &RadrootsPublicKey {
        &self.pubkey
    }

    pub fn roles(&self) -> &BTreeSet<RadrootsActorRole> {
        &self.roles
    }

    pub fn account_id(&self) -> Option<&str> {
        self.account_id.as_deref()
    }

    pub fn source(&self) -> RadrootsActorSource {
        self.source
    }

    pub fn satisfies(&self, required_role: RadrootsActorRole) -> bool {
        role_satisfies(&self.roles, required_role)
    }
}

fn validate_account_id(account_id: impl Into<String>) -> Result<String, RadrootsAuthorityError> {
    let account_id = account_id.into();
    if account_id.is_empty() {
        return Err(RadrootsAuthorityError::InvalidActorAccountIdEmpty);
    }
    if account_id.as_str() != account_id.trim() {
        return Err(RadrootsAuthorityError::InvalidActorAccountIdUntrimmed);
    }
    if account_id.chars().any(char::is_control) {
        return Err(RadrootsAuthorityError::InvalidActorAccountIdControlCharacter);
    }
    if account_id.chars().count() > MAX_ACTOR_ACCOUNT_ID_LEN {
        return Err(RadrootsAuthorityError::InvalidActorAccountIdTooLong {
            max_len: MAX_ACTOR_ACCOUNT_ID_LEN,
        });
    }
    Ok(account_id)
}

pub fn role_satisfies(
    actor_roles: &BTreeSet<RadrootsActorRole>,
    required_role: RadrootsActorRole,
) -> bool {
    match required_role {
        RadrootsActorRole::Any => true,
        role => actor_roles.contains(&role),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_64(character: char) -> String {
        std::iter::repeat_n(character, 64).collect()
    }

    #[test]
    fn any_is_satisfied_by_any_actor_context() {
        let actor = RadrootsActorContext::test(hex_64('a'), []).expect("actor");

        assert!(actor.satisfies(RadrootsActorRole::Any));
    }

    #[test]
    fn specific_roles_require_explicit_membership() {
        let actor =
            RadrootsActorContext::test(hex_64('a'), [RadrootsActorRole::Farmer]).expect("actor");

        assert!(actor.satisfies(RadrootsActorRole::Farmer));
        assert!(!actor.satisfies(RadrootsActorRole::Seller));
    }

    #[test]
    fn farmer_does_not_globally_satisfy_seller() {
        let actor =
            RadrootsActorContext::test(hex_64('a'), [RadrootsActorRole::Farmer]).expect("actor");

        assert!(!actor.satisfies(RadrootsActorRole::Seller));
    }

    #[test]
    fn multi_role_actors_satisfy_each_assigned_role() {
        let actor = RadrootsActorContext::test(
            hex_64('a'),
            [RadrootsActorRole::Farmer, RadrootsActorRole::Seller],
        )
        .expect("actor");

        assert!(actor.satisfies(RadrootsActorRole::Farmer));
        assert!(actor.satisfies(RadrootsActorRole::Seller));
        assert!(!actor.satisfies(RadrootsActorRole::Buyer));
    }

    #[test]
    fn local_account_context_carries_validated_account_id() {
        let actor = RadrootsActorContext::local_account(
            hex_64('a'),
            "acct-field-01",
            [RadrootsActorRole::Farmer],
        )
        .expect("actor");

        assert_eq!(actor.source(), RadrootsActorSource::LocalAccount);
        assert_eq!(actor.account_id(), Some("acct-field-01"));
    }

    #[test]
    fn explicit_pubkey_context_has_no_account_id() {
        let actor = RadrootsActorContext::explicit_pubkey(hex_64('a'), [RadrootsActorRole::Seller])
            .expect("actor");

        assert_eq!(actor.source(), RadrootsActorSource::ExplicitPubkey);
        assert_eq!(actor.account_id(), None);
    }

    #[test]
    fn remote_signer_and_service_contexts_carry_account_ids() {
        let remote = RadrootsActorContext::remote_signer(
            hex_64('a'),
            "acct-remote",
            [RadrootsActorRole::Buyer],
        )
        .expect("remote actor");
        let service =
            RadrootsActorContext::service(hex_64('b'), "acct-service", [RadrootsActorRole::Any])
                .expect("service actor");

        assert_eq!(remote.source(), RadrootsActorSource::RemoteSigner);
        assert_eq!(remote.account_id(), Some("acct-remote"));
        assert_eq!(service.source(), RadrootsActorSource::Service);
        assert_eq!(service.account_id(), Some("acct-service"));
    }

    #[test]
    fn account_id_rejects_invalid_values() {
        assert!(matches!(
            RadrootsActorContext::local_account(hex_64('a'), "", []),
            Err(RadrootsAuthorityError::InvalidActorAccountIdEmpty)
        ));
        assert!(matches!(
            RadrootsActorContext::local_account(hex_64('a'), " account ", []),
            Err(RadrootsAuthorityError::InvalidActorAccountIdUntrimmed)
        ));
        assert!(matches!(
            RadrootsActorContext::local_account(hex_64('a'), "account\nid", []),
            Err(RadrootsAuthorityError::InvalidActorAccountIdControlCharacter)
        ));
        assert!(matches!(
            RadrootsActorContext::local_account(
                hex_64('a'),
                core::iter::repeat_n('a', MAX_ACTOR_ACCOUNT_ID_LEN + 1).collect::<String>(),
                []
            ),
            Err(RadrootsAuthorityError::InvalidActorAccountIdTooLong {
                max_len: MAX_ACTOR_ACCOUNT_ID_LEN
            })
        ));
    }

    #[test]
    fn selector_supports_account_and_draft_resolution() {
        let selector = RadrootsActorSelector::account_id("acct-field-01").expect("selector");
        let request = RadrootsActorResolutionRequest::new(
            selector,
            RadrootsActorRole::Seller,
            Some("radroots.listing.published.v1".to_owned()),
        );

        assert!(matches!(
            request.selector,
            RadrootsActorSelector::AccountId(ref account_id) if account_id == "acct-field-01"
        ));
        assert_eq!(request.required_role, RadrootsActorRole::Seller);
        assert_eq!(
            request.contract_id.as_deref(),
            Some("radroots.listing.published.v1")
        );
        assert!(matches!(
            RadrootsActorSelector::SelectedAccount,
            RadrootsActorSelector::SelectedAccount
        ));
        assert!(matches!(
            RadrootsActorSelector::public_key(hex_64('b')).expect("selector"),
            RadrootsActorSelector::PublicKey(_)
        ));
        assert!(matches!(
            RadrootsActorSelector::DraftExpectedPubkey,
            RadrootsActorSelector::DraftExpectedPubkey
        ));
    }
}

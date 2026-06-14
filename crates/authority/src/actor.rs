#![forbid(unsafe_code)]

use crate::RadrootsAuthorityError;
use radroots_events::contract::RadrootsActorRole;
use radroots_events::ids::RadrootsPublicKey;

#[cfg(not(feature = "std"))]
use alloc::collections::BTreeSet;
#[cfg(feature = "std")]
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsActorSource {
    Direct,
    Account,
    GroupMembership,
    RelayAuth,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsActorSelector {
    Pubkey(RadrootsPublicKey),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsActorResolutionRequest {
    pub selector: RadrootsActorSelector,
    pub required_role: RadrootsActorRole,
}

impl RadrootsActorResolutionRequest {
    pub fn new(selector: RadrootsActorSelector, required_role: RadrootsActorRole) -> Self {
        Self {
            selector,
            required_role,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsActorContext {
    pub pubkey: RadrootsPublicKey,
    pub roles: BTreeSet<RadrootsActorRole>,
    pub source: RadrootsActorSource,
}

impl RadrootsActorContext {
    pub fn new(pubkey: impl AsRef<str>) -> Result<Self, RadrootsAuthorityError> {
        Self::with_roles(pubkey, [])
    }

    pub fn with_roles<I>(pubkey: impl AsRef<str>, roles: I) -> Result<Self, RadrootsAuthorityError>
    where
        I: IntoIterator<Item = RadrootsActorRole>,
    {
        Self::with_source_and_roles(pubkey, RadrootsActorSource::Direct, roles)
    }

    pub fn with_source_and_roles<I>(
        pubkey: impl AsRef<str>,
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
            source,
        })
    }

    pub fn pubkey(&self) -> &RadrootsPublicKey {
        &self.pubkey
    }

    pub fn roles(&self) -> &BTreeSet<RadrootsActorRole> {
        &self.roles
    }

    pub fn source(&self) -> RadrootsActorSource {
        self.source
    }

    pub fn satisfies(&self, required_role: RadrootsActorRole) -> bool {
        role_satisfies(&self.roles, required_role)
    }
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
        let actor = RadrootsActorContext::new(hex_64('a')).expect("actor");

        assert!(actor.satisfies(RadrootsActorRole::Any));
    }

    #[test]
    fn specific_roles_require_explicit_membership() {
        let actor = RadrootsActorContext::with_roles(hex_64('a'), [RadrootsActorRole::Farmer])
            .expect("actor");

        assert!(actor.satisfies(RadrootsActorRole::Farmer));
        assert!(!actor.satisfies(RadrootsActorRole::Seller));
    }

    #[test]
    fn farmer_does_not_globally_satisfy_seller() {
        let actor = RadrootsActorContext::with_roles(hex_64('a'), [RadrootsActorRole::Farmer])
            .expect("actor");

        assert!(!actor.satisfies(RadrootsActorRole::Seller));
    }

    #[test]
    fn multi_role_actors_satisfy_each_assigned_role() {
        let actor = RadrootsActorContext::with_roles(
            hex_64('a'),
            [RadrootsActorRole::Farmer, RadrootsActorRole::Seller],
        )
        .expect("actor");

        assert!(actor.satisfies(RadrootsActorRole::Farmer));
        assert!(actor.satisfies(RadrootsActorRole::Seller));
        assert!(!actor.satisfies(RadrootsActorRole::Buyer));
    }
}

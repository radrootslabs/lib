//! Current signing authorization, separate from historical plan integrity.

use radroots_event::contract::{EventAuthoringPolicy, EventStability, event_contract};
use radroots_event_codec::authoring::AuthoredEventPlan;

use crate::{Actor, actor::ActorSource};

/// Current policy decision for one historically valid authored plan.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurrentAuthoringDecision {
    Allowed,
    AllowedDeprecated { warning_code: &'static str },
    Blocked { code: &'static str },
    Revoked { code: &'static str },
}

/// Host or registry authority for current signing policy.
pub trait CurrentAuthoringAuthority: Send + Sync {
    fn evaluate(&self, plan: &AuthoredEventPlan) -> CurrentAuthoringDecision;
}

/// Current immutable registry policy.
#[derive(Clone, Copy, Debug, Default)]
pub struct CurrentRegistryAuthority;

impl CurrentAuthoringAuthority for CurrentRegistryAuthority {
    fn evaluate(&self, plan: &AuthoredEventPlan) -> CurrentAuthoringDecision {
        let Some(contract) = event_contract(plan.body().contract().contract_id().as_str()) else {
            return CurrentAuthoringDecision::Blocked {
                code: "contract_not_current",
            };
        };
        if contract.authoring_policy() == EventAuthoringPolicy::ReadOnly {
            return CurrentAuthoringDecision::Revoked {
                code: "contract_read_only",
            };
        }
        match contract.stability {
            EventStability::Stable => CurrentAuthoringDecision::Allowed,
            EventStability::Experimental => CurrentAuthoringDecision::AllowedDeprecated {
                warning_code: "contract_experimental",
            },
        }
    }
}

/// Whether a request explicitly accepts a currently deprecated contract.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DeprecatedPlanPolicy {
    #[default]
    Deny,
    Allow,
}

/// Host-owned policy limiting which validated actor provenance may sign.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ManagedSigningPolicy {
    #[default]
    AnyValidatedSource,
    AccountBackedOnly,
    LocalAccountOnly,
}

impl ManagedSigningPolicy {
    #[must_use]
    pub const fn permits(self, actor: &Actor) -> bool {
        match self {
            Self::AnyValidatedSource => true,
            Self::AccountBackedOnly => actor.source().account_id().is_some(),
            Self::LocalAccountOnly => matches!(actor.source(), ActorSource::LocalAccount(_)),
        }
    }
}

#[cfg(test)]
mod tests {
    use radroots_event::contract::AuthorRole;
    use radroots_identity::{AccountId, PublicKey};

    use super::*;

    const KEY: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";

    #[test]
    fn managed_signing_policy_covers_every_provenance_class() {
        let public_key = PublicKey::from_hex(KEY).expect("public key");
        let account_id = AccountId::from_hex(KEY).expect("account ID");
        let explicit = Actor::new(
            public_key,
            ActorSource::ExplicitPublicKey,
            [AuthorRole::Any],
        )
        .expect("explicit actor");
        let local = Actor::new(
            public_key,
            ActorSource::LocalAccount(account_id),
            [AuthorRole::Any],
        )
        .expect("local actor");
        let remote = Actor::new(
            public_key,
            ActorSource::RemoteSigner(account_id),
            [AuthorRole::Any],
        )
        .expect("remote actor");

        for actor in [&explicit, &local, &remote] {
            assert!(ManagedSigningPolicy::AnyValidatedSource.permits(actor));
        }
        assert!(!ManagedSigningPolicy::AccountBackedOnly.permits(&explicit));
        assert!(ManagedSigningPolicy::AccountBackedOnly.permits(&local));
        assert!(ManagedSigningPolicy::AccountBackedOnly.permits(&remote));
        assert!(!ManagedSigningPolicy::LocalAccountOnly.permits(&explicit));
        assert!(ManagedSigningPolicy::LocalAccountOnly.permits(&local));
        assert!(!ManagedSigningPolicy::LocalAccountOnly.permits(&remote));
    }
}

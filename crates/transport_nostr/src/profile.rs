//! Validated product relay profiles and directional access policy.

use crate::{Error, RelayUrl, RelayUrlPolicy};
use std::collections::BTreeSet;

/// Directional access authorized for one configured relay.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum RelayAccess {
    /// Queries and subscriptions are allowed; publication is never attempted.
    ReadOnly,
    /// Queries, subscriptions, and publication are allowed.
    ReadWrite,
}

impl RelayAccess {
    /// Returns whether the profile authorizes event reads.
    #[must_use]
    pub const fn can_read(self) -> bool {
        true
    }

    /// Returns whether the profile authorizes event publication.
    #[must_use]
    pub const fn can_write(self) -> bool {
        matches!(self, Self::ReadWrite)
    }
}

/// Host environment whose network trust rules produced a relay profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum RelayProfileKind {
    /// Public-Internet profile.
    Public,
    /// Development-only profile restricted to exact loopback destinations.
    Simulator,
    /// Physical-device profile using explicit TLS endpoints on trusted networks.
    Device,
}

/// One canonical relay endpoint with explicit network and access policy.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelayEndpoint {
    url: RelayUrl,
    policy: RelayUrlPolicy,
    access: RelayAccess,
}

impl RelayEndpoint {
    /// Parses one endpoint with explicit destination and access policy.
    pub fn new(
        value: impl AsRef<str>,
        policy: RelayUrlPolicy,
        access: RelayAccess,
    ) -> Result<Self, Error> {
        Ok(Self {
            url: RelayUrl::parse(value, policy)?,
            policy,
            access,
        })
    }

    /// Returns the canonical relay URL.
    #[must_use]
    pub const fn url(&self) -> &RelayUrl {
        &self.url
    }

    /// Returns the destination policy applied before and after DNS resolution.
    #[must_use]
    pub const fn policy(&self) -> RelayUrlPolicy {
        self.policy
    }

    /// Returns the read/write authority declared by the profile.
    #[must_use]
    pub const fn access(&self) -> RelayAccess {
        self.access
    }
}

/// Complete validated relay selection for one host environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayProfile {
    kind: RelayProfileKind,
    endpoints: Vec<RelayEndpoint>,
}

impl RelayProfile {
    /// Builds an explicit profile with directional access per endpoint.
    ///
    /// This constructor does not inject a bundled endpoint. Callers provide the
    /// complete validated set they intend to use.
    pub fn explicit<I>(kind: RelayProfileKind, endpoints: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = RelayEndpoint>,
    {
        let endpoints = endpoints
            .into_iter()
            .take(crate::client::MAX_RELAYS + 1)
            .collect::<Vec<_>>();
        Self::validated(kind, endpoints)
    }

    fn validated(kind: RelayProfileKind, endpoints: Vec<RelayEndpoint>) -> Result<Self, Error> {
        if endpoints.is_empty() {
            return Err(Error::EmptyRelaySet);
        }
        if endpoints.len() > crate::client::MAX_RELAYS {
            return Err(Error::TooManyRelays {
                max: crate::client::MAX_RELAYS,
                actual: endpoints.len(),
            });
        }
        let mut seen = BTreeSet::new();
        for endpoint in &endpoints {
            let policy_allowed = match kind {
                RelayProfileKind::Public => endpoint.policy == RelayUrlPolicy::Public,
                RelayProfileKind::Simulator => endpoint.policy == RelayUrlPolicy::Local,
                RelayProfileKind::Device => matches!(
                    endpoint.policy,
                    RelayUrlPolicy::Public | RelayUrlPolicy::PrivateNetwork
                ),
            };
            if !policy_allowed {
                return Err(Error::RelayProfilePolicyMismatch);
            }
            if !seen.insert(endpoint.url.clone()) {
                return Err(Error::DuplicateRelayUrl {
                    url: endpoint.url.to_string(),
                });
            }
        }
        Ok(Self { kind, endpoints })
    }

    /// Returns the selected host-environment profile.
    #[must_use]
    pub const fn kind(&self) -> RelayProfileKind {
        self.kind
    }

    /// Returns endpoints in deterministic profile order.
    #[must_use]
    pub fn endpoints(&self) -> &[RelayEndpoint] {
        self.endpoints.as_slice()
    }
}

#[cfg(test)]
pub(crate) fn test_profile<I, S>(
    kind: RelayProfileKind,
    policy: RelayUrlPolicy,
    values: I,
) -> Result<RelayProfile, Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let endpoints = values
        .into_iter()
        .map(|value| RelayEndpoint::new(value, policy, RelayAccess::ReadWrite))
        .collect::<Result<Vec<_>, _>>()?;
    RelayProfile::explicit(kind, endpoints)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_profile_requires_every_public_destination_and_authority() {
        let profile = RelayProfile::explicit(
            RelayProfileKind::Public,
            [RelayEndpoint::new(
                "wss://write.example",
                RelayUrlPolicy::Public,
                RelayAccess::ReadWrite,
            )
            .expect("endpoint")],
        )
        .expect("public profile");
        assert_eq!(profile.kind(), RelayProfileKind::Public);
        assert_eq!(profile.endpoints().len(), 1);
        assert_eq!(profile.endpoints()[0].url().as_str(), "wss://write.example");
        assert_eq!(profile.endpoints()[0].access(), RelayAccess::ReadWrite);
    }

    #[test]
    fn explicit_profile_preserves_directional_access_without_injecting_endpoints() {
        let profile = RelayProfile::explicit(
            RelayProfileKind::Public,
            [
                RelayEndpoint::new(
                    "wss://read.example",
                    RelayUrlPolicy::Public,
                    RelayAccess::ReadOnly,
                )
                .expect("read endpoint"),
                RelayEndpoint::new(
                    "wss://write.example",
                    RelayUrlPolicy::Public,
                    RelayAccess::ReadWrite,
                )
                .expect("write endpoint"),
            ],
        )
        .expect("explicit profile");

        assert_eq!(profile.endpoints().len(), 2);
        assert_eq!(profile.endpoints()[0].access(), RelayAccess::ReadOnly);
        assert_eq!(profile.endpoints()[1].access(), RelayAccess::ReadWrite);
        assert!(!profile.endpoints()[0].access().can_write());
        assert!(profile.endpoints()[1].access().can_write());
    }

    #[test]
    fn profile_kind_rejects_mismatched_explicit_destination_policy() {
        let public = RelayEndpoint::new(
            "wss://relay.example",
            RelayUrlPolicy::Public,
            RelayAccess::ReadOnly,
        )
        .expect("public endpoint");
        let local = RelayEndpoint::new(
            "ws://127.0.0.1:7447",
            RelayUrlPolicy::Local,
            RelayAccess::ReadWrite,
        )
        .expect("local endpoint");
        let private = RelayEndpoint::new(
            "wss://10.0.0.5:7447",
            RelayUrlPolicy::PrivateNetwork,
            RelayAccess::ReadWrite,
        )
        .expect("private endpoint");

        assert!(RelayProfile::explicit(RelayProfileKind::Public, [public.clone()]).is_ok());
        assert!(RelayProfile::explicit(RelayProfileKind::Simulator, [local.clone()]).is_ok());
        assert!(RelayProfile::explicit(RelayProfileKind::Device, [public]).is_ok());
        assert!(RelayProfile::explicit(RelayProfileKind::Device, [private]).is_ok());
        assert_eq!(
            RelayProfile::explicit(RelayProfileKind::Public, [local]).unwrap_err(),
            Error::RelayProfilePolicyMismatch
        );
    }

    #[test]
    fn empty_and_duplicate_profiles_remain_rejected() {
        assert!(
            RelayProfile::explicit(RelayProfileKind::Public, Vec::<RelayEndpoint>::new()).is_err()
        );
        let endpoint = RelayEndpoint::new(
            "wss://relay.example",
            RelayUrlPolicy::Public,
            RelayAccess::ReadOnly,
        )
        .expect("endpoint");
        assert!(
            RelayProfile::explicit(RelayProfileKind::Public, [endpoint.clone(), endpoint]).is_err()
        );
    }

    #[test]
    fn excessive_and_infinite_endpoint_iterators_terminate_at_the_public_bound() {
        let endpoint = RelayEndpoint::new(
            "wss://relay.example",
            RelayUrlPolicy::Public,
            RelayAccess::ReadOnly,
        )
        .expect("endpoint");
        assert_eq!(
            RelayProfile::explicit(RelayProfileKind::Public, std::iter::repeat(endpoint),)
                .unwrap_err(),
            Error::TooManyRelays {
                max: crate::client::MAX_RELAYS,
                actual: crate::client::MAX_RELAYS + 1,
            }
        );
    }
}

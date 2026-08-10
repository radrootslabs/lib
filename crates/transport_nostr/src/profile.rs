//! Validated product relay profiles and directional access policy.

use crate::{Error, RelayUrl, RelayUrlPolicy};
use std::collections::BTreeSet;

/// Bundled canonical Radroots relay used for reads and publication.
pub const DEFAULT_PUBLIC_RELAY: &str = "wss://radroots.org";

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
    /// Public-Internet profile with the bundled canonical relay.
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
    fn new(
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
    pub fn explicit<I, S>(kind: RelayProfileKind, endpoints: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = (S, RelayAccess)>,
        S: AsRef<str>,
    {
        let policy = match kind {
            RelayProfileKind::Public => RelayUrlPolicy::Public,
            RelayProfileKind::Simulator => RelayUrlPolicy::Local,
            RelayProfileKind::Device => RelayUrlPolicy::PrivateNetwork,
        };
        let endpoints = endpoints
            .into_iter()
            .map(|(url, access)| RelayEndpoint::new(url, policy, access))
            .collect::<Result<Vec<_>, _>>()?;
        Self::validated(kind, endpoints)
    }

    /// Builds the ordinary public profile.
    ///
    /// `wss://radroots.org/` is always present as read-write. Every additional
    /// writable relay must be a TLS public-Internet destination. Supplying the
    /// bundled relay again is rejected as a duplicate.
    pub fn public<I, S>(writable_relays: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut endpoints = vec![RelayEndpoint::new(
            DEFAULT_PUBLIC_RELAY,
            RelayUrlPolicy::Public,
            RelayAccess::ReadWrite,
        )?];
        endpoints.extend(parse_endpoints(
            writable_relays,
            RelayUrlPolicy::Public,
            RelayAccess::ReadWrite,
        )?);
        Self::validated(RelayProfileKind::Public, endpoints)
    }

    /// Builds a development-only profile from exact loopback relays.
    ///
    /// Plaintext `ws://` is accepted only by this profile. At least one relay
    /// is required and every endpoint is writable.
    pub fn simulator<I, S>(loopback_relays: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let endpoints = parse_endpoints(
            loopback_relays,
            RelayUrlPolicy::Local,
            RelayAccess::ReadWrite,
        )?;
        Self::validated(RelayProfileKind::Simulator, endpoints)
    }

    /// Builds a physical-device profile from explicit writable TLS endpoints.
    ///
    /// The bundled canonical relay remains read-write. Additional endpoints may
    /// resolve to public or private addresses, but loopback, unspecified, and
    /// multicast destinations remain forbidden before and after resolution.
    pub fn device<I, S>(writable_relays: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut endpoints = vec![RelayEndpoint::new(
            DEFAULT_PUBLIC_RELAY,
            RelayUrlPolicy::Public,
            RelayAccess::ReadWrite,
        )?];
        endpoints.extend(parse_endpoints(
            writable_relays,
            RelayUrlPolicy::PrivateNetwork,
            RelayAccess::ReadWrite,
        )?);
        Self::validated(RelayProfileKind::Device, endpoints)
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

fn parse_endpoints<I, S>(
    values: I,
    policy: RelayUrlPolicy,
    access: RelayAccess,
) -> Result<Vec<RelayEndpoint>, Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    values
        .into_iter()
        .map(|value| RelayEndpoint::new(value, policy, access))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_profile_authorizes_the_canonical_relay_for_publication() {
        let profile = RelayProfile::public(["wss://write.example"]).expect("public profile");
        assert_eq!(profile.kind(), RelayProfileKind::Public);
        assert_eq!(profile.endpoints().len(), 2);
        assert_eq!(profile.endpoints()[0].url().as_str(), DEFAULT_PUBLIC_RELAY);
        assert_eq!(profile.endpoints()[0].access(), RelayAccess::ReadWrite);
        assert_eq!(profile.endpoints()[1].access(), RelayAccess::ReadWrite);
        assert!(RelayProfile::public([DEFAULT_PUBLIC_RELAY]).is_err());
        assert!(RelayProfile::public(["ws://public.example"]).is_err());
    }

    #[test]
    fn explicit_profile_preserves_directional_access_without_injecting_endpoints() {
        let profile = RelayProfile::explicit(
            RelayProfileKind::Public,
            [
                ("wss://read.example", RelayAccess::ReadOnly),
                ("wss://write.example", RelayAccess::ReadWrite),
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
    fn loopback_is_confined_to_the_simulator_profile() {
        assert!(RelayProfile::simulator(["ws://127.0.0.1:7447"]).is_ok());
        assert!(RelayProfile::simulator(["wss://localhost:7447"]).is_ok());
        assert!(RelayProfile::simulator(Vec::<String>::new()).is_err());
        assert!(RelayProfile::public(["ws://127.0.0.1:7447"]).is_err());
        assert!(RelayProfile::device(["wss://127.0.0.1:7447"]).is_err());
    }

    #[test]
    fn device_profile_requires_tls_and_rejects_duplicate_authority() {
        let profile = RelayProfile::device(["wss://10.0.0.5:7447"]).expect("device profile");
        assert_eq!(profile.kind(), RelayProfileKind::Device);
        assert_eq!(profile.endpoints()[0].access(), RelayAccess::ReadWrite);
        assert_eq!(
            profile.endpoints()[1].policy(),
            RelayUrlPolicy::PrivateNetwork
        );
        assert!(RelayProfile::device(["ws://10.0.0.5:7447"]).is_err());
        assert!(RelayProfile::device([DEFAULT_PUBLIC_RELAY]).is_err());
    }
}
